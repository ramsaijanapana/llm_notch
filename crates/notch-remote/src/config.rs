use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_HOST_ID_LEN: usize = 64;
const MAX_DESTINATION_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum SshHostKeyPolicy {
    /// Use OpenSSH's known_hosts verification. This is the production default.
    Strict,
    /// Accept a new key once, while still rejecting changed keys.
    AcceptNew,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteHostConfig {
    pub id: String,
    pub destination: String,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    pub known_hosts_file: Option<PathBuf>,
    pub host_key_policy: SshHostKeyPolicy,
    pub connect_timeout_seconds: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteTarget {
    pub os: RemoteOs,
    pub architecture: RemoteArchitecture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteOs {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteArchitecture {
    X86_64,
    Aarch64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("remote host id is invalid")]
    InvalidId,
    #[error("SSH destination is invalid")]
    InvalidDestination,
    #[error("SSH port must not be zero")]
    InvalidPort,
    #[error("connect timeout must be between 1 and 120 seconds")]
    InvalidTimeout,
    #[error("path must be absolute")]
    PathNotAbsolute,
}

impl RemoteHostConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.id.is_empty()
            || self.id.len() > MAX_HOST_ID_LEN
            || !self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(ConfigError::InvalidId);
        }

        if !valid_destination(&self.destination) {
            return Err(ConfigError::InvalidDestination);
        }
        if self.port == Some(0) {
            return Err(ConfigError::InvalidPort);
        }
        if !(1..=120).contains(&self.connect_timeout_seconds) {
            return Err(ConfigError::InvalidTimeout);
        }
        for path in [&self.identity_file, &self.known_hosts_file]
            .into_iter()
            .flatten()
        {
            validate_absolute(path)?;
        }
        Ok(())
    }

    /// Builds OpenSSH arguments without invoking a shell or embedding credentials.
    pub fn ssh_args(&self) -> Result<Vec<String>, ConfigError> {
        self.validate()?;
        let mut args = self.ssh_base_args()?;
        args.push(self.destination.clone());
        Ok(args)
    }

    /// Builds shared OpenSSH/scp options without the destination suffix.
    pub fn scp_base_args(&self) -> Result<Vec<String>, ConfigError> {
        self.validate()?;
        let mut args = self.ssh_common_args()?;
        if let Some(port) = self.port {
            args.extend(["-P".into(), port.to_string()]);
        }
        Ok(args)
    }

    fn ssh_base_args(&self) -> Result<Vec<String>, ConfigError> {
        let mut args = self.ssh_common_args()?;
        if let Some(port) = self.port {
            args.extend(["-p".into(), port.to_string()]);
        }
        Ok(args)
    }

    fn ssh_common_args(&self) -> Result<Vec<String>, ConfigError> {
        let mut args = vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            format!("ConnectTimeout={}", self.connect_timeout_seconds),
            "-o".into(),
            format!(
                "StrictHostKeyChecking={}",
                match self.host_key_policy {
                    SshHostKeyPolicy::Strict => "yes",
                    SshHostKeyPolicy::AcceptNew => "accept-new",
                }
            ),
        ];
        if let Some(identity) = &self.identity_file {
            args.extend(["-i".into(), identity.display().to_string()]);
        }
        if let Some(known_hosts) = &self.known_hosts_file {
            args.extend([
                "-o".into(),
                format!("UserKnownHostsFile={}", known_hosts.display()),
            ]);
        }
        Ok(args)
    }
}

fn validate_absolute(path: &Path) -> Result<(), ConfigError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(ConfigError::PathNotAbsolute)
    }
}

fn valid_destination(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DESTINATION_LEN
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'.' | b'-' | b'_' | b'@' | b'[' | b']' | b':' | b'%')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> RemoteHostConfig {
        RemoteHostConfig {
            id: "build-server".into(),
            destination: "dev@example.internal".into(),
            port: Some(2222),
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        }
    }

    #[test]
    fn arguments_are_structured_and_strict_by_default() {
        let args = config().ssh_args().unwrap();
        assert!(args.contains(&"StrictHostKeyChecking=yes".into()));
        assert_eq!(args.last().unwrap(), "dev@example.internal");
    }

    #[test]
    fn destination_rejects_shell_metacharacters_and_options() {
        for destination in ["-proxy", "host;rm", "host name", "host$(bad)"] {
            let mut value = config();
            value.destination = destination.into();
            assert_eq!(value.validate(), Err(ConfigError::InvalidDestination));
        }
    }
}
