//! Resolve the `llm-notch-relay` sidecar at runtime.
//!
//! Resolution order (first existing file wins):
//! 1. Tauri [`AppHandle::path().executable_dir()`] — sidecar next to the desktop exe
//!    (packaged `externalBin` from `tauri.conf.json`, prepared as
//!    `src-tauri/binaries/llm-notch-relay-<target>` by `npm run native:prepare-helper`)
//! 2. Tauri [`AppHandle::path().resource_dir()`] — bundled resources fallback
//! 3. `LLM_NOTCH_RELAY_BIN` environment override (development / CI injection)
//! 4. Workspace `target/debug/llm-notch-relay` fallback for local `tauri dev`
//!
//! See [`docs/platform/release-gates.md`](../../../docs/platform/release-gates.md).

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Bundled relay filename inside the Tauri resource directory.
pub fn bundled_relay_filename() -> &'static str {
    if cfg!(windows) {
        "llm-notch-relay.exe"
    } else {
        "llm-notch-relay"
    }
}

/// Expected path inside a packaged app's resource directory.
pub fn bundled_relay_in_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join(bundled_relay_filename())
}

/// Directory containing cross-compiled relay sidecars (`llm-notch-relay-<target-triple>`).
pub fn relay_binaries_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries")
}

/// Resolve the directory used for cross-compiled relay deploy artifacts.
///
/// Packaged builds bundle target-suffixed relay sidecars under
/// `$RESOURCE/binaries/`; development builds use [`relay_binaries_directory`].
pub fn resolve_relay_binaries_directory(app: &AppHandle) -> PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("binaries");
        if bundled.is_dir()
            && bundled.read_dir().map_or(false, |entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with("llm-notch-relay-"))
                })
            })
        {
            return bundled;
        }
    }
    relay_binaries_directory()
}

/// Resolve the relay binary used for deployment previews and bundled remote relay.
pub fn resolve_relay_binary_path(app: &AppHandle) -> PathBuf {
    if let Ok(exe_dir) = app.path().executable_dir() {
        let sidecar = exe_dir.join(bundled_relay_filename());
        if sidecar.is_file() {
            return sidecar;
        }
    }

    if let Ok(resource) = app.path().resource_dir() {
        let bundled = bundled_relay_in_resource_dir(&resource);
        if bundled.is_file() {
            return bundled;
        }
    }

    if let Ok(path) = std::env::var("LLM_NOTCH_RELAY_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_relay = manifest_dir
        .join("../target/debug")
        .join(bundled_relay_filename());
    if target_relay.is_file() {
        return target_relay;
    }

    target_relay
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_relay_filename_matches_platform() {
        #[cfg(windows)]
        assert_eq!(bundled_relay_filename(), "llm-notch-relay.exe");
        #[cfg(not(windows))]
        assert_eq!(bundled_relay_filename(), "llm-notch-relay");
    }

    #[test]
    fn resource_dir_joins_expected_name() {
        let path = bundled_relay_in_resource_dir(Path::new("/tmp/resources"));
        assert!(path.ends_with(bundled_relay_filename()));
    }

    #[test]
    fn relay_binaries_directory_points_at_sidecar_staging_dir() {
        let dir = relay_binaries_directory();
        assert!(dir.ends_with("binaries"));
    }
}
