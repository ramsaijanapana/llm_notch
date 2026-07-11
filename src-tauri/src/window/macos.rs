//! macOS overlay and dashboard configuration via AppKit.
//!
//! # NSPanel vs NSWindow (honest capability report)
//!
//! The strongest non-activating overlay path on macOS is a true [`NSPanel`] created at
//! construction time with `NSWindowStyleMask::NonactivatingPanel`, `becomesKeyOnlyIfNeeded`,
//! and `NSFloatingWindowLevel`. Tauri 2 constructs webview hosts as [`NSWindow`] instances;
//! we can apply panel-like style masks and collection behaviors post-creation, but cannot
//! reliably reclass an existing `NSWindow` into a floating `NSPanel` without private
//! subclassing that Tauri does not expose.
//!
//! [`NSWindowCollectionBehavior::FullScreenAuxiliary`] is requested when
//! `show_over_fullscreen` is enabled. It improves visibility beside some fullscreen apps but
//! is **not guaranteed** across Spaces, Stage Manager, or every fullscreen host. Do not claim
//! guaranteed fullscreen overlay coverage in product copy.

use crate::window::error::{WindowError, WindowResult};
use crate::window::types::{CapabilityStatus, NotchInsets, OverlayPlatformCapability};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApp, NSApplicationActivationPolicy, NSFloatingWindowLevel, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use tauri::{Manager, WebviewWindow};

const OVERLAY_LABEL: &str = "overlay";

/// Schedule AppKit overlay configuration on the UI thread.
pub fn configure_overlay(
    window: &WebviewWindow,
    show_over_fullscreen: bool,
) -> WindowResult<OverlayPlatformCapability> {
    if window.label() != OVERLAY_LABEL {
        return Err(WindowError::WindowNotFound(OVERLAY_LABEL));
    }
    schedule_overlay_styles(window, show_over_fullscreen)?;
    Ok(build_overlay_capability())
}

pub fn reapply_overlay(window: &WebviewWindow, show_over_fullscreen: bool) -> WindowResult<()> {
    schedule_overlay_styles(window, show_over_fullscreen)
}

pub fn configure_dashboard(window: &WebviewWindow) -> WindowResult<()> {
    if window.label() != "dashboard" {
        return Err(WindowError::WindowNotFound("dashboard"));
    }
    window
        .run_on_main_thread(move || set_activation_policy(NSApplicationActivationPolicy::Regular))
        .map_err(WindowError::Tauri)
}

pub fn on_dashboard_hidden(app: &tauri::AppHandle, show_over_fullscreen: bool) -> WindowResult<()> {
    let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) else {
        return Ok(());
    };
    reapply_overlay(&overlay, show_over_fullscreen)
}

/// Best-effort native safe-area insets for the screen containing the overlay.
///
/// AppKit only allows this query on the main thread. Callers retain the
/// monitor work-area fallback when invoked from another thread.
pub fn notch_insets(window: &WebviewWindow) -> Option<NotchInsets> {
    let _mtm = MainThreadMarker::new()?;
    let raw = window.ns_window().ok()?;
    // SAFETY: Tauri owns this NSWindow for at least the lifetime of `window`,
    // and MainThreadMarker proves this query is executing on the AppKit thread.
    let ns_window: &NSWindow = unsafe { &*raw.cast() };
    let screen = ns_window.screen()?;
    let insets = screen.safeAreaInsets();
    let scale = window.scale_factor().ok().unwrap_or(1.0);
    Some(NotchInsets {
        top: (insets.top * scale).round() as i32,
        right: (insets.right * scale).round() as i32,
        bottom: (insets.bottom * scale).round() as i32,
        left: (insets.left * scale).round() as i32,
    })
}

fn schedule_overlay_styles(window: &WebviewWindow, show_over_fullscreen: bool) -> WindowResult<()> {
    let window = window.clone();
    let scheduled_window = window.clone();
    window
        .run_on_main_thread(move || {
            if let Err(error) = apply_overlay_styles(&scheduled_window, show_over_fullscreen) {
                tracing::warn!(%error, "AppKit overlay configuration failed");
            }
        })
        .map_err(WindowError::Tauri)
}

fn apply_overlay_styles(window: &WebviewWindow, show_over_fullscreen: bool) -> WindowResult<()> {
    let raw = window
        .ns_window()
        .map_err(|_| WindowError::Platform("failed to resolve NSWindow handle"))?;
    // SAFETY: this function is only scheduled on the main thread and Tauri
    // provides a valid NSWindow pointer while the WebviewWindow is alive.
    let ns_window: &NSWindow = unsafe { &*raw.cast() };

    let style = NSWindowStyleMask::Borderless
        | NSWindowStyleMask::NonactivatingPanel
        | NSWindowStyleMask::UtilityWindow;
    ns_window.setStyleMask(style);
    ns_window.setLevel(NSFloatingWindowLevel);
    ns_window.setHidesOnDeactivate(false);
    ns_window.setIgnoresMouseEvents(false);
    let behavior = collection_behavior(show_over_fullscreen);
    ns_window.setCollectionBehavior(behavior);

    let dashboard_visible = window
        .app_handle()
        .get_webview_window("dashboard")
        .and_then(|dashboard| dashboard.is_visible().ok())
        .unwrap_or(false);
    if !dashboard_visible {
        set_activation_policy(NSApplicationActivationPolicy::Accessory);
    }
    Ok(())
}

fn collection_behavior(show_over_fullscreen: bool) -> NSWindowCollectionBehavior {
    let mut behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::IgnoresCycle
        | NSWindowCollectionBehavior::Stationary;
    if show_over_fullscreen {
        behavior |= NSWindowCollectionBehavior::FullScreenAuxiliary;
    }
    behavior
}

fn set_activation_policy(policy: NSApplicationActivationPolicy) {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("activation policy update requested off the AppKit thread");
        return;
    };
    if !NSApp(mtm).setActivationPolicy(policy) {
        tracing::warn!(?policy, "AppKit rejected activation policy update");
    }
}

fn build_overlay_capability() -> OverlayPlatformCapability {
    OverlayPlatformCapability {
        non_activating: CapabilityStatus::Partial {
            fallback: "Tauri NSWindow with NonactivatingPanel style; true NSPanel requires construction-time subclassing outside Tauri APIs",
        },
        topmost: CapabilityStatus::Supported,
        all_spaces: CapabilityStatus::Supported,
        taskbar_excluded: CapabilityStatus::Supported,
        notch_insets: CapabilityStatus::Supported,
        activation_policy: CapabilityStatus::Partial {
            fallback: "Accessory policy is requested when dashboard is hidden; AppKit may reject dynamic changes outside a bundled .app",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_fallback_is_reported_honestly() {
        let capability = build_overlay_capability();
        assert!(matches!(
            capability.non_activating,
            CapabilityStatus::Partial { .. }
        ));
        assert_eq!(capability.notch_insets, CapabilityStatus::Supported);
        assert!(matches!(
            capability.activation_policy,
            CapabilityStatus::Partial { .. }
        ));
    }

    #[test]
    fn fullscreen_auxiliary_follows_setting() {
        assert!(
            collection_behavior(true).contains(NSWindowCollectionBehavior::FullScreenAuxiliary)
        );
        assert!(
            !collection_behavior(false).contains(NSWindowCollectionBehavior::FullScreenAuxiliary)
        );
    }
}
