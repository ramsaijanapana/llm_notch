use std::path::Path;

use interprocess::local_socket::{ListenerOptions, Name};

use crate::error::{IpcError, IpcResult};
use crate::limits::PIPE_FILENAME;

pub fn socket_filename() -> &'static str {
    PIPE_FILENAME
}

pub fn ensure_runtime_dir(path: &Path) -> IpcResult<()> {
    std::fs::create_dir_all(path).map_err(IpcError::Io)
}

pub fn harden_file(_path: &Path) -> IpcResult<()> {
    Ok(())
}

pub fn build_listener_options(name: Name<'_>) -> IpcResult<ListenerOptions<'_>> {
    use interprocess::os::windows::local_socket::ListenerOptionsExt;
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;

    let sd = SecurityDescriptor::new().map_err(IpcError::Io)?;
    Ok(ListenerOptions::new().name(name).security_descriptor(sd))
}

pub fn post_bind_harden(_socket_path: &Path) -> IpcResult<()> {
    Ok(())
}

pub fn current_uid() -> u32 {
    0
}
