//! Native overlay and dashboard window adapters for the llm_notch Tauri 2 desktop host.
//!
//! # Integration (`lib.rs` wiring)
//!
//! Add the module and initialize windows during Tauri setup:
//!
//! ```ignore
//! mod window;
//!
//! use window::{WindowCoordinator, OverlayMode};
//!
//! tauri::Builder::default()
//!     .setup(|app| {
//!         let handle = app.handle().clone();
//!         let mut coordinator = WindowCoordinator::new(handle);
//!
//!         // After config windows exist (`overlay`, `dashboard` labels from tauri.conf.json):
//!         let capability = coordinator.setup_overlay()?;
//!         tracing::info!(?capability, "overlay native capability");
//!
//!         // Example: switch to peek on hover / shortcut
//!         // coordinator.set_overlay_mode(OverlayMode::Peek)?;
//!
//!         // Example: open dashboard from tray / shortcut
//!         // coordinator.open_dashboard()?;
//!
//!         app.manage(coordinator);
//!         Ok(())
//!     })
//! ```
//!
//! # Window labels
//!
//! | Label       | Role      | Logical size        | Notes                                      |
//! |-------------|-----------|---------------------|--------------------------------------------|
//! | `overlay`   | HUD strip | 360×44 / 400×240    | Transparent, topmost, non-activating       |
//! | `dashboard` | App UI    | 900×640 (min 720×520) | Normal focusable window                    |
//!
//! # AppKit behavior
//!
//! `NSPanel`, `NSScreen`, and `NSRunningApplication` APIs are compiled in.
//! Native safe-area and activation-policy support are used. Tauri constructs
//! the webview as `NSWindow`, so true construction-time `NSPanel` subclassing
//! remains an explicitly reported fallback.
//!
//! # Platform notes
//!
//! - **macOS**: AppKit configuration runs on the main thread via [`WebviewWindow::run_on_main_thread`].
//!   Accessory activation policy is applied when safe; restored when the dashboard opens.
//! - **Windows**: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, `WS_EX_APPWINDOW` cleared,
//!   `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`.

use std::sync::Arc;

use parking_lot::Mutex;

pub mod coordinator;
pub mod error;
pub mod geometry;
pub mod types;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod stub;
#[cfg(target_os = "windows")]
pub mod windows;

pub use coordinator::WindowCoordinator;
pub use types::OverlayMode;

pub type SharedWindowCoordinator = Arc<Mutex<WindowCoordinator>>;

#[cfg(test)]
mod spec_tests {
    use super::types::OverlayMode;

    #[test]
    fn overlay_spec_sizes_are_stable() {
        assert_eq!(OverlayMode::COMPACT_LOGICAL.width, 360.0);
        assert_eq!(OverlayMode::COMPACT_LOGICAL.height, 44.0);
        assert_eq!(OverlayMode::PEEK_LOGICAL.width, 400.0);
        assert_eq!(OverlayMode::PEEK_LOGICAL.height, 240.0);
    }
}
