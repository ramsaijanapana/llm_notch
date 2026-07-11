fn main() {
    const COMMANDS: &[&str] = &[
        "bootstrap",
        "subscribe_stream",
        "unsubscribe_stream",
        "get_history",
        "get_session_events",
        "set_overlay_mode",
        "open_dashboard",
        "open_session",
        "acknowledge_attention",
        "get_settings",
        "list_displays",
        "update_settings",
        "purge_history",
        "set_startup_enabled",
        "set_global_shortcut",
        "integration_health",
        "preview_connector_change",
        "apply_connector_change",
        "remove_connector",
        "repair_connector",
        "rollback_connector",
        "detect_connectors",
        "connector_health",
    ];
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    tauri_build::try_build(attributes).expect("failed to generate Tauri command permissions");
}
