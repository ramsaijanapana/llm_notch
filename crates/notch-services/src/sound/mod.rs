mod assets;
pub mod backend;
mod engine;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use engine::SoundEngine;

pub const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_THEME_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_DURATION_MS: u32 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SoundEvent {
    Approval,
    Question,
    Completed,
    Failed,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundAsset {
    /// Relative path inside a theme directory or archive.
    pub path: String,
    pub size_bytes: u64,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundTheme {
    pub schema_version: u16,
    pub id: String,
    pub name: String,
    pub author: String,
    pub events: BTreeMap<SoundEvent, SoundAsset>,
}

impl SoundTheme {
    pub fn validate(&self) -> Result<(), SoundError> {
        if self.schema_version != 1 {
            return Err(SoundError::UnsupportedSchema(self.schema_version));
        }
        if self.id.trim().is_empty() || self.name.trim().is_empty() {
            return Err(SoundError::InvalidManifest(
                "theme id and name must not be empty".into(),
            ));
        }
        if self.events.is_empty() {
            return Err(SoundError::InvalidManifest(
                "a theme must map at least one event".into(),
            ));
        }
        let mut total = 0_u64;
        let mut paths = BTreeSet::new();
        for asset in self.events.values() {
            validate_asset_path(&asset.path)?;
            if !paths.insert(asset.path.to_ascii_lowercase()) {
                return Err(SoundError::DuplicateAsset(asset.path.clone()));
            }
            if asset.size_bytes == 0 || asset.size_bytes > MAX_ASSET_BYTES {
                return Err(SoundError::InvalidAssetSize {
                    path: asset.path.clone(),
                    size: asset.size_bytes,
                });
            }
            if asset.duration_ms == 0 || asset.duration_ms > MAX_DURATION_MS {
                return Err(SoundError::InvalidDuration {
                    path: asset.path.clone(),
                    duration_ms: asset.duration_ms,
                });
            }
            total = total
                .checked_add(asset.size_bytes)
                .ok_or(SoundError::ThemeTooLarge)?;
        }
        if total > MAX_THEME_BYTES {
            return Err(SoundError::ThemeTooLarge);
        }
        Ok(())
    }
}

fn validate_asset_path(path: &str) -> Result<(), SoundError> {
    let normalized = path.replace('\\', "/");
    let has_drive_prefix = normalized
        .as_bytes()
        .get(1)
        .is_some_and(|character| *character == b':');
    if normalized.is_empty()
        || normalized.starts_with('/')
        || has_drive_prefix
        || normalized.chars().any(char::is_control)
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(SoundError::UnsafePath(path.into()));
    }
    let extension = normalized
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    if !matches!(extension.as_deref(), Some("wav" | "ogg")) {
        return Err(SoundError::UnsupportedFormat(path.into()));
    }
    Ok(())
}

pub fn builtin_8_bit_theme() -> SoundTheme {
    let asset_size = assets::builtin_asset_size_bytes();
    let events = [
        (SoundEvent::Approval, "approval.wav"),
        (SoundEvent::Question, "question.wav"),
        (SoundEvent::Completed, "completed.wav"),
        (SoundEvent::Failed, "failed.wav"),
        (SoundEvent::Notification, "notification.wav"),
    ]
    .into_iter()
    .map(|(event, path)| {
        (
            event,
            SoundAsset {
                path: format!("builtin/8-bit/{path}"),
                size_bytes: asset_size,
                duration_ms: assets::BUILTIN_DURATION_MS,
            },
        )
    })
    .collect();
    SoundTheme {
        schema_version: 1,
        id: "builtin.8-bit".into(),
        name: "8-Bit Signals".into(),
        author: "LLM Notch".into(),
        events,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    /// Local minutes after midnight, inclusive.
    pub start_minute: u16,
    /// Local minutes after midnight, exclusive.
    pub end_minute: u16,
}

impl QuietHours {
    pub fn validate(self) -> Result<(), SoundError> {
        if self.start_minute >= 1_440 || self.end_minute >= 1_440 {
            return Err(SoundError::InvalidQuietHours);
        }
        Ok(())
    }

    pub fn contains(self, local_minute: u16) -> bool {
        if self.start_minute == self.end_minute {
            return false;
        }
        if self.start_minute < self.end_minute {
            (self.start_minute..self.end_minute).contains(&local_minute)
        } else {
            local_minute >= self.start_minute || local_minute < self.end_minute
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundRouting {
    pub enabled: bool,
    pub volume: f32,
    pub quiet_hours: Option<QuietHours>,
    #[serde(default)]
    pub event_volume: BTreeMap<SoundEvent, f32>,
    #[serde(default)]
    pub agent_volume: BTreeMap<String, f32>,
}

impl SoundRouting {
    pub fn effective_volume(
        &self,
        event: SoundEvent,
        agent: Option<&str>,
        local_minute: u16,
    ) -> Result<Option<f32>, SoundError> {
        validate_volume(self.volume)?;
        if local_minute >= 1_440 {
            return Err(SoundError::InvalidQuietHours);
        }
        if let Some(hours) = self.quiet_hours {
            hours.validate()?;
            if hours.contains(local_minute) {
                return Ok(None);
            }
        }
        if !self.enabled {
            return Ok(None);
        }
        let event_volume = self.event_volume.get(&event).copied().unwrap_or(1.0);
        let agent_volume = agent
            .and_then(|agent| self.agent_volume.get(agent))
            .copied()
            .unwrap_or(1.0);
        validate_volume(event_volume)?;
        validate_volume(agent_volume)?;
        Ok(Some(self.volume * event_volume * agent_volume))
    }
}

fn validate_volume(volume: f32) -> Result<(), SoundError> {
    if volume.is_finite() && (0.0..=1.0).contains(&volume) {
        Ok(())
    } else {
        Err(SoundError::InvalidVolume)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedAudio {
    Embedded(&'static [u8]),
    File(std::path::PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest {
    pub event: SoundEvent,
    pub agent: Option<String>,
    pub audio: ResolvedAudio,
    pub volume: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackOutcome {
    pub played: bool,
    pub effective_volume: Option<f32>,
    pub reason: Option<String>,
}

/// Implemented by native Windows, macOS, or test audio engines.
pub trait SoundBackend: Send + Sync {
    fn backend_id(&self) -> &str;
    fn play(&self, request: &PlaybackRequest) -> Result<(), SoundError>;
    fn stop_all(&self) -> Result<(), SoundError>;
}

/// Platform factories keep backend selection out of shared routing code.
pub trait SoundBackendFactory: Send + Sync {
    fn create(&self) -> Result<Box<dyn SoundBackend>, SoundError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SoundError {
    #[error("unsupported sound theme schema version {0}")]
    UnsupportedSchema(u16),
    #[error("invalid sound theme manifest: {0}")]
    InvalidManifest(String),
    #[error("unsafe asset path: {0}")]
    UnsafePath(String),
    #[error("unsupported sound format: {0}")]
    UnsupportedFormat(String),
    #[error("duplicate sound asset: {0}")]
    DuplicateAsset(String),
    #[error("invalid size {size} for sound asset {path}")]
    InvalidAssetSize { path: String, size: u64 },
    #[error("invalid duration {duration_ms}ms for sound asset {path}")]
    InvalidDuration { path: String, duration_ms: u32 },
    #[error("sound theme exceeds its size limit")]
    ThemeTooLarge,
    #[error("volume must be a finite number between zero and one")]
    InvalidVolume,
    #[error("quiet hours must use minutes in the range 0..1440")]
    InvalidQuietHours,
    #[error("native sound playback is not supported on this platform")]
    UnsupportedPlatform,
    #[error("native sound backend failed: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_theme_is_complete_and_valid() {
        let theme = builtin_8_bit_theme();
        theme.validate().unwrap();
        assert_eq!(theme.events.len(), 5);
    }

    #[test]
    fn traversal_absolute_and_executable_paths_are_rejected() {
        for path in [
            "../escape.wav",
            "sounds/../escape.wav",
            "/root.wav",
            "C:\\root.wav",
        ] {
            assert!(matches!(
                validate_asset_path(path),
                Err(SoundError::UnsafePath(_))
            ));
        }
        assert!(matches!(
            validate_asset_path("sounds/run.exe"),
            Err(SoundError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn excessive_size_and_duration_are_rejected() {
        let mut theme = builtin_8_bit_theme();
        theme
            .events
            .get_mut(&SoundEvent::Failed)
            .unwrap()
            .size_bytes = MAX_ASSET_BYTES + 1;
        assert!(matches!(
            theme.validate(),
            Err(SoundError::InvalidAssetSize { .. })
        ));

        let mut theme = builtin_8_bit_theme();
        theme
            .events
            .get_mut(&SoundEvent::Failed)
            .unwrap()
            .duration_ms = MAX_DURATION_MS + 1;
        assert!(matches!(
            theme.validate(),
            Err(SoundError::InvalidDuration { .. })
        ));
    }

    #[test]
    fn aggregate_theme_size_is_limited() {
        let mut theme = builtin_8_bit_theme();
        for asset in theme.events.values_mut() {
            asset.size_bytes = MAX_ASSET_BYTES;
        }
        assert!(matches!(theme.validate(), Err(SoundError::ThemeTooLarge)));
    }

    #[test]
    fn empty_manifest_and_control_characters_are_rejected() {
        let mut theme = builtin_8_bit_theme();
        theme.events.clear();
        assert!(matches!(
            theme.validate(),
            Err(SoundError::InvalidManifest(_))
        ));
        assert!(matches!(
            validate_asset_path("sounds/bad\0.wav"),
            Err(SoundError::UnsafePath(_))
        ));
    }

    #[test]
    fn duplicate_paths_are_rejected_case_insensitively() {
        let mut theme = builtin_8_bit_theme();
        theme.events.get_mut(&SoundEvent::Failed).unwrap().path = theme.events
            [&SoundEvent::Completed]
            .path
            .to_ascii_uppercase();
        assert!(matches!(
            theme.validate(),
            Err(SoundError::DuplicateAsset(_))
        ));
    }

    #[test]
    fn quiet_hours_support_same_day_and_overnight_windows() {
        let daytime = QuietHours {
            start_minute: 60,
            end_minute: 120,
        };
        assert!(daytime.contains(90));
        assert!(!daytime.contains(120));

        let overnight = QuietHours {
            start_minute: 1_320,
            end_minute: 420,
        };
        assert!(overnight.contains(1_400));
        assert!(overnight.contains(300));
        assert!(!overnight.contains(800));
    }

    #[test]
    fn routing_combines_volume_and_mutes_during_quiet_hours() {
        let routing = SoundRouting {
            enabled: true,
            volume: 0.8,
            quiet_hours: Some(QuietHours {
                start_minute: 1_320,
                end_minute: 420,
            }),
            event_volume: BTreeMap::from([(SoundEvent::Completed, 0.5)]),
            agent_volume: BTreeMap::from([("codex".into(), 0.5)]),
        };
        assert_eq!(
            routing
                .effective_volume(SoundEvent::Completed, Some("codex"), 720)
                .unwrap(),
            Some(0.2)
        );
        assert_eq!(
            routing
                .effective_volume(SoundEvent::Completed, Some("codex"), 60)
                .unwrap(),
            None
        );
    }

    #[test]
    fn invalid_routing_values_are_rejected() {
        let routing = SoundRouting {
            enabled: true,
            volume: 1.1,
            quiet_hours: None,
            event_volume: BTreeMap::new(),
            agent_volume: BTreeMap::new(),
        };
        assert!(matches!(
            routing.effective_volume(SoundEvent::Question, None, 0),
            Err(SoundError::InvalidVolume)
        ));
    }
}
