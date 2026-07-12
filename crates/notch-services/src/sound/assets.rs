use std::path::Path;

use super::{ResolvedAudio, SoundAsset, SoundError, SoundEvent, SoundTheme};

const SAMPLE_RATE: u32 = 22_050;
pub const BUILTIN_DURATION_MS: u32 = 500;

fn encode_wav_u8_mono(samples: &[u8], sample_rate: u32) -> Vec<u8> {
    let data_size = u32::try_from(samples.len()).expect("sample buffer fits in u32");
    let block_align = 1_u16;
    let byte_rate = sample_rate * u32::from(block_align);
    let mut buffer = Vec::with_capacity(44 + samples.len());
    buffer.extend_from_slice(b"RIFF");
    buffer.extend_from_slice(&(36 + data_size).to_le_bytes());
    buffer.extend_from_slice(b"WAVE");
    buffer.extend_from_slice(b"fmt ");
    buffer.extend_from_slice(&16_u32.to_le_bytes());
    buffer.extend_from_slice(&1_u16.to_le_bytes());
    buffer.extend_from_slice(&1_u16.to_le_bytes());
    buffer.extend_from_slice(&sample_rate.to_le_bytes());
    buffer.extend_from_slice(&byte_rate.to_le_bytes());
    buffer.extend_from_slice(&block_align.to_le_bytes());
    buffer.extend_from_slice(&8_u16.to_le_bytes());
    buffer.extend_from_slice(b"data");
    buffer.extend_from_slice(&data_size.to_le_bytes());
    buffer.extend_from_slice(samples);
    buffer
}

fn square_wave_samples(base_hz: f32, duration_ms: u32, pitch_bend: f32) -> Vec<u8> {
    let sample_count = (u64::from(SAMPLE_RATE) * u64::from(duration_ms) / 1000) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for index in 0..sample_count {
        let progress = index as f32 / sample_count.max(1) as f32;
        let frequency = base_hz * (1.0 + pitch_bend * progress);
        let phase = (index as f32 / SAMPLE_RATE as f32) * frequency;
        let value = if phase.fract() < 0.5 { 220_u8 } else { 36_u8 };
        samples.push(value);
    }
    samples
}

fn chiptune_wav(base_hz: f32, pitch_bend: f32) -> Vec<u8> {
    let samples = square_wave_samples(base_hz, BUILTIN_DURATION_MS, pitch_bend);
    encode_wav_u8_mono(&samples, SAMPLE_RATE)
}

fn asset_for_event(event: SoundEvent) -> &'static [u8] {
    use std::sync::LazyLock;

    static APPROVAL: LazyLock<Vec<u8>> = LazyLock::new(|| chiptune_wav(988.0, 0.15));
    static QUESTION: LazyLock<Vec<u8>> = LazyLock::new(|| chiptune_wav(740.0, 0.35));
    static COMPLETED: LazyLock<Vec<u8>> = LazyLock::new(|| chiptune_wav(660.0, 0.55));
    static FAILED: LazyLock<Vec<u8>> = LazyLock::new(|| chiptune_wav(220.0, -0.10));
    static NOTIFICATION: LazyLock<Vec<u8>> = LazyLock::new(|| chiptune_wav(880.0, 0.0));

    match event {
        SoundEvent::Approval => APPROVAL.as_slice(),
        SoundEvent::Question => QUESTION.as_slice(),
        SoundEvent::Completed => COMPLETED.as_slice(),
        SoundEvent::Failed => FAILED.as_slice(),
        SoundEvent::Notification => NOTIFICATION.as_slice(),
    }
}

pub fn builtin_asset_size_bytes() -> u64 {
    asset_for_event(SoundEvent::Approval).len() as u64
}

pub fn resolve_builtin_asset(path: &str) -> Result<&'static [u8], SoundError> {
    let file_name = path
        .strip_prefix("builtin/8-bit/")
        .ok_or_else(|| SoundError::Backend(format!("unsupported asset path {path}")))?;
    let event = match file_name {
        "approval.wav" => SoundEvent::Approval,
        "question.wav" => SoundEvent::Question,
        "completed.wav" => SoundEvent::Completed,
        "failed.wav" => SoundEvent::Failed,
        "notification.wav" => SoundEvent::Notification,
        _ => {
            return Err(SoundError::Backend(format!(
                "unknown built-in sound asset {path}"
            )));
        }
    };
    Ok(asset_for_event(event))
}

pub fn validate_theme_id(theme_id: &str) -> Result<(), SoundError> {
    if theme_id.trim().is_empty()
        || theme_id.contains('/')
        || theme_id.contains('\\')
        || theme_id.contains("..")
        || theme_id.chars().any(char::is_control)
    {
        return Err(SoundError::UnsafePath(theme_id.into()));
    }
    Ok(())
}

