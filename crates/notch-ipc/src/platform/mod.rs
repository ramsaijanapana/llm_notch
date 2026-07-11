//! Platform-specific runtime paths and permission hardening.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(unix, windows)))]
mod fallback {
    use std::path::Path;

    use crate::error::{IpcError, IpcResult};

    pub fn socket_filename() -> &'static str {
        "ingest.sock"
    }

    pub fn ensure_runtime_dir(path: &Path) -> IpcResult<()> {
        std::fs::create_dir_all(path).map_err(IpcError::Io)
    }

    pub fn harden_file(_path: &Path) -> IpcResult<()> {
        Ok(())
    }

    pub fn build_listener_options(
        name: interprocess::local_socket::Name<'_>,
    ) -> interprocess::local_socket::ListenerOptions<'_> {
        interprocess::local_socket::ListenerOptions::new().name(name)
    }
}

#[cfg(not(any(unix, windows)))]
pub use fallback::*;
