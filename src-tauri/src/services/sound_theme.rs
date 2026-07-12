use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use chrono::Timelike;
use notch_protocol::{
    AttentionKind, PublicSettings, SessionStatus, SoundEvent as ProtocolSoundEvent,
};
use notch_services::sound::{
    PlaybackOutcome, SoundEngine, SoundError, SoundEvent, SoundRouting, SoundTheme,
    builtin_8_bit_theme,
};
use notch_services::sound_pack::{
    SoundPackError, SoundPackValidation, reserved_theme_ids, validate_and_install_pack_bytes,
    validate_pack_bytes,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const DEFAULT_SOUND_THEME_ID: &str = "builtin.8-bit";

static SOUND_ENGINE: LazyLock<Mutex<Result<SoundEngine, SoundError>>> =
    LazyLock::new(|| Mutex::new(SoundEngine::with_default_backend()));

pub fn validated_builtin_themes() -> Result<Vec<SoundTheme>, SoundError> {
    let theme = builtin_8_bit_theme();
    theme.validate()?;
    Ok(vec![theme])
}

pub fn sound_themes_root(base: &Path) -> PathBuf {
    base.join("sound-themes")
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSoundPackRequest {
    pub pack_base64: String,
    pub install: bool,
}

pub fn import_sound_pack(
    pack_bytes: &[u8],
    themes_root: &Path,
    install: bool,
) -> Result<SoundPackValidation, SoundPackError> {
    if install {
        validate_and_install_pack_bytes(pack_bytes, themes_root, &reserved_theme_ids())
    } else {
        validate_pack_bytes(pack_bytes)
    }
}

pub fn list_installed_themes(themes_root: &Path) -> Result<Vec<SoundTheme>, SoundPackError> {
    let mut themes = validated_builtin_themes()
        .map_err(|error| SoundPackError::InstallFailed(error.to_string()))?;
    if !themes_root.is_dir() {
        return Ok(themes);
    }
    for entry in std::fs::read_dir(themes_root).map_err(|error| {
        SoundPackError::InstallFailed(format!("could not read themes directory: {error}"))
    })? {
        let entry = entry.map_err(|error| {
            SoundPackError::InstallFailed(format!("could not read theme entry: {error}"))
        })?;
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let manifest_path = entry.path().join("theme.json");
        if !manifest_path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&manifest_path).map_err(|error| {
            SoundPackError::InstallFailed(format!(
                "could not read {}: {error}",
                manifest_path.display()
            ))
        })?;
        let theme: SoundTheme = serde_json::from_slice(&bytes).map_err(|error| {
            SoundPackError::InstallFailed(format!(
                "invalid theme manifest at {}: {error}",
                manifest_path.display()
            ))
        })?;
        theme.validate().map_err(SoundPackError::InvalidTheme)?;
        if reserved_theme_ids()
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(&theme.id))
        {
            continue;
        }
        themes.push(theme);
    }
    Ok(themes)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundRoutingPreviewRequest {
    pub routing: SoundRouting,
    pub event: SoundEvent,
    pub agent: Option<String>,
    pub local_minute: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundRoutingPreview {
    pub audible: bool,
    pub effective_volume: Option<f32>,
    pub reason: Option<String>,
}

pub fn preview_routing(
    request: &SoundRoutingPreviewRequest,
) -> Result<SoundRoutingPreview, SoundError> {
    let volume = request.routing.effective_volume(
        request.event,
        request.agent.as_deref(),
        request.local_minute,
    )?;
    let reason = if volume.is_some() {
        None
    } else if !request.routing.enabled {
        Some("sound is disabled".into())
    } else {
        Some("quiet hours are active".into())
    };
    Ok(SoundRoutingPreview {
        audible: volume.is_some_and(|volume| volume > 0.0),
        effective_volume: volume,
        reason,
    })
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPlayRequest {
    pub theme_id: String,
    pub event: SoundEvent,
    pub routing: SoundRouting,
    pub agent: Option<String>,
    pub local_minute: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPlayResult {
    pub played: bool,
    pub effective_volume: Option<f32>,
    pub reason: Option<String>,
    pub backend_id: String,
}

impl From<PlaybackOutcome> for SoundPlayResult {
    fn from(outcome: PlaybackOutcome) -> Self {
        Self {
            played: outcome.played,
            effective_volume: outcome.effective_volume,
            reason: outcome.reason,
            backend_id: String::new(),
        }
    }
}

pub fn play_sound(
    request: &SoundPlayRequest,
    themes_root: &Path,
) -> Result<SoundPlayResult, SoundError> {
    let guard = SOUND_ENGINE
        .lock()
        .map_err(|_| SoundError::Backend("sound engine lock poisoned".into()))?;
    let engine = guard
        .as_ref()
        .map_err(|error| SoundError::Backend(error.to_string()))?;
    let outcome = engine.play_event(
        themes_root,
        &request.theme_id,
        request.event,
        &request.routing,
        request.agent.as_deref(),
        request.local_minute,
    )?;
    Ok(SoundPlayResult {
        played: outcome.played,
        effective_volume: outcome.effective_volume,
        reason: outcome.reason,
        backend_id: engine.backend_id().into(),
    })
}

#[derive(Debug, Clone, Copy)]
pub struct SoundNotificationContext<'a> {
    pub sound_enabled: bool,
    pub settings: &'a PublicSettings,
    pub themes_root: &'a Path,
    pub local_minute: u16,
}

pub fn themes_root_from_app(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("app data dir unavailable: {error}"))?;
    Ok(sound_themes_root(&app_data_dir))
}

pub fn resolve_theme_id(selected: Option<&str>) -> String {
    selected
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(DEFAULT_SOUND_THEME_ID)
        .to_string()
}

pub fn local_minute_now() -> u16 {
    chrono::Local::now().hour() as u16 * 60 + chrono::Local::now().minute() as u16
}

pub fn sound_event_for_attention(attention: AttentionKind) -> ProtocolSoundEvent {
    match attention {
        AttentionKind::Approval | AttentionKind::Permission => ProtocolSoundEvent::Approval,
        AttentionKind::Question => ProtocolSoundEvent::Question,
        AttentionKind::Error => ProtocolSoundEvent::Failed,
        AttentionKind::None => ProtocolSoundEvent::Notification,
    }
}

pub fn sound_event_for_alert(kind: notch_core::AlertKind) -> ProtocolSoundEvent {
    match kind {
        notch_core::AlertKind::CpuCritical | notch_core::AlertKind::MemoryHigh => {
            ProtocolSoundEvent::Notification
        }
        notch_core::AlertKind::CpuWarn => ProtocolSoundEvent::Notification,
        notch_core::AlertKind::NewAttention => ProtocolSoundEvent::Notification,
    }
}

pub fn sound_event_for_session_status(status: SessionStatus) -> Option<ProtocolSoundEvent> {
    match status {
        SessionStatus::Completed => Some(ProtocolSoundEvent::Completed),
        SessionStatus::Failed => Some(ProtocolSoundEvent::Failed),
        _ => None,
    }
}

pub fn native_playback_available() -> bool {
    notch_services::sound::backend::native_playback_supported()
}

pub fn routing_from_settings(settings: &PublicSettings) -> SoundRouting {
    let routing = &settings.sound_routing;
    SoundRouting {
        enabled: settings.alert_sound_enabled && routing.enabled,
        volume: routing.volume,
        quiet_hours: routing
            .quiet_hours
            .map(|hours| notch_services::sound::QuietHours {
                start_minute: hours.start_minute,
                end_minute: hours.end_minute,
            }),
        event_volume: routing
            .event_volume
            .iter()
            .map(|(event, volume)| (to_service_sound_event(*event), *volume))
            .collect(),
        agent_volume: routing.agent_volume.clone(),
    }
}

pub fn play_notification_sound(
    ctx: &SoundNotificationContext<'_>,
    event: ProtocolSoundEvent,
    agent: Option<&str>,
) -> Result<PlaybackOutcome, SoundError> {
    if !ctx.sound_enabled {
        return Ok(PlaybackOutcome {
            played: false,
            effective_volume: None,
            reason: Some("sound is disabled".into()),
        });
    }
    if !native_playback_available() {
        return Ok(PlaybackOutcome {
            played: false,
            effective_volume: None,
            reason: Some("native sound playback is not supported on this platform".into()),
        });
    }
    let request = SoundPlayRequest {
        theme_id: resolve_theme_id(ctx.settings.selected_sound_theme_id.as_deref()),
        event: to_service_sound_event(event),
        routing: routing_from_settings(ctx.settings),
        agent: agent.map(str::to_string),
        local_minute: ctx.local_minute,
    };
    let guard = SOUND_ENGINE
        .lock()
        .map_err(|_| SoundError::Backend("sound engine lock poisoned".into()))?;
    let engine = guard
        .as_ref()
        .map_err(|error| SoundError::Backend(error.to_string()))?;
    engine.play_event(
        ctx.themes_root,
        &request.theme_id,
        request.event,
        &request.routing,
        request.agent.as_deref(),
        request.local_minute,
    )
}

fn to_service_sound_event(event: ProtocolSoundEvent) -> SoundEvent {
    match event {
        ProtocolSoundEvent::Approval => SoundEvent::Approval,
        ProtocolSoundEvent::Question => SoundEvent::Question,
        ProtocolSoundEvent::Completed => SoundEvent::Completed,
        ProtocolSoundEvent::Failed => SoundEvent::Failed,
        ProtocolSoundEvent::Notification => SoundEvent::Notification,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use notch_protocol::{PublicSettings, SoundRouting as ProtocolSoundRouting};
    use notch_services::sound::{PlaybackRequest, QuietHours, SoundBackend, SoundEngine};
    use tempfile::TempDir;

    use super::*;

    fn request() -> SoundRoutingPreviewRequest {
        SoundRoutingPreviewRequest {
            routing: SoundRouting {
                enabled: true,
                volume: 0.8,
                quiet_hours: None,
                event_volume: BTreeMap::new(),
                agent_volume: BTreeMap::new(),
            },
            event: SoundEvent::Completed,
            agent: Some("codex".into()),
            local_minute: 720,
        }
    }

    fn play_request() -> SoundPlayRequest {
        SoundPlayRequest {
            theme_id: "builtin.8-bit".into(),
            event: SoundEvent::Completed,
            routing: SoundRouting {
                enabled: true,
                volume: 0.8,
                quiet_hours: None,
                event_volume: BTreeMap::from([(SoundEvent::Completed, 0.5)]),
                agent_volume: BTreeMap::from([("codex".into(), 0.5)]),
            },
            agent: Some("codex".into()),
            local_minute: 720,
        }
    }

    struct NoopBackend;

    impl SoundBackend for NoopBackend {
        fn backend_id(&self) -> &str {
            "noop"
        }

        fn play(&self, _request: &PlaybackRequest) -> Result<(), SoundError> {
            Ok(())
        }

        fn stop_all(&self) -> Result<(), SoundError> {
            Ok(())
        }
    }

    #[test]
    fn resolve_theme_id_defaults_to_builtin() {
        assert_eq!(resolve_theme_id(None), "builtin.8-bit");
        assert_eq!(resolve_theme_id(Some("")), "builtin.8-bit");
        assert_eq!(resolve_theme_id(Some("custom.theme")), "custom.theme");
    }

    #[test]
    fn routing_from_settings_honors_master_toggle_and_quiet_hours() {
        let settings = PublicSettings {
            overlay_enabled: true,
            autostart_enabled: false,
            reduced_motion: false,
            sampling_interval_ms: 1_000,
            selected_display: None,
            show_over_fullscreen: false,
            history_retention_hours: 24,
            alert_sound_enabled: false,
            selected_sound_theme_id: None,
            sound_routing: ProtocolSoundRouting {
                enabled: true,
                volume: 0.5,
                quiet_hours: Some(notch_protocol::QuietHours {
                    start_minute: 600,
                    end_minute: 900,
                }),
                event_volume: Default::default(),
                agent_volume: Default::default(),
            },
        };
        assert!(!routing_from_settings(&settings).enabled);
    }

    #[test]
    fn built_in_theme_is_valid_before_ipc_exposure() {
        let themes = validated_builtin_themes().unwrap();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].id, "builtin.8-bit");
        assert_eq!(themes[0].events.len(), 5);
    }

    #[test]
    fn preview_reports_volume_without_playing_audio() {
        let preview = preview_routing(&request()).unwrap();
        assert!(preview.audible);
        assert_eq!(preview.effective_volume, Some(0.8));
    }

    #[test]
    fn preview_validates_and_explains_quiet_hours() {
        let mut request = request();
        request.routing.quiet_hours = Some(QuietHours {
            start_minute: 600,
            end_minute: 900,
        });
        let preview = preview_routing(&request).unwrap();
        assert!(!preview.audible);
        assert_eq!(preview.reason.as_deref(), Some("quiet hours are active"));

        request.routing.volume = 2.0;
        assert!(matches!(
            preview_routing(&request),
            Err(SoundError::InvalidVolume)
        ));
    }

    #[test]
    #[cfg(not(any(windows, target_os = "macos")))]
    fn play_notification_skips_when_native_playback_unavailable() {
        let dir = TempDir::new().expect("tempdir");
        let settings = PublicSettings {
            overlay_enabled: true,
            autostart_enabled: false,
            reduced_motion: false,
            sampling_interval_ms: 1_000,
            selected_display: None,
            show_over_fullscreen: false,
            history_retention_hours: 24,
            alert_sound_enabled: true,
            selected_sound_theme_id: None,
            sound_routing: ProtocolSoundRouting::default(),
        };
        let ctx = SoundNotificationContext {
            sound_enabled: true,
            settings: &settings,
            themes_root: dir.path(),
            local_minute: 720,
        };
        let outcome =
            play_notification_sound(&ctx, ProtocolSoundEvent::Notification, None).unwrap();
        assert!(!outcome.played);
        assert_eq!(
            outcome.reason.as_deref(),
            Some("native sound playback is not supported on this platform")
        );
    }

    #[test]
    fn play_respects_routing_before_backend() {
        let themes_root = TempDir::new().expect("tempdir");
        let engine = SoundEngine::with_backend(Box::new(NoopBackend));
        let mut request = play_request();
        request.routing.quiet_hours = Some(QuietHours {
            start_minute: 600,
            end_minute: 900,
        });
        let outcome = engine
            .play_event(
                themes_root.path(),
                &request.theme_id,
                request.event,
                &request.routing,
                request.agent.as_deref(),
                request.local_minute,
            )
            .unwrap();
        assert!(!outcome.played);
        assert_eq!(outcome.reason.as_deref(), Some("quiet hours are active"));
    }

    #[test]
    fn play_applies_effective_volume_with_backend() {
        let themes_root = TempDir::new().expect("tempdir");
        let engine = SoundEngine::with_backend(Box::new(NoopBackend));
        let request = play_request();
        let outcome = engine
            .play_event(
                themes_root.path(),
                &request.theme_id,
                request.event,
                &request.routing,
                request.agent.as_deref(),
                request.local_minute,
            )
            .unwrap();
        assert!(outcome.played);
        assert_eq!(outcome.effective_volume, Some(0.2));
    }
}
