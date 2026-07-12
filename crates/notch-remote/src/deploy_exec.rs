use std::path::Path;
use std::process::Command;

use thiserror::Error;

use crate::{
    DeploymentPlan, DeploymentStep, RemoteArchitecture, RemoteHostConfig, RemoteOs, RemoteTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentOutcome {
    pub completed_steps: Vec<DeploymentStep>,
    pub probed_target: Option<RemoteTarget>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DeployTransportError {
    #[error("SSH executable is unavailable")]
    SshUnavailable,
    #[error("SCP executable is unavailable")]
    ScpUnavailable,
    #[error("SSH host verification failed")]
    HostVerificationFailed,
    #[error("SSH authentication failed")]
    AuthenticationFailed,
    #[error("remote deploy command failed: {0}")]
    Process(String),
    #[error("remote deploy protocol failed: {0}")]
    Protocol(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeployExecError {
    #[error("deployment step failed: {step:?}: {source}")]
    StepFailed {
        step: DeploymentStep,
        source: DeployTransportError,
    },
    #[error("relay artifact hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("probed remote target {probed:?} does not match deployment artifact target {artifact:?}")]
    TargetMismatch {
        probed: RemoteTarget,
        artifact: RemoteTarget,
    },
    #[error("remote relay path is missing from deployment plan")]
    MissingRelayPath,
}

pub trait DeployTransport {
    fn probe_target(&self, host: &RemoteHostConfig) -> Result<RemoteTarget, DeployTransportError>;
    fn run_remote(
        &self,
        host: &RemoteHostConfig,
        script: &str,
    ) -> Result<String, DeployTransportError>;
    fn upload_file(
        &self,
        host: &RemoteHostConfig,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<(), DeployTransportError>;
}

#[derive(Debug, Clone)]
pub struct OpenSshDeployTransport {
    ssh_executable: String,
    scp_executable: String,
}

impl OpenSshDeployTransport {
    pub fn new(ssh_executable: impl Into<String>, scp_executable: impl Into<String>) -> Self {
        Self {
            ssh_executable: ssh_executable.into(),
            scp_executable: scp_executable.into(),
        }
    }
}

impl DeployTransport for OpenSshDeployTransport {
    fn probe_target(&self, host: &RemoteHostConfig) -> Result<RemoteTarget, DeployTransportError> {
        let output = self.run_remote(host, "uname -s && uname -m")?;
        parse_probe_output(&output)
    }

    fn run_remote(
        &self,
        host: &RemoteHostConfig,
        script: &str,
    ) -> Result<String, DeployTransportError> {
        let mut args = host
            .ssh_args()
            .map_err(|error| DeployTransportError::Protocol(error.to_string()))?;
        args.push(script.to_string());
        run_command(&self.ssh_executable, &args, DeployTransportError::SshUnavailable)
    }

    fn upload_file(
        &self,
        host: &RemoteHostConfig,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<(), DeployTransportError> {
        if !local_path.is_file() {
            return Err(DeployTransportError::Protocol(format!(
                "local relay artifact is missing: {}",
                local_path.display()
            )));
        }
        let mut args = host
            .scp_base_args()
            .map_err(|error| DeployTransportError::Protocol(error.to_string()))?;
        args.push(local_path.display().to_string());
        args.push(format!("{}:{remote_path}", host.destination));
        run_command(&self.scp_executable, &args, DeployTransportError::ScpUnavailable)?;
        Ok(())
    }
}

pub struct DeploymentExecutor<'a, T: DeployTransport + ?Sized> {
    transport: &'a T,
}

impl<'a, T: DeployTransport + ?Sized> DeploymentExecutor<'a, T> {
    pub fn new(transport: &'a T) -> Self {
        Self { transport }
    }

    pub fn execute(
        &self,
        host: &RemoteHostConfig,
        plan: &DeploymentPlan,
    ) -> Result<DeploymentOutcome, DeployExecError> {
        let mut completed = Vec::new();
        let mut probed_target = None;
        let mut temporary_path = None;

        for step in &plan.steps {
            match step {
                DeploymentStep::ProbeTarget => {
                    let target = self
                        .transport
                        .probe_target(host)
                        .map_err(|source| DeployExecError::StepFailed {
                            step: step.clone(),
                            source,
                        })?;
                    if target != plan.artifact.target {
                        return Err(DeployExecError::TargetMismatch {
                            probed: target,
                            artifact: plan.artifact.target,
                        });
                    }
                    probed_target = Some(target);
                    completed.push(step.clone());
                }
                DeploymentStep::CreatePrivateDirectory { remote_directory } => {
                    self.transport
                        .run_remote(host, &format!("mkdir -p '{remote_directory}'"))
                        .map_err(|source| DeployExecError::StepFailed {
                            step: step.clone(),
                            source,
                        })?;
                    completed.push(step.clone());
                }
                DeploymentStep::UploadTemporary { remote_path } => {
                    self.transport
                        .upload_file(host, &plan.artifact.local_path, remote_path)
                        .map_err(|source| DeployExecError::StepFailed {
                            step: step.clone(),
                            source,
                        })?;
                    temporary_path = Some(remote_path.clone());
                    completed.push(step.clone());
                }
                DeploymentStep::VerifySha256 { expected_sha256 } => {
                    let remote_path = temporary_path.as_deref().ok_or(DeployExecError::MissingRelayPath)?;
                    let actual = remote_sha256(self.transport, host, remote_path).map_err(
                        |source| DeployExecError::StepFailed {
                            step: step.clone(),
                            source,
                        },
                    )?;
                    if actual != *expected_sha256 {
                        return Err(DeployExecError::HashMismatch {
                            expected: expected_sha256.clone(),
                            actual,
                        });
                    }
                    completed.push(step.clone());
                }
                DeploymentStep::ActivateAtomically { remote_path } => {
                    let temporary = temporary_path.as_deref().ok_or(DeployExecError::MissingRelayPath)?;
                    activate_relay(self.transport, host, temporary, remote_path).map_err(
                        |source| DeployExecError::StepFailed {
                            step: step.clone(),
                            source,
                        },
                    )?;
                    completed.push(step.clone());
                }
                DeploymentStep::StartStdioRelay { .. } => break,
            }
        }

        Ok(DeploymentOutcome {
            completed_steps: completed,
            probed_target,
        })
    }
}

fn remote_sha256<T: DeployTransport + ?Sized>(
    transport: &T,
    host: &RemoteHostConfig,
    remote_path: &str,
) -> Result<String, DeployTransportError> {
    let script = format!(
        "if command -v sha256sum >/dev/null 2>&1; then sha256sum '{remote_path}'; elif command -v shasum >/dev/null 2>&1; then shasum -a 256 '{remote_path}'; else exit 127; fi | awk '{{print $1}}'"
    );
    let output = transport.run_remote(host, &script)?;
    let hash = output.trim().to_ascii_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DeployTransportError::Protocol(format!(
            "remote hash output was invalid: {output}"
        )));
    }
    Ok(hash)
}

fn activate_relay<T: DeployTransport + ?Sized>(
    transport: &T,
    host: &RemoteHostConfig,
    temporary_path: &str,
    active_path: &str,
) -> Result<(), DeployTransportError> {
    let script = format!(
        "test -f '{temporary_path}' && mv '{temporary_path}' '{active_path}' && chmod +x '{active_path}'"
    );
    transport.run_remote(host, &script).map(|_| ())
}

fn parse_probe_output(output: &str) -> Result<RemoteTarget, DeployTransportError> {
    let mut lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let os = lines
        .next()
        .ok_or_else(|| DeployTransportError::Protocol("probe output missing OS".into()))?;
    let architecture = lines
        .next()
        .ok_or_else(|| DeployTransportError::Protocol("probe output missing architecture".into()))?;
    Ok(RemoteTarget {
        os: match os {
            "Linux" => RemoteOs::Linux,
            "Darwin" => RemoteOs::Macos,
            other => {
                return Err(DeployTransportError::Protocol(format!(
                    "unsupported remote OS: {other}"
                )));
            }
        },
        architecture: match architecture {
            "x86_64" | "amd64" => RemoteArchitecture::X86_64,
            "aarch64" | "arm64" => RemoteArchitecture::Aarch64,
            other => {
                return Err(DeployTransportError::Protocol(format!(
                    "unsupported remote architecture: {other}"
                )));
            }
        },
    })
}

fn run_command(
    executable: &str,
    args: &[String],
    missing_error: DeployTransportError,
) -> Result<String, DeployTransportError> {
    let output = Command::new(executable)
        .args(args)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                missing_error
            } else {
                DeployTransportError::Process(error.to_string())
            }
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    Err(classify_command_failure(&output.stderr, &output.stdout))
}

fn classify_command_failure(stderr: &[u8], stdout: &[u8]) -> DeployTransportError {
    let mut combined = String::from_utf8_lossy(stderr).into_owned();
    if combined.trim().is_empty() {
        combined = String::from_utf8_lossy(stdout).into_owned();
    }
    let normalized = combined.to_ascii_lowercase();
    if normalized.contains("host key verification failed")
        || normalized.contains("remote host identification has changed")
    {
        DeployTransportError::HostVerificationFailed
    } else if normalized.contains("permission denied") {
        DeployTransportError::AuthenticationFailed
    } else {
        DeployTransportError::Process(combined.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::{DeploymentError, RelayArtifact, SshHostKeyPolicy};
    use crate::transport::DEFAULT_REMOTE_BIN_DIRECTORY;

    use super::*;

    fn sample_host() -> RemoteHostConfig {
        RemoteHostConfig {
            id: "dev-box".into(),
            destination: "dev@example.internal".into(),
            port: None,
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        }
    }

    fn sample_plan() -> DeploymentPlan {
        let host = sample_host();
        let artifact = RelayArtifact::from_bytes(
            PathBuf::from("/tmp/relay"),
            RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            },
            b"relay-binary",
        );
        DeploymentPlan::new(&host, artifact, DEFAULT_REMOTE_BIN_DIRECTORY).expect("plan")
    }

    struct FakeDeployTransport {
        probe_result: Result<RemoteTarget, DeployTransportError>,
        remote_outputs: HashMap<String, String>,
        uploads: RefCell<Vec<(PathBuf, String)>>,
        fail_step: Option<&'static str>,
    }

    impl FakeDeployTransport {
        fn success(expected_hash: &str) -> Self {
            let mut remote_outputs = HashMap::new();
            remote_outputs.insert(
                format!("mkdir -p '{DEFAULT_REMOTE_BIN_DIRECTORY}'"),
                String::new(),
            );
            remote_outputs.insert(
                format!(
                    "if command -v sha256sum >/dev/null 2>&1; then sha256sum '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay.tmp'; elif command -v shasum >/dev/null 2>&1; then shasum -a 256 '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay.tmp'; else exit 127; fi | awk '{{print $1}}'"
                ),
                expected_hash.into(),
            );
            remote_outputs.insert(
                format!(
                    "test -f '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay.tmp' && mv '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay.tmp' '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay' && chmod +x '{DEFAULT_REMOTE_BIN_DIRECTORY}/llm-notch-relay'"
                ),
                String::new(),
            );
            Self {
                probe_result: Ok(RemoteTarget {
                    os: RemoteOs::Linux,
                    architecture: RemoteArchitecture::X86_64,
                }),
                remote_outputs,
                uploads: RefCell::new(Vec::new()),
                fail_step: None,
            }
        }
    }

    impl DeployTransport for FakeDeployTransport {
        fn probe_target(&self, _host: &RemoteHostConfig) -> Result<RemoteTarget, DeployTransportError> {
            if self.fail_step == Some("probe") {
                return Err(DeployTransportError::AuthenticationFailed);
            }
            self.probe_result.clone()
        }

        fn run_remote(
            &self,
            _host: &RemoteHostConfig,
            script: &str,
        ) -> Result<String, DeployTransportError> {
            if self.fail_step == Some("mkdir") && script.starts_with("mkdir -p") {
                return Err(DeployTransportError::Process("mkdir failed".into()));
            }
            if self.fail_step == Some("verify") && script.contains("sha256sum") {
                return Err(DeployTransportError::Process("hash failed".into()));
            }
            if self.fail_step == Some("activate") && script.starts_with("test -f") {
                return Err(DeployTransportError::Process("mv failed".into()));
            }
            self.remote_outputs
                .get(script)
                .cloned()
                .ok_or_else(|| DeployTransportError::Protocol(format!("unexpected script: {script}")))
        }

        fn upload_file(
            &self,
            _host: &RemoteHostConfig,
            local_path: &Path,
            remote_path: &str,
        ) -> Result<(), DeployTransportError> {
            if self.fail_step == Some("upload") {
                return Err(DeployTransportError::Process("upload failed".into()));
            }
            self.uploads
                .borrow_mut()
                .push((local_path.to_path_buf(), remote_path.to_string()));
            Ok(())
        }
    }

    #[test]
    fn parse_probe_output_accepts_linux_and_darwin() {
        assert_eq!(
            parse_probe_output("Linux\nx86_64\n").unwrap(),
            RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            }
        );
        assert_eq!(
            parse_probe_output("Darwin\narm64\n").unwrap(),
            RemoteTarget {
                os: RemoteOs::Macos,
                architecture: RemoteArchitecture::Aarch64,
            }
        );
    }

    #[test]
    fn executor_runs_through_activation_and_skips_start_relay() {
        let plan = sample_plan();
        let expected_hash = plan
            .steps
            .iter()
            .find_map(|step| match step {
                DeploymentStep::VerifySha256 { expected_sha256 } => Some(expected_sha256.clone()),
                _ => None,
            })
            .expect("verify step");
        let transport = FakeDeployTransport::success(&expected_hash);
        let outcome = DeploymentExecutor::new(&transport)
            .execute(&sample_host(), &plan)
            .expect("deploy");
        assert!(outcome.probed_target.is_some());
        assert!(outcome
            .completed_steps
            .iter()
            .any(|step| matches!(step, DeploymentStep::ActivateAtomically { .. })));
        assert!(!outcome
            .completed_steps
            .iter()
            .any(|step| matches!(step, DeploymentStep::StartStdioRelay { .. })));
        assert_eq!(transport.uploads.borrow().len(), 1);
    }

    #[test]
    fn executor_fails_on_hash_mismatch_without_activation() {
        let plan = sample_plan();
        let wrong_hash = "0".repeat(64);
        let transport = FakeDeployTransport::success(&wrong_hash);
        let error = DeploymentExecutor::new(&transport)
            .execute(&sample_host(), &plan)
            .expect_err("hash mismatch");
        assert!(matches!(error, DeployExecError::HashMismatch { .. }));
        assert!(transport.uploads.borrow().len() == 1);
    }

    #[test]
    fn executor_surfaces_transport_failures_honestly() {
        let plan = sample_plan();
        let mut transport = FakeDeployTransport::success("unused");
        transport.fail_step = Some("probe");
        let error = DeploymentExecutor::new(&transport)
            .execute(&sample_host(), &plan)
            .expect_err("probe failed");
        assert!(matches!(
            error,
            DeployExecError::StepFailed {
                step: DeploymentStep::ProbeTarget,
                source: DeployTransportError::AuthenticationFailed,
            }
        ));
    }

    #[test]
    fn scp_base_args_use_capital_port_flag() {
        let mut host = sample_host();
        host.port = Some(2222);
        let args = host.scp_base_args().expect("scp args");
        assert!(args.contains(&"-P".into()));
        assert!(args.contains(&"2222".into()));
        assert!(!args.contains(&"-p".into()));
    }

    #[test]
    fn executor_rejects_probe_target_mismatch_with_artifact() {
        let mut plan = sample_plan();
        plan.artifact.target = RemoteTarget {
            os: RemoteOs::Macos,
            architecture: RemoteArchitecture::Aarch64,
        };
        let expected_hash = plan
            .steps
            .iter()
            .find_map(|step| match step {
                DeploymentStep::VerifySha256 { expected_sha256 } => Some(expected_sha256.clone()),
                _ => None,
            })
            .expect("verify step");
        let transport = FakeDeployTransport::success(&expected_hash);
        let error = DeploymentExecutor::new(&transport)
            .execute(&sample_host(), &plan)
            .expect_err("target mismatch");
        assert!(matches!(
            error,
            DeployExecError::TargetMismatch {
                probed,
                artifact,
            } if probed.os == RemoteOs::Linux
                && artifact.os == RemoteOs::Macos
        ));
        assert!(transport.uploads.borrow().is_empty());
    }

    #[test]
    fn invalid_plan_still_rejected_before_execution() {
        let host = sample_host();
        let artifact = RelayArtifact::from_bytes(
            PathBuf::from("/tmp/relay"),
            RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            },
            b"",
        );
        assert_eq!(
            DeploymentPlan::new(&host, artifact, DEFAULT_REMOTE_BIN_DIRECTORY),
            Err(DeploymentError::InvalidSize)
        );
    }
}
