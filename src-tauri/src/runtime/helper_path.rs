//! Resolve the `llm-notch-hook` sidecar at runtime.
//!
//! Resolution order (first existing file wins):
//! 1. `LLM_NOTCH_HOOK_BIN` environment override (development / CI injection)
//! 2. Tauri [`AppHandle::path().resource_dir()`] — packaged `externalBin` from
//!    `tauri.conf.json` → `binaries/llm-notch-hook` (prepared as
//!    `src-tauri/binaries/llm-notch-hook-<target>` by `npm run native:prepare-helper`)
//! 3. Workspace `target/debug/llm-notch-hook` fallback for local `tauri dev`
//!
//! See [`docs/integrations/helper-paths.md`](../../../docs/integrations/helper-paths.md).

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Bundled helper filename inside the Tauri resource directory.
pub fn bundled_helper_filename() -> &'static str {
    if cfg!(windows) {
        "llm-notch-hook.exe"
    } else {
        "llm-notch-hook"
    }
}

/// Expected path inside a packaged app's resource directory.
pub fn bundled_helper_in_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join(bundled_helper_filename())
}

/// Resolve the hook helper path for connector management and health probes.
pub fn resolve_helper_path(app: &AppHandle) -> PathBuf {
    if let Ok(path) = std::env::var("LLM_NOTCH_HOOK_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
    }

    if let Ok(resource) = app.path().resource_dir() {
        let bundled = bundled_helper_in_resource_dir(&resource);
        if bundled.is_file() {
            return bundled;
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_helper = manifest_dir
        .join("../target/debug")
        .join(bundled_helper_filename());
    if target_helper.is_file() {
        return target_helper;
    }

    target_helper
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_helper_filename_matches_platform() {
        #[cfg(windows)]
        assert_eq!(bundled_helper_filename(), "llm-notch-hook.exe");
        #[cfg(not(windows))]
        assert_eq!(bundled_helper_filename(), "llm-notch-hook");
    }

    #[test]
    fn resource_dir_joins_expected_name() {
        let path = bundled_helper_in_resource_dir(Path::new("/tmp/resources"));
        assert!(path.ends_with(bundled_helper_filename()));
    }
}
