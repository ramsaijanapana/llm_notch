//! Windows overlay and dashboard native window configuration via Win32.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use crate::window::error::{WindowError, WindowResult};
use crate::window::types::{CapabilityStatus, NotchInsets, OverlayPlatformCapability};
use tauri::WebviewWindow;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GWL_EXSTYLE, GWLP_WNDPROC, GetWindowLongPtrW, HWND_TOPMOST, MA_NOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos,
    WINDOW_EX_STYLE, WM_MOUSEACTIVATE, WNDPROC, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

const OVERLAY_LABEL: &str = "overlay";
static OVERLAY_MOUSE_ACTIVATE_HOOK: AtomicBool = AtomicBool::new(false);
static OVERLAY_ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

/// Per-monitor DPI awareness snapshot for validation and smoke tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiAwarenessStatus {
    pub per_monitor_v2: bool,
    pub awareness: DPI_AWARENESS,
}

impl DpiAwarenessStatus {
    #[must_use]
    pub fn acceptable_for_overlay(&self) -> bool {
        self.per_monitor_v2 || self.awareness == DPI_AWARENESS(2)
    }
}

/// Validate that the host process is per-monitor DPI aware (V2 preferred).
pub fn validate_per_monitor_dpi_awareness() -> DpiAwarenessStatus {
    unsafe {
        let context = GetThreadDpiAwarenessContext();
        let per_monitor_v2 =
            AreDpiAwarenessContextsEqual(context, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                .as_bool();
        let awareness = GetAwarenessFromDpiAwarenessContext(context);
        DpiAwarenessStatus {
            per_monitor_v2,
            awareness,
        }
    }
}

/// Configure the overlay HWND for non-activating, topmost, taskbar-excluded behavior.
pub fn configure_overlay(
    window: &WebviewWindow,
    _show_over_fullscreen: bool,
) -> WindowResult<OverlayPlatformCapability> {
    if window.label() != OVERLAY_LABEL {
        return Err(WindowError::WindowNotFound(OVERLAY_LABEL));
    }

    let dpi = validate_per_monitor_dpi_awareness();
    if !dpi.acceptable_for_overlay() {
        tracing::warn!(
            ?dpi,
            "process DPI awareness below per-monitor; overlay geometry may be incorrect on mixed-DPI setups"
        );
    }

    let hwnd = hwnd_from_tauri(window)?;
    apply_overlay_styles(hwnd)?;
    install_mouse_activate_hook(hwnd)?;

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

/// Reports whether the WM_MOUSEACTIVATE / MA_NOACTIVATE subclass is installed.
pub fn overlay_mouse_activate_policy() -> CapabilityStatus {
    if OVERLAY_MOUSE_ACTIVATE_HOOK.load(Ordering::Acquire) {
        CapabilityStatus::Supported
    } else {
        CapabilityStatus::Partial {
            fallback: "WM_MOUSEACTIVATE MA_NOACTIVATE hook not yet installed",
        }
    }
}

/// Expected overlay extended styles after native configuration.
#[must_use]
pub fn expected_overlay_ex_styles() -> WINDOW_EX_STYLE {
    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW
}

/// Read the current overlay extended window styles (for smoke tests).
pub fn overlay_ex_styles(hwnd: HWND) -> WindowResult<WINDOW_EX_STYLE> {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        Ok(WINDOW_EX_STYLE(current))
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

fn install_mouse_activate_hook(hwnd: HWND) -> WindowResult<()> {
    if OVERLAY_MOUSE_ACTIVATE_HOOK.load(Ordering::Acquire) {
        return Ok(());
    }

    unsafe {
        let original = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
        if original == 0 {
            return Err(WindowError::Platform("GWLP_WNDPROC unavailable"));
        }
        if OVERLAY_ORIGINAL_WNDPROC
            .compare_exchange(0, original, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            OVERLAY_MOUSE_ACTIVATE_HOOK.store(true, Ordering::Release);
            return Ok(());
        }
        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, overlay_wndproc as *const () as isize);
    }

    OVERLAY_MOUSE_ACTIVATE_HOOK.store(true, Ordering::Release);
    Ok(())
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    let original = OVERLAY_ORIGINAL_WNDPROC.load(Ordering::Acquire);
    let proc: WNDPROC = unsafe { std::mem::transmute(original) };
    unsafe { CallWindowProcW(proc, hwnd, msg, wparam, lparam) }
}

/// Request per-monitor V2 DPI awareness for the current process (used by smoke tests).
pub fn ensure_process_per_monitor_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_activate_policy_reports_supported_after_hook_flag() {
        OVERLAY_MOUSE_ACTIVATE_HOOK.store(true, Ordering::Release);
        assert_eq!(
            overlay_mouse_activate_policy(),
            CapabilityStatus::Supported
        );
        OVERLAY_MOUSE_ACTIVATE_HOOK.store(false, Ordering::Release);
    }

    #[test]
    fn expected_overlay_styles_include_noactivate_and_toolwindow() {
        let styles = expected_overlay_ex_styles();
        assert!(styles.contains(WS_EX_NOACTIVATE));
        assert!(styles.contains(WS_EX_TOOLWINDOW));
        assert!(!styles.contains(WS_EX_APPWINDOW));
    }

    #[test]
    fn per_monitor_dpi_awareness_is_acceptable_in_test_host() {
        ensure_process_per_monitor_dpi_awareness();
        let status = validate_per_monitor_dpi_awareness();
        assert!(
            status.acceptable_for_overlay(),
            "test host should be per-monitor DPI aware after ensure: {status:?}"
        );
    }

    #[test]
    fn dpi_acceptance_logic_prefers_v2_or_per_monitor() {
        assert!(DpiAwarenessStatus {
            per_monitor_v2: true,
            awareness: DPI_AWARENESS(0),
        }
        .acceptable_for_overlay());
        assert!(DpiAwarenessStatus {
            per_monitor_v2: false,
            awareness: DPI_AWARENESS(2),
        }
        .acceptable_for_overlay());
        assert!(!DpiAwarenessStatus {
            per_monitor_v2: false,
            awareness: DPI_AWARENESS(0),
        }
        .acceptable_for_overlay());
    }
}
