#[path = "../src/services/quota.rs"]
mod quota_service;
#[path = "../src/services/sound_theme.rs"]
mod sound_theme_service;

use tempfile::TempDir;

#[test]
fn quota_command_never_fabricates_usage() {
    let snapshots = quota_service::DesktopQuotaRegistry::default().list_snapshots();
    assert_eq!(snapshots.len(), 6);
    assert!(snapshots.iter().all(|snapshot| {
        snapshot.used.is_none() && snapshot.remaining.is_none() && snapshot.limit.is_none()
    }));
    let configured = snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.service == "claude"
                || snapshot.service == "codex"
                || snapshot.service == "gemini"
                || snapshot.service == "kimi"
        })
        .collect::<Vec<_>>();
    assert_eq!(configured.len(), 4);
    assert!(configured.iter().all(|snapshot| {
        snapshot.message.as_ref().is_some_and(|message| message.contains("set "))
    }));
}

#[test]
fn sound_theme_listing_includes_validated_builtin_theme() {
    let dir = TempDir::new().expect("tempdir");
    let themes = sound_theme_service::list_installed_themes(dir.path()).expect("themes");
    assert_eq!(themes.len(), 1);
    themes[0].validate().unwrap();
    assert_eq!(themes[0].id, "builtin.8-bit");
}
