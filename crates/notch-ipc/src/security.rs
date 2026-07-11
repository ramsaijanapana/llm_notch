//! Security capability reporting and peer verification helpers.

use crate::error::{IpcError, IpcResult};

/// Whether same-user peer verification is available and enforced on this platform/build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerCheckCapability {
    /// Effective UID (Unix) verified against the host process.
    Enforced,
    /// Windows: security descriptor restricts pipe access; PID available but subject to reuse races.
    ProcessIdOnly,
    /// Peer credentials unavailable; rely on token auth and filesystem permissions only.
    Unavailable,
}

/// Security features active for a running ingest server instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityCapabilities {
    pub token_auth: bool,
    pub runtime_dir_user_only: bool,
    pub descriptor_user_only: bool,
    pub socket_user_only: bool,
    pub peer_check: PeerCheckCapability,
    pub remote_clients_rejected: bool,
}

impl SecurityCapabilities {
    pub fn platform_default() -> Self {
        #[cfg(unix)]
        {
            Self {
                token_auth: true,
                runtime_dir_user_only: true,
                descriptor_user_only: true,
                socket_user_only: true,
                peer_check: PeerCheckCapability::Enforced,
                remote_clients_rejected: true,
            }
        }
        #[cfg(windows)]
        {
            Self {
                token_auth: true,
                // The data-local directory inherits the user's ACL. This build
                // does not yet verify or replace that filesystem ACL.
                runtime_dir_user_only: false,
                descriptor_user_only: false,
                socket_user_only: true,
                peer_check: PeerCheckCapability::ProcessIdOnly,
                remote_clients_rejected: true,
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            Self {
                token_auth: true,
                runtime_dir_user_only: false,
                descriptor_user_only: false,
                socket_user_only: false,
                peer_check: PeerCheckCapability::Unavailable,
                remote_clients_rejected: false,
            }
        }
    }
}

/// Verify that the connected peer belongs to the same OS user when credentials are available.
pub fn verify_same_user_peer(
    creds: &interprocess::local_socket::PeerCreds,
    capabilities: &SecurityCapabilities,
) -> IpcResult<()> {
    match capabilities.peer_check {
        PeerCheckCapability::Enforced => {
            #[cfg(unix)]
            {
                use nix::unistd::User;
                let host_uid = User::from_uid(nix::unistd::Uid::current())
                    .map_err(|err| IpcError::Io(err.into()))?
                    .ok_or_else(|| IpcError::PeerRejected("host user unavailable".into()))?
                    .uid
                    .as_raw();
                let peer_uid = creds
                    .euid()
                    .ok_or_else(|| IpcError::PeerRejected("peer euid unavailable".into()))?;
                if peer_uid != host_uid {
                    return Err(IpcError::PeerRejected(format!(
                        "peer uid {peer_uid} != host uid {host_uid}"
                    )));
                }
                Ok(())
            }
            #[cfg(not(unix))]
            {
                let _ = creds;
                Err(IpcError::PeerRejected(
                    "same-user peer check unavailable on this platform".into(),
                ))
            }
        }
        PeerCheckCapability::ProcessIdOnly => {
            // PID is observable for diagnostics; token auth remains the primary gate on Windows.
            let _ = creds;
            Ok(())
        }
        PeerCheckCapability::Unavailable => {
            let _ = creds;
            Ok(())
        }
    }
}
