use tauri::{State, WebviewWindow};

use crate::commands::error::CommandError;
use crate::commands::types::RequestedOverlayMode;
use crate::commands::validation::validate_session_id;
use crate::state::HostState;
use crate::window::{OverlayMode, SharedWindowCoordinator};

#[tauri::command]
pub fn set_overlay_mode(
    window: WebviewWindow,
    mode: RequestedOverlayMode,
    windows: State<'_, SharedWindowCoordinator>,
) -> Result<(), CommandError> {
    if !matches!(window.label(), "overlay" | "dashboard") {
        return Err(CommandError::InvalidRequest(
            "overlay mode is limited to native application windows".into(),
        ));
    }
    let native_mode = match mode {
        RequestedOverlayMode::Collapsed => OverlayMode::Compact,
        RequestedOverlayMode::Peek | RequestedOverlayMode::Expanded => OverlayMode::Peek,
    };
    windows
        .lock()
        .set_overlay_mode(native_mode)
        .map_err(|error| CommandError::Internal(error.to_string()))
}

#[tauri::command]
pub fn open_dashboard(
    focus: Option<bool>,
    windows: State<'_, SharedWindowCoordinator>,
) -> Result<(), CommandError> {
    windows
        .lock()
        .open_dashboard(focus.unwrap_or(true))
        .map_err(|error| CommandError::Internal(error.to_string()))
}

#[tauri::command]
pub fn acknowledge_attention(
    session_id: String,
    host: State<'_, std::sync::Arc<HostState>>,
) -> Result<(), CommandError> {
    validate_session_id(&session_id)?;
    host.acknowledge_attention(session_id)
        .map_err(CommandError::Internal)
}
