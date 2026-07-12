use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::deploy::RelayArtifact;
use crate::{RemoteArchitecture, RemoteOs, RemoteTarget};

const RELAY_BINARY_NAME: &str = "llm-notch-relay";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RelayArtifactError {
    #[error("remote deploy does not support {0:?} targets over SSH")]
    UnsupportedTarget(RemoteTarget),
    #[error("no relay artifact for {target:?}; expected at {expected_path}")]
    MissingArtifact {
        target: RemoteTarget,
        expected_path: PathBuf,
    },
    #[error("relay artifact is unreadable at {path}: {message}")]
    Unreadable { path: PathBuf, message: String },
}

/// Maps a probed remote target to the Rust target triple used by cross-compiled relay artifacts.
pub fn rust_triple_for_target(target: RemoteTarget) -> Option<&'static str> {
    match target.os {
        RemoteOs::Windows => None,
        _ => Some(remote_target_triple(target)),
    }
}

pub fn sidecar_filename_for_target(target: RemoteTarget) -> Option<String> {
    let triple = rust_triple_for_target(target)?;
    Some(if triple.contains("windows") {
        format!("{RELAY_BINARY_NAME}-{triple}.exe")
    } else {
        format!("{RELAY_BINARY_NAME}-{triple}")
    })
}

pub fn remote_target_triple(target: RemoteTarget) -> &'static str {
    match (target.os, target.architecture) {
        (RemoteOs::Linux, RemoteArchitecture::X86_64) => "x86_64-unknown-linux-gnu",
        (RemoteOs::Linux, RemoteArchitecture::Aarch64) => "aarch64-unknown-linux-gnu",
        (RemoteOs::Macos, RemoteArchitecture::X86_64) => "x86_64-apple-darwin",
        (RemoteOs::Macos, RemoteArchitecture::Aarch64) => "aarch64-apple-darwin",
        (RemoteOs::Windows, RemoteArchitecture::X86_64) => "x86_64-pc-windows-msvc",
        (RemoteOs::Windows, RemoteArchitecture::Aarch64) => "aarch64-pc-windows-msvc",
    }
}

/// Resolves a relay artifact for a probed remote target from local sidecars or `target/` builds.
pub fn resolve_relay_artifact(
    relay_binaries_dir: &Path,
    host_relay_path: &Path,
    target: RemoteTarget,
) -> Result<RelayArtifact, RelayArtifactError> {
    let triple =
        rust_triple_for_target(target).ok_or(RelayArtifactError::UnsupportedTarget(target))?;
    let expected_path = relay_binaries_dir.join(
        sidecar_filename_for_target(target).expect("triple-checked target has sidecar filename"),
    );
    let candidates = relay_artifact_candidates(relay_binaries_dir, host_relay_path, triple);
    let local_path = candidates.into_iter().find(|path| path.is_file()).ok_or(
        RelayArtifactError::MissingArtifact {
            target,
            expected_path,
        },
    )?;
    let bytes = std::fs::read(&local_path).map_err(|error| RelayArtifactError::Unreadable {
        path: local_path.clone(),
        message: error.to_string(),
    })?;
    Ok(RelayArtifact::from_bytes(local_path, target, &bytes))
}

fn relay_artifact_candidates(
    relay_binaries_dir: &Path,
    host_relay_path: &Path,
    triple: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(filename) = sidecar_filename_for_triple(triple) {
        candidates.push(relay_binaries_dir.join(filename));
    }
    if let Some(workspace_root) = workspace_root_from_binaries_dir(relay_binaries_dir) {
        for profile in ["release", "debug"] {
            candidates.push(
                workspace_root
                    .join("target")
                    .join(triple)
                    .join(profile)
                    .join(relay_binary_filename(triple)),
            );
        }
    }
    if host_relay_path_matches_triple(host_relay_path, triple) {
        candidates.push(host_relay_path.to_path_buf());
    }
    candidates
}

fn workspace_root_from_binaries_dir(relay_binaries_dir: &Path) -> Option<PathBuf> {
    relay_binaries_dir
        .parent()
        .and_then(|src_tauri| src_tauri.parent())
        .map(Path::to_path_buf)
}

fn sidecar_filename_for_triple(triple: &str) -> Option<String> {
    if triple.contains("windows") {
        Some(format!("{RELAY_BINARY_NAME}-{triple}.exe"))
    } else {
        Some(format!("{RELAY_BINARY_NAME}-{triple}"))
    }
}

fn relay_binary_filename(triple: &str) -> String {
    if triple.contains("windows") {
        format!("{RELAY_BINARY_NAME}.exe")
    } else {
        RELAY_BINARY_NAME.into()
    }
}

