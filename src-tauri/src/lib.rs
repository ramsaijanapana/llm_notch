//! Native Tauri 2 host for llm_notch.

mod commands;
mod services;
mod state;
mod stream;
mod window;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use notch_core::{AppCore, SqliteRepository};
use notch_protocol::PublicSettings;
use parking_lot::Mutex;
use services::global_shortcut::ShortcutHandler;
use services::tray::TrayActionHandler;
use services::{
    AlertNotifier, AutostartService, BACKGROUND_LAUNCH_ARG, DEFAULT_DASHBOARD_SHORTCUT, GlobalShortcutService,
    SharedTrayService, TrayMenuAction, TrayMenuModel, TrayService,
};
use state::{HostState, SystemClock, register_builtin_adapters};
use stream::StreamHub;
use tauri::{Manager, RunEvent, Wry};
use tracing::{info, warn};
use window::{SharedWindowCoordinator, WindowCoordinator};

struct DashboardShortcut {
    windows: SharedWindowCoordinator,
}

impl ShortcutHandler for DashboardShortcut {
    fn on_dashboard_shortcut(&self) {
        if let Err(error) = self.windows.lock().toggle_dashboard() {
            warn!(%error, "dashboard shortcut failed");
        }
    }
}

struct DesktopTrayActions {
    app: tauri::AppHandle,
    host: Arc<HostState>,
    windows: SharedWindowCoordinator,
    tray: SharedTrayService<Wry>,
    alert_notifier: Arc<AlertNotifier>,
}

impl TrayActionHandler for DesktopTrayActions {
    fn on_tray_action(&self, action: TrayMenuAction) {
        let result = match action {
            TrayMenuAction::ShowIsland => {
                let result = self.windows.lock().show_overlay();
                if result.is_ok() {
                    self.update_tray_model(true);
                }
                result
            }
            TrayMenuAction::HideIsland => {
                let result = self.windows.lock().hide_overlay();
                if result.is_ok() {
                    self.update_tray_model(false);
                }
                result
            }
            TrayMenuAction::OpenDashboard | TrayMenuAction::Settings => {
                self.windows.lock().open_dashboard(true)
            }
            TrayMenuAction::PauseMetrics => {
                self.host.set_metrics_paused(true);
                self.update_tray_model(self.current_island_visibility());
                return;
            }
            TrayMenuAction::ResumeMetrics => {
                self.host.set_metrics_paused(false);
                self.update_tray_model(self.current_island_visibility());
                return;
            }
            TrayMenuAction::Quit => {
                let host = Arc::clone(&self.host);
                let app = self.app.clone();
                tauri::async_runtime::spawn(async move {
                    host.shutdown().await;
                    app.exit(0);
                });
                return;
            }
        };
        if let Err(error) = result {
            warn!(%error, ?action, "tray action failed");
        }
    }
}

impl DesktopTrayActions {
    fn current_island_visibility(&self) -> bool {
        self.app
            .get_webview_window("overlay")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false)
    }

    fn update_tray_model(&self, island_visible: bool) {
        if let Err(error) = synchronize_tray_model(
            &self.app,
            &self.host,
            &self.tray,
            island_visible,
            &self.alert_notifier,
        ) {
            warn!(%error, "tray menu update failed");
        }
    }
}

