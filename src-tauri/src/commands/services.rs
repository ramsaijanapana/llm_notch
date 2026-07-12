#[path = "../services/quota.rs"]
mod quota_service;

use crate::services::sound_theme as sound_theme_service;
use notch_services::sound::SoundTheme;
use notch_services::sound_pack::SoundPackValidation;
use quota_service::{DesktopQuotaRegistry, QuotaSnapshotView};
use sound_theme_service::{
    ImportSoundPackRequest, SoundPlayRequest, SoundPlayResult, SoundRoutingPreview,
    SoundRoutingPreviewRequest,
};
use tauri::AppHandle;

const MAX_PACK_BASE64_LEN: usize = 12 * 1024 * 1024;

/// Lists real snapshots when providers exist and explicit unavailable rows otherwise.
#[tauri::command]
pub fn list_quota_snapshots() -> Vec<QuotaSnapshotView> {
    DesktopQuotaRegistry::default().list_snapshots()
}

#[tauri::command]
pub fn get_sound_themes(app: AppHandle) -> Result<Vec<SoundTheme>, String> {
    let themes_root = app_data_sound_themes_root(&app)?;
    sound_theme_service::list_installed_themes(&themes_root).map_err(|error| error.to_string())
}

/// Validates routing and calculates its effect. It never triggers native playback.
#[tauri::command]
pub fn preview_sound_routing(
    request: SoundRoutingPreviewRequest,
) -> Result<SoundRoutingPreview, String> {
    sound_theme_service::preview_routing(&request).map_err(|error| error.to_string())
}

/// Plays a validated theme event through the native audio backend.
#[tauri::command]
pub fn play_sound_event(
    app: AppHandle,
    request: SoundPlayRequest,
) -> Result<SoundPlayResult, String> {
    let themes_root = app_data_sound_themes_root(&app)?;
    sound_theme_service::play_sound(&request, &themes_root).map_err(|error| error.to_string())
}

/// Validates a `.notch-sound` zip payload and optionally installs it under app data.
#[tauri::command]
pub fn import_sound_pack(
    app: AppHandle,
    request: ImportSoundPackRequest,
) -> Result<SoundPackValidation, String> {
    if request.pack_base64.len() > MAX_PACK_BASE64_LEN {
        return Err("sound pack payload is too large".into());
    }
    let pack_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        request.pack_base64.as_bytes(),
    )
    .map_err(|error| format!("invalid base64 sound pack payload: {error}"))?;
    let themes_root = app_data_sound_themes_root(&app)?;
    std::fs::create_dir_all(&themes_root)
        .map_err(|error| format!("could not prepare sound themes directory: {error}"))?;
    sound_theme_service::import_sound_pack(&pack_bytes, &themes_root, request.install)
        .map_err(|error| error.to_string())
}

fn app_data_sound_themes_root(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    sound_theme_service::themes_root_from_app(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_return_honest_bootstrap_data() {
        let quotas = list_quota_snapshots();
        assert_eq!(quotas.len(), 6);
        assert!(quotas.iter().all(|quota| {
            quota.used.is_none() && quota.remaining.is_none() && quota.limit.is_none()
        }));

        let claude = quotas.iter().find(|quota| quota.service == "claude").unwrap();
        assert_eq!(claude.availability, quota_service::QuotaAvailability::Unavailable);
        assert!(claude.message.as_ref().unwrap().contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn import_service_rejects_invalid_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = sound_theme_service::import_sound_pack(b"not-a-pack", dir.path(), false);
        assert!(result.is_err());
    }
}
