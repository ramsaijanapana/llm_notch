//! Rust-owned desktop services. Guest webviews never invoke plugin APIs directly.

pub mod alerts;
pub mod autostart;
pub mod global_shortcut;
pub mod tray;

pub use alerts::{AlertNotifier, play_alert_sound};

pub use autostart::{AutostartError, AutostartService, BACKGROUND_LAUNCH_ARG};
pub use global_shortcut::{DEFAULT_DASHBOARD_SHORTCUT, GlobalShortcutService, ShortcutError};
pub use tray::{SharedTrayService, TrayMenuAction, TrayMenuModel, TrayService};