pub(crate) fn synchronize_tray_model(
    app: &tauri::AppHandle,
    host: &HostState,
    tray: &SharedTrayService<Wry>,
    island_visible: bool,
    alert_notifier: &AlertNotifier,
) -> Result<(), String> {
    let mut tray = tray
        .lock()
        .map_err(|_| "tray service lock poisoned".to_string())?;
    let resource_alert =
        alert_notifier.observe(&host.active_alerts(), host.settings().alert_sound_enabled);
    let model = tray
        .model()
        .clone()
        .synchronize(host.metrics_paused(), island_visible)
        .with_resource_alert(resource_alert);
    tray.update_model(app, model)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tauri=info".into()),
        )
        .try_init()
        .ok();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let foreground = !args.iter().any(|arg| arg == BACKGROUND_LAUNCH_ARG);
            if let Some(windows) = app.try_state::<SharedWindowCoordinator>() {
                if let Err(error) = windows.lock().open_dashboard(foreground) {
                    warn!(%error, "second instance could not open dashboard");
                }
            } else if let Some(dashboard) = app.get_webview_window("dashboard") {
                let _ = dashboard.show();
                if foreground {
                    let _ = dashboard.set_focus();
                }
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![BACKGROUND_LAUNCH_ARG]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::bootstrap,
            commands::bootstrap::subscribe_stream,
            commands::bootstrap::unsubscribe_stream,
            commands::bootstrap::get_history,
            commands::bootstrap::get_session_events,
            commands::overlay::set_overlay_mode,
            commands::overlay::open_dashboard,
            commands::overlay::open_session,
            commands::overlay::acknowledge_attention,
            commands::settings::get_settings,
            commands::settings::list_displays,
            commands::settings::update_settings,
            commands::settings::purge_history,
            commands::settings::set_startup_enabled,
            commands::settings::set_global_shortcut,
            commands::integration::integration_health,
            commands::integration::preview_connector_change,
            commands::integration::apply_connector_change,
            commands::integration::remove_connector,
            commands::integration::repair_connector,
            commands::integration::rollback_connector,
            commands::integration::connector_health,
            commands::integration::detect_connectors,
        ])
        .setup(|app| {
            let database_path = application_database_path(app.handle())?;
            let stream_hub = Arc::new(StreamHub::default());
            let repository = Arc::new(SqliteRepository::open(&database_path)?);
            harden_database_file(&database_path)?;
            let core = Arc::new(AppCore::new(
                SystemClock,
                repository,
                Arc::clone(&stream_hub),
                default_settings(),
            )?);
            register_builtin_adapters(&core).map_err(anyhow::Error::msg)?;

            let alert_notifier = Arc::new(AlertNotifier::new());
            let host = Arc::new(HostState::with_runtime_dir_and_notifier(
                Arc::clone(&core),
                notch_metrics::MetricsEngine::new(),
                Arc::clone(&stream_hub),
                None,
                Arc::clone(&alert_notifier),
            ));
            let windows = Arc::new(Mutex::new(WindowCoordinator::new(app.handle().clone())));
            app.manage(Arc::clone(&host));
            app.manage(Arc::clone(&windows));

            let shortcuts = Arc::new(StdMutex::new(GlobalShortcutService::<Wry>::default()));
            app.manage(Arc::clone(&shortcuts));
            let shortcut_handler = Arc::new(DashboardShortcut {
                windows: Arc::clone(&windows),
            });
            if let Err(error) = shortcuts
                .lock()
                .map_err(|_| anyhow::anyhow!("shortcut service lock poisoned"))?
                .replace_registration(
                    app.handle(),
                    DEFAULT_DASHBOARD_SHORTCUT,
                    shortcut_handler,
                )
            {
                warn!(%error, "default global shortcut unavailable");
            }

            {
                let mut coordinator = windows.lock();
                let settings = host.settings();
                #[cfg(target_os = "windows")]
                let settings = if settings.show_over_fullscreen {
                    warn!("showOverFullscreen is unsupported on Windows; resetting the persisted preference");
                    let mut corrected = settings;
                    corrected.show_over_fullscreen = false;
                    host.update_settings(corrected.clone())
                        .map_err(anyhow::Error::msg)?;
                    corrected
                } else {
                    settings
                };
                coordinator.set_target_monitor(settings.selected_display.clone());
                if let Err(error) =
                    coordinator.set_show_over_fullscreen(settings.show_over_fullscreen)
                {
                    warn!(%error, "could not apply fullscreen overlay preference");
                }
                let mut setup_result = coordinator.setup_overlay();
                if let Err(error) = &setup_result {
                    if settings.selected_display.is_some() {
                        warn!(
                            %error,
                            "selected display unavailable; falling back to automatic display"
                        );
                        coordinator.set_target_monitor(None);
                        setup_result = coordinator.setup_overlay();
                    }
                }
                match setup_result {
                    Ok(capability) => {
                        info!(?capability, "native overlay capability initialized");
                    }
                    Err(error) => {
                        warn!(%error, "native overlay enhancement unavailable; Tauri flags retained");
                    }
                }
                if !settings.overlay_enabled {
                    if let Err(error) = coordinator.hide_overlay() {
                        warn!(%error, "could not apply persisted hidden overlay state");
                    }
                }
            }

            let tray = Arc::new(StdMutex::new(TrayService::<Wry>::default()));
            if let Some(icon) = app.default_window_icon().cloned() {
                let tray_handler = Arc::new(DesktopTrayActions {
                    app: app.handle().clone(),
                    host: Arc::clone(&host),
                    windows: Arc::clone(&windows),
                    tray: Arc::clone(&tray),
                    alert_notifier: Arc::clone(&alert_notifier),
                });
                let tray_model = TrayMenuModel {
                    island_visible: app
                        .get_webview_window("overlay")
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false),
                    ..TrayMenuModel::default()
                }
                .synchronize(
                    host.metrics_paused(),
                    app.get_webview_window("overlay")
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false),
                );
                if let Err(error) = tray
                    .lock()
                    .map_err(|_| anyhow::anyhow!("tray service lock poisoned"))?
                    .register(app.handle(), tray_model, icon, tray_handler)
                {
                    warn!(%error, "system tray unavailable");
                }
            } else {
                warn!("system tray unavailable because no application icon was generated");
            }
            app.manage(Arc::clone(&tray));
            host.attach_tray_hooks(app.handle().clone(), Arc::clone(&tray));

            if let Err(error) =
                AutostartService::sync_with_settings(app.handle(), host.settings().autostart_enabled)
            {
                warn!(%error, "could not synchronize autostart state");
            }

            host.start_background();
            let signal_host = Arc::clone(&host);
            let signal_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_host.shutdown().await;
                    signal_app.exit(0);
                }
            });
            let initial_snapshot = host.snapshot();
            info!(
                protocol_version = notch_protocol::PROTOCOL_VERSION,
                captured_at_ms = initial_snapshot.captured_at_ms,
                database = %database_path.display(),
                windows = ?app.webview_windows().keys().collect::<Vec<_>>(),
                "llm_notch desktop host initialized"
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let Some(host) = window.app_handle().try_state::<Arc<HostState>>() else {
                    return;
                };
                if host.is_shutting_down() {
                    return;
                }
                api.prevent_close();
                if window.label() == "overlay" {
                    services::tray::log_window_hide_instead_of_quit(window.label());
                    let _ = window.hide();
                    if let Some(tray) =
                        window.app_handle().try_state::<SharedTrayService<Wry>>()
                    {
                        if let Err(error) = synchronize_tray_model(
                            window.app_handle(),
                            &host,
                            &tray,
                            false,
                            &host.alert_notifier(),
                        ) {
                            warn!(%error, "tray menu update after overlay close failed");
                        }
                    }
                } else if window.label() == "dashboard" {
                    if let Some(windows) = window
                        .app_handle()
                        .try_state::<SharedWindowCoordinator>()
                    {
                        let _ = windows.lock().hide_dashboard();
                    } else {
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building llm_notch desktop application");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } => {
            if let Some(host) = app.try_state::<Arc<HostState>>() {
                if !host.is_shutting_down() {
                    api.prevent_exit();
                    let host = Arc::clone(&host);
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        host.shutdown().await;
                        app.exit(0);
                    });
                }
            }
        }
        RunEvent::Exit => {
            if let Some(host) = app.try_state::<Arc<HostState>>() {
                host.begin_shutdown();
            }
        }
        _ => {}
    });
}

fn default_settings() -> PublicSettings {
    PublicSettings {
        overlay_enabled: true,
        autostart_enabled: false,
        reduced_motion: false,
        sampling_interval_ms: 1_000,
        selected_display: None,
        show_over_fullscreen: false,
        history_retention_hours: 24,
        alert_sound_enabled: false,
    }
}

fn application_database_path(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;
    harden_directory(&app_data_dir)?;
    Ok(app_data_dir.join("llm-notch.sqlite3"))
}

fn harden_database_file(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn harden_directory(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}
