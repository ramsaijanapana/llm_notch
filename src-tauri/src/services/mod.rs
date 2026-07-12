//! Rust-owned desktop services. Guest webviews never invoke plugin APIs directly.

pub mod alerts;
pub mod autostart;
pub mod global_shortcut;
pub mod remote;
pub mod remote_supervisor;
pub mod sound_theme;
pub mod tray;

pub use alerts::AlertNotifier;

pub use autostart::{AutostartError, AutostartService, BACKGROUND_LAUNCH_ARG};
pub use global_shortcut::{DEFAULT_DASHBOARD_SHORTCUT, GlobalShortcutService, ShortcutError};
pub use tray::{SharedTrayService, TrayMenuAction, TrayMenuModel, TrayService};
