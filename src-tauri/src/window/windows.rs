//! Windows overlay and dashboard native window configuration via Win32.

use crate::window::error::{WindowError, WindowResult};
use crate::window::types::{CapabilityStatus, NotchInsets, OverlayPlatformCapability};
use tauri::WebviewWindow;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WINDOW_EX_STYLE, WS_EX_APPWINDOW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

const OVERLAY_LABEL: &str = "overlay";

/// Configure the overlay HWND for non-activating, topmost, taskbar-excluded behavior.
pub fn configure_overlay(
    window: &WebviewWindow,
    _show_over_fullscreen: bool,
) -> WindowResult<OverlayPlatformCapability> {
    if window.label() != OVERLAY_LABEL {
        return Err(WindowError::WindowNotFound(OVERLAY_LABEL));
    }

    let hwnd = hwnd_from_tauri(window)?;
    apply_overlay_styles(hwnd)?;

    Ok(OverlayPlatformCapability {
        non_activating: CapabilityStatus::Supported,
        topmost: CapabilityStatus::Supported,
        all_spaces: CapabilityStatus::Supported,
        taskbar_excluded: CapabilityStatus::Supported,
        notch_insets: CapabilityStatus::Partial {
            fallback: "notch insets supplied via geometry DisplaySnapshot on Windows",
        },
        activation_policy: CapabilityStatus::Supported,
    })
}

/// Reapply overlay native flags after mode or visibility changes.
pub fn reapply_overlay(window: &WebviewWindow, show_over_fullscreen: bool) -> WindowResult<()> {
    let _ = configure_overlay(window, show_over_fullscreen)?;
    Ok(())
}

/// Configure the dashboard window (no special non-activating styles).
pub fn configure_dashboard(_window: &WebviewWindow) -> WindowResult<()> {
    Ok(())
}

/// Hook invoked when the dashboard hides.
pub fn on_dashboard_hidden(
    _app: &tauri::AppHandle,
    _show_over_fullscreen: bool,
) -> WindowResult<()> {
    Ok(())
}

pub fn notch_insets(_window: &WebviewWindow) -> Option<NotchInsets> {
    None
}

/// Hook point for WM_MOUSEACTIVATE no-activate handling.
///
/// With `WS_EX_NOACTIVATE` applied, mouse clicks should not activate the overlay. A dedicated
/// window procedure hook can return `MA_NOACTIVATE` for defense in depth when added later.
pub fn overlay_mouse_activate_policy() -> CapabilityStatus {
    CapabilityStatus::Partial {
        fallback: "WS_EX_NOACTIVATE set; WM_MOUSEACTIVATE MA_NOACTIVATE hook not yet installed",
    }
}

fn hwnd_from_tauri(window: &WebviewWindow) -> WindowResult<HWND> {
    window
        .hwnd()
        .map_err(|err| WindowError::Platform(Box::leak(err.to_string().into_boxed_str())))
}

fn apply_overlay_styles(hwnd: HWND) -> WindowResult<()> {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut next = WINDOW_EX_STYLE(current);
        next |= WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
        next &= !WS_EX_APPWINDOW;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next.0 as isize);

        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .map_err(|_| WindowError::Platform("SetWindowPos TOPMOST failed"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_activate_policy_documents_partial_hook_status() {
        let status = overlay_mouse_activate_policy();
        assert!(matches!(status, CapabilityStatus::Partial { .. }));
    }
}
