use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use interprocess::local_socket::{ListenerOptions, Name};
use nix::unistd::Uid;

use crate::error::{IpcError, IpcResult};
use crate::limits::SOCKET_FILENAME;

pub fn socket_filename() -> &'static str {
    SOCKET_FILENAME
}

pub fn ensure_runtime_dir(path: &Path) -> IpcResult<()> {
    std::fs::create_dir_all(path).map_err(IpcError::Io)?;
    set_mode(path, 0o700)
}

pub fn harden_file(path: &Path) -> IpcResult<()> {
    set_mode(path, 0o600)
}

fn set_mode(path: &Path, mode: u32) -> IpcResult<()> {
    let mut perms = std::fs::metadata(path).map_err(IpcError::Io)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms).map_err(IpcError::Io)?;
    Ok(())
}

pub fn build_listener_options(name: Name<'_>) -> IpcResult<ListenerOptions<'_>> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use interprocess::os::unix::local_socket::ListenerOptionsExt;
        return Ok(ListenerOptions::new().name(name).mode(0o600));
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Ok(ListenerOptions::new().name(name))
    }
}

pub fn post_bind_harden(socket_path: &Path) -> IpcResult<()> {
    if socket_path.exists() {
        harden_file(socket_path)?;
    }
    Ok(())
}

pub fn current_uid() -> u32 {
    Uid::current().as_raw()
}