fn host_relay_path_matches_triple(path: &Path, triple: &str) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(triple))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn linux_x86_64() -> RemoteTarget {
        RemoteTarget {
            os: RemoteOs::Linux,
            architecture: RemoteArchitecture::X86_64,
        }
    }

    fn darwin_arm64() -> RemoteTarget {
        RemoteTarget {
            os: RemoteOs::Macos,
            architecture: RemoteArchitecture::Aarch64,
        }
    }

    #[test]
    fn remote_target_triple_maps_probe_targets() {
        assert_eq!(
            rust_triple_for_target(linux_x86_64()),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            rust_triple_for_target(darwin_arm64()),
            Some("aarch64-apple-darwin")
        );
        assert_eq!(
            rust_triple_for_target(RemoteTarget {
                os: RemoteOs::Windows,
                architecture: RemoteArchitecture::X86_64,
            }),
            None
        );
    }

    #[test]
    fn resolve_prefers_prepared_sidecar_over_target_dir() {
        let workspace = tempdir().expect("tempdir");
        let binaries_dir = workspace.path().join("src-tauri/binaries");
        let sidecar = binaries_dir.join("llm-notch-relay-x86_64-unknown-linux-gnu");
        let target_build = workspace
            .path()
            .join("target/x86_64-unknown-linux-gnu/debug/llm-notch-relay");
        fs::create_dir_all(sidecar.parent().expect("parent")).expect("mkdir");
        fs::create_dir_all(target_build.parent().expect("parent")).expect("mkdir");
        fs::write(&sidecar, b"sidecar-relay").expect("write sidecar");
        fs::write(&target_build, b"target-relay").expect("write target");

        let artifact = resolve_relay_artifact(
            &binaries_dir,
            Path::new("/missing/host-relay"),
            linux_x86_64(),
        )
        .expect("artifact");

        assert_eq!(artifact.local_path, sidecar);
        assert_eq!(artifact.target, linux_x86_64());
    }

    #[test]
    fn resolve_falls_back_to_target_build_output() {
        let workspace = tempdir().expect("tempdir");
        let binaries_dir = workspace.path().join("src-tauri/binaries");
        let target_build = workspace
            .path()
            .join("target/aarch64-apple-darwin/release/llm-notch-relay");
        fs::create_dir_all(&binaries_dir).expect("mkdir binaries");
        fs::create_dir_all(target_build.parent().expect("parent")).expect("mkdir");
        fs::write(&target_build, b"darwin-relay").expect("write target");

        let artifact = resolve_relay_artifact(
            &binaries_dir,
            Path::new("/missing/host-relay"),
            darwin_arm64(),
        )
        .expect("artifact");

        assert_eq!(artifact.local_path, target_build);
        assert_eq!(artifact.target, darwin_arm64());
    }

    #[test]
    fn resolve_honestly_fails_when_matching_artifact_missing() {
        let workspace = tempdir().expect("tempdir");
        let binaries_dir = workspace.path().join("src-tauri/binaries");
        let wrong_target = binaries_dir.join("llm-notch-relay-aarch64-apple-darwin");
        fs::create_dir_all(&binaries_dir).expect("mkdir");
        fs::write(&wrong_target, b"wrong-arch").expect("write wrong");

        let error = resolve_relay_artifact(
            &binaries_dir,
            Path::new("/missing/host-relay"),
            linux_x86_64(),
        )
        .expect_err("missing artifact");

        assert_eq!(
            error,
            RelayArtifactError::MissingArtifact {
                target: linux_x86_64(),
                expected_path: binaries_dir.join("llm-notch-relay-x86_64-unknown-linux-gnu"),
            }
        );
    }

    #[test]
    fn host_relay_path_used_only_when_filename_matches_triple() {
        let workspace = tempdir().expect("tempdir");
        let binaries_dir = workspace.path().join("src-tauri/binaries");
        let host_relay = binaries_dir.join("llm-notch-relay-x86_64-unknown-linux-gnu");
        fs::create_dir_all(&binaries_dir).expect("mkdir");
        fs::write(&host_relay, b"host-relay").expect("write host relay");

        let artifact =
            resolve_relay_artifact(&binaries_dir, &host_relay, linux_x86_64()).expect("artifact");
        assert_eq!(artifact.local_path, host_relay);

        let mismatch = resolve_relay_artifact(&binaries_dir, &host_relay, darwin_arm64())
            .expect_err("host relay should not satisfy different target");
        assert!(matches!(
            mismatch,
            RelayArtifactError::MissingArtifact { .. }
        ));
    }
}
