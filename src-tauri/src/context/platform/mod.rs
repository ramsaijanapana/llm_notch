//! Platform-specific activation backends.

pub mod bridge;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod stub;
#[cfg(target_os = "windows")]
pub mod windows;
