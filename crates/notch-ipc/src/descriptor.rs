//! Runtime descriptor persisted with user-only permissions.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::auth::AuthToken;
use crate::error::{IpcError, IpcResult};
use crate::limits::{DESCRIPTOR_FILENAME, IPC_WIRE_VERSION};

/// Filesystem locations and auth material for a running ingest server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeDescriptor {
    pub v: u16,
    pub socket_path: String,
    pub token_b64: String,
    pub host_pid: u32,
    pub started_at_ms: i64,
}

impl RuntimeDescriptor {
    pub fn new(connect_path: impl Into<String>, token: &AuthToken, started_at_ms: i64) -> Self {
        Self {
            v: IPC_WIRE_VERSION,
            socket_path: connect_path.into(),
            token_b64: token.encode_b64(),
            host_pid: std::process::id(),
            started_at_ms,
        }
    }

    pub fn token(&self) -> IpcResult<AuthToken> {
        AuthToken::decode_b64(&self.token_b64)
    }

    pub fn write_to(&self, path: &Path) -> IpcResult<()> {
        if let Some(parent) = path.parent() {
            crate::platform::ensure_runtime_dir(parent)?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_vec(self).map_err(|err| IpcError::Other(err.into()))?;
        fs::write(&tmp, json).map_err(IpcError::Io)?;
        crate::platform::harden_file(&tmp)?;
        fs::rename(&tmp, path).map_err(IpcError::Io)?;
        crate::platform::harden_file(path)?;
        Ok(())
    }

    pub fn read_from(path: &Path) -> IpcResult<Self> {
        let bytes = fs::read(path).map_err(IpcError::Io)?;
        let descriptor: Self = serde_json::from_slice(&bytes)
            .map_err(|err| IpcError::FrameRejected(format!("descriptor JSON invalid: {err}")))?;
        if descriptor.v != IPC_WIRE_VERSION {
            return Err(IpcError::FrameRejected(format!(
                "unsupported descriptor version {}",
                descriptor.v
            )));
        }
        Ok(descriptor)
    }
}

pub fn default_runtime_dir() -> IpcResult<PathBuf> {
    let dirs = ProjectDirs::from("com", "llm_notch", "llm_notch")
        .ok_or_else(|| IpcError::InvalidConfig("cannot resolve project directories".into()))?;
    Ok(dirs.data_local_dir().join("runtime"))
}

pub fn descriptor_path_for(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(DESCRIPTOR_FILENAME)
}

pub fn find_descriptor() -> IpcResult<RuntimeDescriptor> {
    let runtime_dir = default_runtime_dir()?;
    let path = descriptor_path_for(&runtime_dir);
    if !path.exists() {
        return Err(IpcError::DescriptorUnavailable);
    }
    RuntimeDescriptor::read_from(&path)
}

pub fn find_descriptor_in(runtime_dir: &Path) -> IpcResult<RuntimeDescriptor> {
    RuntimeDescriptor::read_from(&descriptor_path_for(runtime_dir))
}

pub fn socket_path_for(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(crate::platform::socket_filename())
}

pub fn connect_path_for(runtime_dir: &Path) -> String {
    #[cfg(unix)]
    {
        socket_path_for(runtime_dir).to_string_lossy().into_owned()
    }
    #[cfg(windows)]
    {
        let _ = runtime_dir;
        r"\\.\pipe\llm_notch_ingest".into()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = runtime_dir;
        String::new()
    }
}
