use std::sync::Arc;

use tauri::{AppHandle, State, Wry};

use crate::commands::error::CommandError;
use crate::commands::types::DisplayOption;
use crate::commands::validation::{
    validate_accelerator, validate_platform_settings, validate_settings,
};
use crate::services::AutostartService;
use crate::services::global_shortcut::{SharedShortcutService, ShortcutHandler};
use crate::services::tray::SharedTrayService;
use crate::state::HostState;
use crate::window::SharedWindowCoordinator;

struct WindowToggle {
    windows: SharedWindowCoordinator,
}

impl ShortcutHandler for WindowToggle {
    fn on_dashboard_shortcut(&self) {
        if let Err(error) = self.windows.lock().toggle_dashboard() {
            tracing::warn!(%error, "global shortcut could not toggle dashboard");
        }
    }
}

#[tauri::command]
pub fn get_settings(
    host: State<'_, Arc<HostState>>,
) -> Result<notch_protocol::PublicSettings, CommandError> {
    Ok(host.settings())
}

#[tauri::command]
pub fn list_displays(
    windows: State<'_, SharedWindowCoordinator>,
) -> Result<Vec<DisplayOption>, CommandError> {
    windows
        .lock()
        .available_displays()
        .map(|displays| {
            displays
                .into_iter()
                .map(|display| DisplayOption {
                    id: display.id,
                    label: display.label,
                    primary: display.primary,
                })
                .collect()
        })
        .map_err(|error| CommandError::Internal(error.to_string()))
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    settings: notch_protocol::PublicSettings,
    host: State<'_, Arc<HostState>>,
    windows: State<'_, SharedWindowCoordinator>,
    tray: State<'_, SharedTrayService<Wry>>,
) -> Result<notch_protocol::PublicSettings, CommandError> {
    validate_settings(&settings)?;
    validate_platform_settings(&settings, cfg!(target_os = "windows"))?;
    let previous = host.settings();

    if previous.autostart_enabled != settings.autostart_enabled {
        AutostartService::set_enabled(&app, settings.autostart_enabled)?;
    }

    let mut coordinator = windows.lock();
    coordinator.set_target_monitor(settings.selected_display.clone());
    let window_result = coordinator
        .set_show_over_fullscreen(settings.show_over_fullscreen)
        .and_then(|()| {
            if settings.overlay_enabled {
                coordinator.show_overlay()
            } else {
                coordinator.hide_overlay()
            }
        });
    if let Err(error) = window_result {
        coordinator.set_target_monitor(previous.selected_display.clone());
        let _ = coordinator.set_show_over_fullscreen(previous.show_over_fullscreen);
        let _ = if previous.overlay_enabled {
            coordinator.show_overlay()
        } else {
            coordinator.hide_overlay()
        };
        if previous.autostart_enabled != settings.autostart_enabled {
            let _ = AutostartService::set_enabled(&app, previous.autostart_enabled);
        }
        return Err(CommandError::Internal(error.to_string()));
    }
    if let Err(error) = crate::synchronize_tray_model(&app, &host, &tray, settings.overlay_enabled)
    {
        coordinator.set_target_monitor(previous.selected_display.clone());
        let _ = coordinator.set_show_over_fullscreen(previous.show_over_fullscreen);
        let _ = if previous.overlay_enabled {
            coordinator.show_overlay()
        } else {
            coordinator.hide_overlay()
        };
        let _ = crate::synchronize_tray_model(&app, &host, &tray, previous.overlay_enabled);
        return Err(CommandError::Internal(error));
    }

    let persisted = match host.update_settings(settings.clone()) {
        Ok(persisted) => persisted,
        Err(error) => {
            coordinator.set_target_monitor(previous.selected_display.clone());
            let _ = coordinator.set_show_over_fullscreen(previous.show_over_fullscreen);
            let _ = if previous.overlay_enabled {
                coordinator.show_overlay()
            } else {
                coordinator.hide_overlay()
            };
            if previous.autostart_enabled != settings.autostart_enabled {
                let _ = AutostartService::set_enabled(&app, previous.autostart_enabled);
            }
            let _ = crate::synchronize_tray_model(&app, &host, &tray, previous.overlay_enabled);
            return Err(CommandError::Internal(error));
        }
    };

    Ok(persisted)
}

#[tauri::command]
pub fn purge_history(host: State<'_, Arc<HostState>>) -> Result<u64, CommandError> {
    host.purge_metric_history().map_err(CommandError::Internal)
}

#[tauri::command]
pub fn set_startup_enabled(
    app: AppHandle,
    enabled: bool,
    host: State<'_, Arc<HostState>>,
) -> Result<(), CommandError> {
    let previous = host.settings();
    AutostartService::set_enabled(&app, enabled)?;
    let mut settings = previous.clone();
    settings.autostart_enabled = enabled;
    if let Err(error) = host.update_settings(settings) {
        let _ = AutostartService::set_enabled(&app, previous.autostart_enabled);
        return Err(CommandError::Internal(error));
    }
    Ok(())
}

#[tauri::command]
pub fn set_global_shortcut(
    app: AppHandle,
    accelerator: String,
    shortcuts: State<'_, SharedShortcutService<Wry>>,
    windows: State<'_, SharedWindowCoordinator>,
) -> Result<(), CommandError> {
    validate_accelerator(&accelerator)?;
    let handler = Arc::new(WindowToggle {
        windows: Arc::clone(&windows),
    });
    shortcuts
        .lock()
        .map_err(|_| CommandError::Internal("shortcut service lock poisoned".into()))?
        .replace_registration(&app, &accelerator, handler)?;
    Ok(())
}
