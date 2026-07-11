//! Trusted autostart control via the Rust-side autostart plugin only.

use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt;
use tracing::info;

pub const BACKGROUND_LAUNCH_ARG: &str = "--background";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutostartState {
    Disabled,
    Enabled,
}

#[derive(Debug, thiserror::Error)]
pub enum AutostartError {
    #[error("autostart query failed: {0}")]
    Query(String),
    #[error("autostart update failed: {0}")]
    Update(String),
}

/// Autostart defaults to off; enabling registers launch with [`BACKGROUND_LAUNCH_ARG`].
pub struct AutostartService;

impl AutostartService {
    pub fn is_enabled<R: Runtime>(app: &AppHandle<R>) -> Result<bool, AutostartError> {
        app.autolaunch()
            .is_enabled()
            .map_err(|e| AutostartError::Query(e.to_string()))
    }

    pub fn set_enabled<R: Runtime>(
        app: &AppHandle<R>,
        enabled: bool,
    ) -> Result<(), AutostartError> {
        let manager = app.autolaunch();
        if enabled {
            manager
                .enable()
                .map_err(|e| AutostartError::Update(e.to_string()))?;
            info!(
                arg = BACKGROUND_LAUNCH_ARG,
                "autostart enabled with background launch arg"
            );
        } else {
            manager
                .disable()
                .map_err(|e| AutostartError::Update(e.to_string()))?;
            info!("autostart disabled");
        }
        Ok(())
    }

    pub fn sync_with_settings<R: Runtime>(
        app: &AppHandle<R>,
        autostart_enabled: bool,
    ) -> Result<AutostartState, AutostartError> {
        let current = Self::is_enabled(app)?;
        if current != autostart_enabled {
            Self::set_enabled(app, autostart_enabled)?;
        }
        Ok(if autostart_enabled {
            AutostartState::Enabled
        } else {
            AutostartState::Disabled
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_arg_constant_matches_cli_contract() {
        assert_eq!(BACKGROUND_LAUNCH_ARG, "--background");
    }
}
