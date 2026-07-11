//! System tray menu model and registration.
//!
//! Window close hides the overlay; explicit tray Quit is the only exit path.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Runtime,
    image::Image,
    menu::{Menu, MenuEvent, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};
use tracing::{debug, warn};

/// Visual state reflected by the tray icon asset selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayIconState {
    Idle,
    Active,
    Attention,
    Paused,
    Error,
}

/// Stable menu action identifiers used by the integration owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrayMenuAction {
    ShowIsland,
    HideIsland,
    OpenDashboard,
    PauseMetrics,
    ResumeMetrics,
    Settings,
    Quit,
}

impl TrayMenuAction {
    pub const fn menu_id(self) -> &'static str {
        match self {
            Self::ShowIsland => "tray.show-island",
            Self::HideIsland => "tray.hide-island",
            Self::OpenDashboard => "tray.open-dashboard",
            Self::PauseMetrics => "tray.pause-metrics",
            Self::ResumeMetrics => "tray.resume-metrics",
            Self::Settings => "tray.settings",
            Self::Quit => "tray.quit",
        }
    }

    pub fn from_menu_id(id: &str) -> Option<Self> {
        match id {
            "tray.show-island" => Some(Self::ShowIsland),
            "tray.hide-island" => Some(Self::HideIsland),
            "tray.open-dashboard" => Some(Self::OpenDashboard),
            "tray.pause-metrics" => Some(Self::PauseMetrics),
            "tray.resume-metrics" => Some(Self::ResumeMetrics),
            "tray.settings" => Some(Self::Settings),
            "tray.quit" => Some(Self::Quit),
            _ => None,
        }
    }
}

/// Declarative tray menu model independent of native menu handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuModel {
    pub icon_state: TrayIconState,
    /// Short status line (e.g. active session count).
    pub status_text: String,
    /// Pending/health indicator line shown above actionable items.
    pub indicator_text: String,
    pub metrics_paused: bool,
    pub island_visible: bool,
}

impl Default for TrayMenuModel {
    fn default() -> Self {
        Self {
            icon_state: TrayIconState::Idle,
            status_text: "llm_notch".into(),
            indicator_text: "Healthy".into(),
            metrics_paused: false,
            island_visible: false,
        }
    }
}

impl TrayMenuModel {
    pub fn synchronize(mut self, metrics_paused: bool, island_visible: bool) -> Self {
        self.metrics_paused = metrics_paused;
        self.island_visible = island_visible;
        self.icon_state = if metrics_paused {
            TrayIconState::Paused
        } else {
            TrayIconState::Idle
        };
        self.indicator_text = if metrics_paused {
            "Metrics paused".into()
        } else {
            "Healthy".into()
        };
        self
    }
}

pub trait TrayActionHandler: Send + Sync + 'static {
    fn on_tray_action(&self, action: TrayMenuAction);
}

#[derive(Debug, thiserror::Error)]
pub enum TrayError {
    #[error("tray icon already registered")]
    AlreadyRegistered,
    #[error("tray registration failed: {0}")]
    Registration(String),
}

pub struct TrayService<R: Runtime> {
    tray: Option<TrayIcon<R>>,
    model: TrayMenuModel,
}

pub type SharedTrayService<R> = Arc<Mutex<TrayService<R>>>;

impl<R: Runtime> Default for TrayService<R> {
    fn default() -> Self {
        Self {
            tray: None,
            model: TrayMenuModel::default(),
        }
    }
}

impl<R: Runtime> TrayService<R> {
    pub fn model(&self) -> &TrayMenuModel {
        &self.model
    }

    pub fn update_model(
        &mut self,
        app: &AppHandle<R>,
        model: TrayMenuModel,
    ) -> Result<(), TrayError> {
        let menu =
            build_menu(app, &model).map_err(|error| TrayError::Registration(error.to_string()))?;
        if let Some(tray) = &self.tray {
            tray.set_menu(Some(menu))
                .map_err(|error| TrayError::Registration(error.to_string()))?;
            Self::apply_model_to_tray(tray, &model)?;
        }
        self.model = model;
        Ok(())
    }

