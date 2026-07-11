//! No-op platform adapters for targets without native overlay hooks.

use crate::window::error::WindowResult;
use crate::window::types::{NotchInsets, OverlayPlatformCapability};
use tauri::WebviewWindow;

pub fn configure_overlay(
    _window: &WebviewWindow,
    _show_over_fullscreen: bool,
) -> WindowResult<OverlayPlatformCapability> {
    Ok(OverlayPlatformCapability::stub())
}

pub fn reapply_overlay(_window: &WebviewWindow, _show_over_fullscreen: bool) -> WindowResult<()> {
    Ok(())
}

pub fn configure_dashboard(_window: &WebviewWindow) -> WindowResult<()> {
    Ok(())
}

pub fn on_dashboard_hidden(
    _app: &tauri::AppHandle,
    _show_over_fullscreen: bool,
) -> WindowResult<()> {
    Ok(())
}

pub fn notch_insets(_window: &WebviewWindow) -> Option<NotchInsets> {
    None
}
