use std::path::{Path, PathBuf};

/// Returns true when any of the supplied executable basenames resolves on `PATH` or known install dirs.
pub fn any_executable(names: &[&str]) -> bool {
    resolve_executable(names).is_some()
}

/// Returns true when any of the supplied executable basenames resolves on `PATH`.
pub fn any_on_path(names: &[&str]) -> bool {
    names.iter().any(|name| find_on_path(name).is_some())
}

/// Resolve the first matching executable basename on `PATH` or known install directories.
pub fn resolve_executable(names: &[&str]) -> Option<PathBuf> {
    for name in names {
        if let Some(path) = find_on_path(name) {
            return Some(path);
        }
    }
    for dir in known_search_dirs() {
        for name in names {
            for candidate in candidate_paths(&dir, name) {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Resolve the first matching executable basename on `PATH`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for candidate in candidate_paths(&dir, name) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn candidate_paths(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(dir.join(name));
    #[cfg(windows)]
    {
        candidates.push(dir.join(format!("{name}.exe")));
        candidates.push(dir.join(format!("{name}.cmd")));
        candidates.push(dir.join(format!("{name}.bat")));
        candidates.push(dir.join(format!("{name}.ps1")));
    }
    candidates
}

/// Fixed, published install locations checked after `PATH` on each platform.
fn known_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            dirs.push(local.join("Programs").join("cursor"));
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            dirs.push(PathBuf::from(program_files).join("cursor"));
        }
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(program_files_x86).join("cursor"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/Applications/Cursor.app/Contents/Resources/app/bin"));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local").join("bin"));
            dirs.push(home.join(".npm-global").join("bin"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local").join("bin"));
            dirs.push(home.join(".npm-global").join("bin"));
        }
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_common_path_entries() {
        #[cfg(windows)]
        assert!(any_on_path(&["cmd"]));
        #[cfg(unix)]
        assert!(any_on_path(&["sh"]));
    }

    #[test]
    fn missing_executable_returns_none() {
        assert!(find_on_path("definitely-not-a-real-llm-notch-agent-9999").is_none());
        assert!(resolve_executable(&["definitely-not-a-real-llm-notch-agent-9999"]).is_none());
    }

    #[test]
    fn resolve_prefers_path_before_known_dirs() {
        let resolved = resolve_executable(&["cmd"]);
        #[cfg(windows)]
        assert!(resolved.is_some());
        #[cfg(unix)]
        assert!(resolved.is_some());
    }
}
