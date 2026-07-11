//! Platform-specific activation backends.

#[cfg(target_os = "macos")]
pub mod macos;
pub mod stub;
#[cfg(target_os = "windows")]
pub mod windows;
