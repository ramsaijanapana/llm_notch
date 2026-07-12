//! Resolve bundled connector templates at runtime.
//!
//! Resolution order (first existing directory wins):
//! 1. `LLM_NOTCH_INTEGRATIONS_DIR` environment override (development / CI)
//! 2. Tauri [`AppHandle::path().resource_dir()`] → `integrations/` (packaged resources)
//! 3. Workspace `integrations/` fallback for local `tauri dev` / `cargo test`

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Relative path inside the Tauri resource bundle.
pub fn bundled_integrations_subdir() -> &'static str {
    "integrations"
}

/// Expected integrations root inside a packaged app's resource directory.
pub fn bundled_integrations_in_resource_dir(resource_dir: &Path) -> PathBuf {
    resource_dir.join(bundled_integrations_subdir())
}

/// Dev/test fallback: repository `integrations/` next to the workspace root.
pub fn dev_integrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("integrations")
}

/// Resolve connector template root for [`notch_connectors::ConnectorConfig`].
pub fn resolve_integrations_dir(app: &AppHandle) -> PathBuf {
    if let Ok(path) = std::env::var("LLM_NOTCH_INTEGRATIONS_DIR") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return path;
        }
    }

    if let Ok(resource) = app.path().resource_dir() {
        let bundled = bundled_integrations_in_resource_dir(&resource);
        if bundled.is_dir() {
            return bundled;
        }
    }

    let dev = dev_integrations_dir();
    if dev.is_dir() {
        return dev;
    }

    dev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_integrations_dir_contains_cursor_template() {
        let dir = dev_integrations_dir();
        assert!(
            dir.join("cursor/hooks.json.template").is_file(),
            "expected cursor template under {}",
            dir.display()
        );
    }

    #[test]
    fn bundled_subdir_joins_expected_name() {
        let path = bundled_integrations_in_resource_dir(Path::new("/tmp/resources"));
        assert!(path.ends_with("integrations"));
    }
}