    pub fn register(
        &mut self,
        app: &AppHandle<R>,
        model: TrayMenuModel,
        icon: Image<'_>,
        handler: Arc<dyn TrayActionHandler>,
    ) -> Result<(), TrayError> {
        if self.tray.is_some() {
            return Err(TrayError::AlreadyRegistered);
        }

        self.model = model;
        let menu =
            build_menu(app, &self.model).map_err(|e| TrayError::Registration(e.to_string()))?;

        let handler_for_menu = Arc::clone(&handler);
        let handler_for_tray = Arc::clone(&handler);
        app.on_menu_event(move |_app, event: MenuEvent| {
            if let Some(action) = TrayMenuAction::from_menu_id(event.id().as_ref()) {
                handler_for_menu.on_tray_action(action);
            }
        });

        let tray = TrayIconBuilder::with_id("main")
            .icon(icon)
            .menu(&menu)
            .tooltip(&self.model.status_text)
            .show_menu_on_left_click(true)
            .on_tray_icon_event(move |_tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    handler_for_tray.on_tray_action(TrayMenuAction::ShowIsland);
                }
            })
            .build(app)
            .map_err(|e| TrayError::Registration(e.to_string()))?;

        Self::apply_model_to_tray(&tray, &self.model)?;
        self.tray = Some(tray);

        debug!(
            icon_state = ?self.model.icon_state,
            "system tray registered"
        );
        Ok(())
    }

    fn apply_model_to_tray(tray: &TrayIcon<R>, model: &TrayMenuModel) -> Result<(), TrayError> {
        let tooltip = format!("{}\n{}", model.status_text, model.indicator_text);
        tray.set_tooltip(Some(tooltip))
            .map_err(|e| TrayError::Registration(e.to_string()))
    }
}

fn build_menu<R: Runtime>(app: &AppHandle<R>, model: &TrayMenuModel) -> tauri::Result<Menu<R>> {
    let show_hide = if model.island_visible {
        (TrayMenuAction::HideIsland, "Hide Island")
    } else {
        (TrayMenuAction::ShowIsland, "Show Island")
    };

    let pause_resume = pause_resume_action(model.metrics_paused);

    let indicator = MenuItem::with_id(
        app,
        "tray.indicator",
        &model.indicator_text,
        false,
        None::<&str>,
    )?;
    let status = MenuItem::with_id(app, "tray.status", &model.status_text, false, None::<&str>)?;
    let show_hide_item =
        MenuItem::with_id(app, show_hide.0.menu_id(), show_hide.1, true, None::<&str>)?;
    let dashboard = MenuItem::with_id(
        app,
        TrayMenuAction::OpenDashboard.menu_id(),
        "Open Dashboard",
        true,
        None::<&str>,
    )?;
    let pause_resume_item = MenuItem::with_id(
        app,
        pause_resume.0.menu_id(),
        pause_resume.1,
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        app,
        TrayMenuAction::Settings.menu_id(),
        "Settings",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        TrayMenuAction::Quit.menu_id(),
        "Quit",
        true,
        None::<&str>,
    )?;

    Menu::with_items(
        app,
        &[
            &indicator,
            &status,
            &show_hide_item,
            &dashboard,
            &pause_resume_item,
            &settings,
            &quit,
        ],
    )
}

fn pause_resume_action(paused: bool) -> (TrayMenuAction, &'static str) {
    if paused {
        (TrayMenuAction::ResumeMetrics, "Resume Metrics")
    } else {
        (TrayMenuAction::PauseMetrics, "Pause Metrics")
    }
}

/// Logs when a window close was converted to hide rather than quit.
pub fn log_window_hide_instead_of_quit(label: &str) {
    warn!(
        window = label,
        "window close hides overlay; use tray Quit to exit"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_action_ids_round_trip() {
        for action in [
            TrayMenuAction::ShowIsland,
            TrayMenuAction::HideIsland,
            TrayMenuAction::OpenDashboard,
            TrayMenuAction::PauseMetrics,
            TrayMenuAction::ResumeMetrics,
            TrayMenuAction::Settings,
            TrayMenuAction::Quit,
        ] {
            assert_eq!(TrayMenuAction::from_menu_id(action.menu_id()), Some(action));
        }
        assert!(TrayMenuAction::from_menu_id("unknown").is_none());
    }

    #[test]
    fn default_model_is_healthy_idle() {
        let model = TrayMenuModel::default();
        assert_eq!(model.icon_state, TrayIconState::Idle);
        assert!(!model.metrics_paused);
        assert!(!model.island_visible);
    }

    #[test]
    fn paused_model_rebuilds_with_resume_action() {
        assert_eq!(
            pause_resume_action(true),
            (TrayMenuAction::ResumeMetrics, "Resume Metrics")
        );
        assert_eq!(
            pause_resume_action(false),
            (TrayMenuAction::PauseMetrics, "Pause Metrics")
        );
        let hidden_while_paused = TrayMenuModel::default().synchronize(true, false);
        assert!(hidden_while_paused.metrics_paused);
        assert!(!hidden_while_paused.island_visible);
        assert_eq!(
            pause_resume_action(hidden_while_paused.metrics_paused).0,
            TrayMenuAction::ResumeMetrics
        );
        let shown_while_paused = hidden_while_paused.synchronize(true, true);
        assert!(shown_while_paused.metrics_paused);
        assert!(shown_while_paused.island_visible);
        assert_eq!(
            pause_resume_action(shown_while_paused.metrics_paused).0,
            TrayMenuAction::ResumeMetrics
        );
        let resumed = shown_while_paused.synchronize(false, true);
        assert!(!resumed.metrics_paused);
        assert!(resumed.island_visible);
        assert_eq!(
            pause_resume_action(resumed.metrics_paused).0,
            TrayMenuAction::PauseMetrics
        );
    }
}