pub fn load_installed_theme(themes_root: &Path, theme_id: &str) -> Result<SoundTheme, SoundError> {
    validate_theme_id(theme_id)?;
    if crate::sound_pack::is_reserved_theme_id(theme_id) {
        return Err(SoundError::Backend(format!(
            "unknown sound theme {theme_id}"
        )));
    }
    let theme_dir = themes_root.join(theme_id);
    let manifest_path = theme_dir.join("theme.json");
    let bytes = std::fs::read(&manifest_path).map_err(|error| {
        SoundError::Backend(format!(
            "installed theme `{theme_id}` is missing or unreadable: {error}"
        ))
    })?;
    let theme: SoundTheme = serde_json::from_slice(&bytes).map_err(|error| {
        SoundError::InvalidManifest(format!(
            "invalid theme manifest at {}: {error}",
            manifest_path.display()
        ))
    })?;
    if theme.id != theme_id {
        return Err(SoundError::InvalidManifest(format!(
            "theme manifest id `{}` does not match installed directory `{theme_id}`",
            theme.id
        )));
    }
    theme.validate()?;
    Ok(theme)
}

pub fn resolve_playback_asset(
    themes_root: &Path,
    theme: &SoundTheme,
    asset: &SoundAsset,
) -> Result<ResolvedAudio, SoundError> {
    if asset.path.starts_with("builtin/") {
        return Ok(ResolvedAudio::Embedded(resolve_builtin_asset(&asset.path)?));
    }

    validate_theme_id(&theme.id)?;
    let theme_dir = themes_root.join(&theme.id);
    let asset_path = theme_dir.join(&asset.path);
    let canonical_theme = std::fs::canonicalize(&theme_dir).map_err(|error| {
        SoundError::Backend(format!(
            "installed theme `{}` is missing or unreadable: {error}",
            theme.id
        ))
    })?;
    let canonical_asset = std::fs::canonicalize(&asset_path).map_err(|error| {
        SoundError::Backend(format!(
            "sound asset {} is missing or unreadable: {error}",
            asset.path
        ))
    })?;
    if !path_is_within(&canonical_asset, &canonical_theme) {
        return Err(SoundError::UnsafePath(asset.path.clone()));
    }
    let metadata = std::fs::metadata(&canonical_asset).map_err(|error| {
        SoundError::Backend(format!(
            "could not read metadata for {}: {error}",
            asset.path
        ))
    })?;
    if metadata.len() != asset.size_bytes {
        return Err(SoundError::InvalidAssetSize {
            path: asset.path.clone(),
            size: asset.size_bytes,
        });
    }
    Ok(ResolvedAudio::File(canonical_asset))
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::sound::SoundEvent;
    use crate::sound_pack::fixtures::{build_zip, pack_entries, sample_theme};
    use crate::sound_pack::{reserved_theme_ids, validate_and_install_pack_bytes};
    use tempfile::TempDir;

    #[test]
    fn generated_assets_are_valid_wav_payloads() {
        for event in [
            SoundEvent::Approval,
            SoundEvent::Question,
            SoundEvent::Completed,
            SoundEvent::Failed,
            SoundEvent::Notification,
        ] {
            let bytes = asset_for_event(event);
            assert!(bytes.starts_with(b"RIFF"));
            assert!(bytes.get(8..12).is_some_and(|header| header == b"WAVE"));
            assert!(bytes.len() > 44);
        }
    }

    #[test]
    fn builtin_paths_resolve_to_embedded_assets() {
        let bytes = resolve_builtin_asset("builtin/8-bit/completed.wav").unwrap();
        assert_eq!(bytes, asset_for_event(SoundEvent::Completed));
    }

    #[test]
    fn installed_theme_assets_resolve_under_theme_directory() {
        let dir = TempDir::new().expect("tempdir");
        let theme = sample_theme();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        validate_and_install_pack_bytes(&zip, dir.path(), &reserved_theme_ids()).expect("install");

        let loaded = load_installed_theme(dir.path(), "community.test-pack").expect("theme");
        let asset = loaded.events.get(&SoundEvent::Completed).expect("asset");
        let resolved = resolve_playback_asset(dir.path(), &loaded, asset).expect("resolved");
        assert!(matches!(resolved, ResolvedAudio::File(_)));
        if let ResolvedAudio::File(path) = resolved {
            assert!(
                path.ends_with("sounds/completed.wav") || path.ends_with("sounds\\completed.wav")
            );
            let theme_dir = dir.path().join("community.test-pack");
            let canonical_theme = theme_dir.canonicalize().expect("canonical theme");
            assert!(path.starts_with(&canonical_theme));
        }
    }

    #[test]
    fn installed_theme_asset_rejects_size_mismatch() {
        let dir = TempDir::new().expect("tempdir");
        let theme = sample_theme();
        let entries = pack_entries(&theme, b"RIFF");
        let zip = build_zip(&entries);
        validate_and_install_pack_bytes(&zip, dir.path(), &reserved_theme_ids()).expect("install");

        let asset_path = dir.path().join("community.test-pack/sounds/completed.wav");
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&asset_path)
            .expect("open asset");
        file.write_all(b"tamper").expect("tamper asset");

        let loaded = load_installed_theme(dir.path(), "community.test-pack").expect("theme");
        let asset = loaded.events.get(&SoundEvent::Completed).expect("asset");
        assert!(matches!(
            resolve_playback_asset(dir.path(), &loaded, asset),
            Err(SoundError::InvalidAssetSize { .. })
        ));
    }

    #[test]
    fn traversal_theme_ids_are_rejected() {
        assert!(matches!(
            load_installed_theme(Path::new("/tmp"), "../escape"),
            Err(SoundError::UnsafePath(_))
        ));
    }
}
