//! Window coordinator and platform adapter errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("window `{0}` not found")]
    WindowNotFound(&'static str),

    #[error("no display monitor available")]
    NoMonitor,

    #[error("monitor `{0}` not found")]
    MonitorNotFound(String),

    #[error("platform adapter error: {0}")]
    Platform(&'static str),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

pub type WindowResult<T> = Result<T, WindowError>;
