use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::transport::DEFAULT_REMOTE_RUNTIME_DIRECTORY;
use crate::{RemoteHostConfig, RemoteTarget};

const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayArtifact {
    pub local_path: PathBuf,
    pub target: RemoteTarget,
    pub byte_len: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum DeploymentStep {
    ProbeTarget,
    CreatePrivateDirectory {
        remote_directory: String,
    },
    UploadTemporary {
        remote_path: String,
    },
    VerifySha256 {
        expected_sha256: String,
    },
    ActivateAtomically {
        remote_path: String,
    },
    StartStdioRelay {
        remote_path: String,
        event_spool_dir: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPlan {
    pub host_id: String,
    pub artifact: RelayArtifact,
    pub steps: Vec<DeploymentStep>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeploymentError {
    #[error("relay artifact is empty or exceeds the size limit")]
    InvalidSize,
    #[error("relay artifact hash must be lowercase SHA-256")]
    InvalidHash,
    #[error("remote directory is not an allowed private path")]
    InvalidRemoteDirectory,
    #[error("remote host configuration is invalid")]
    InvalidHost,
}

impl RelayArtifact {
    pub fn from_bytes(local_path: PathBuf, target: RemoteTarget, bytes: &[u8]) -> Self {
        Self {
            local_path,
            target,
            byte_len: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(bytes)),
        }
    }

    pub fn validate(&self) -> Result<(), DeploymentError> {
        if self.byte_len == 0 || self.byte_len > MAX_ARTIFACT_BYTES {
            return Err(DeploymentError::InvalidSize);
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(DeploymentError::InvalidHash);
        }
        Ok(())
    }
}

impl DeploymentPlan {
    pub fn new(
        host: &RemoteHostConfig,
        artifact: RelayArtifact,
        remote_directory: &str,
    ) -> Result<Self, DeploymentError> {
        Self::new_with_runtime_dir(
            host,
            artifact,
            remote_directory,
            DEFAULT_REMOTE_RUNTIME_DIRECTORY,
        )
    }

    pub fn new_with_runtime_dir(
        host: &RemoteHostConfig,
        artifact: RelayArtifact,
        remote_directory: &str,
        remote_runtime_directory: &str,
    ) -> Result<Self, DeploymentError> {
        host.validate().map_err(|_| DeploymentError::InvalidHost)?;
        artifact.validate()?;
        if !valid_remote_directory(remote_directory) {
            return Err(DeploymentError::InvalidRemoteDirectory);
        }
        if !valid_remote_directory(remote_runtime_directory) {
            return Err(DeploymentError::InvalidRemoteDirectory);
        }
        let temporary = format!("{remote_directory}/llm-notch-relay.tmp");
        let active = format!("{remote_directory}/llm-notch-relay");
        Ok(Self {
            host_id: host.id.clone(),
            artifact: artifact.clone(),
            steps: vec![
                DeploymentStep::ProbeTarget,
                DeploymentStep::CreatePrivateDirectory {
                    remote_directory: remote_directory.into(),
                },
                DeploymentStep::UploadTemporary {
                    remote_path: temporary,
                },
                DeploymentStep::VerifySha256 {
                    expected_sha256: artifact.sha256,
                },
                DeploymentStep::ActivateAtomically {
                    remote_path: active.clone(),
                },
                DeploymentStep::StartStdioRelay {
                    remote_path: active,
                    event_spool_dir: remote_runtime_directory.into(),
                },
            ],
        })
    }
}

/// Honest operator note for remote agent hook installs that pair with relay `--event-spool`.
pub fn remote_hook_spool_guidance(event_spool_dir: &str) -> String {
    format!(
        "Configure remote agent hooks to spool events into {event_spool_dir} (same directory relay start passes to --event-spool). \
         Prefix hook commands with LLM_NOTCH_EVENT_SPOOL=1 or pass --spool-dir {event_spool_dir} on llm-notch-hook. \
         See integrations/remote/ for reviewed examples. Local desktop hooks keep default IPC when LLM_NOTCH_EVENT_SPOOL is unset."
    )
}

fn valid_remote_directory(value: &str) -> bool {
    value.starts_with("~/.")
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'~' | b'/' | b'.' | b'-' | b'_')
        })
        && !value.contains("..")
}

#[cfg(test)]
mod tests {
    use crate::config::{RemoteArchitecture, RemoteOs, SshHostKeyPolicy};
    use crate::transport::{DEFAULT_REMOTE_BIN_DIRECTORY, DEFAULT_REMOTE_RUNTIME_DIRECTORY};

    use super::*;

    #[test]
    fn remote_hook_spool_guidance_mentions_runtime_dir_and_local_ipc_default() {
        let guidance = remote_hook_spool_guidance(DEFAULT_REMOTE_RUNTIME_DIRECTORY);
        assert!(guidance.contains(DEFAULT_REMOTE_RUNTIME_DIRECTORY));
        assert!(guidance.contains("LLM_NOTCH_EVENT_SPOOL=1"));
        assert!(guidance.contains("integrations/remote/"));
        assert!(guidance.contains("unset"));
    }

    #[test]
    fn plan_verifies_before_atomic_activation() {
        let host = RemoteHostConfig {
            id: "remote-1".into(),
            destination: "remote.example".into(),
            port: None,
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        };
        let artifact = RelayArtifact::from_bytes(
            "relay".into(),
            RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            },
            b"relay-binary",
        );
        let plan = DeploymentPlan::new(&host, artifact, DEFAULT_REMOTE_BIN_DIRECTORY).unwrap();
        let verify = plan
            .steps
            .iter()
            .position(|step| matches!(step, DeploymentStep::VerifySha256 { .. }))
            .unwrap();
        let activate = plan
            .steps
            .iter()
            .position(|step| matches!(step, DeploymentStep::ActivateAtomically { .. }))
            .unwrap();
        assert!(verify < activate);
        assert!(matches!(
            plan.steps.last(),
            Some(DeploymentStep::StartStdioRelay {
                event_spool_dir,
                ..
            }) if event_spool_dir == DEFAULT_REMOTE_RUNTIME_DIRECTORY
        ));
    }
}
