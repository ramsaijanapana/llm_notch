//! Conservative process discovery: observes running executables only.
//!
//! Does **not** create [`notch_protocol::AgentSession`] records or invent session IDs.
//! When hook IPC later supplies an `external_session_id`, upstream state may correlate
//! `process_running` evidence with verified sessions — that merge is not implemented here.
//!
//! Scanning is limited to catalog-declared executable basenames with ambiguous names
//! (for example `agent`) filtered out to keep false-attribution risk low.

use notch_agent_catalog::AgentCatalog;

/// Positive evidence that a catalog agent executable is running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRunningEvidence {
    pub running: bool,
    pub matched_name: Option<String>,
}

/// Returns process-running evidence for a catalog integration id.
pub fn process_running_for_catalog(catalog_id: &str) -> ProcessRunningEvidence {
    let patterns = process_scan_patterns_for(catalog_id);
    if patterns.is_empty() {
        return ProcessRunningEvidence {
            running: false,
            matched_name: None,
        };
    }
    scan_running_process(&patterns, &list_running_process_basenames())
}

/// Returns conservative basename patterns for process scanning.
pub fn process_scan_patterns_for(catalog_id: &str) -> Vec<String> {
    let catalog = AgentCatalog::vibe_island_25();
    let Some(descriptor) = catalog.get(catalog_id) else {
        return Vec::new();
    };
    descriptor
        .executable_names
        .iter()
        .map(|name| normalize_process_basename(name))
        .filter(|name| !name.is_empty())
        .filter(|name| !is_ambiguous_process_name(name))
        .collect()
}

/// Matches `patterns` against a supplied process basename list (test seam).
pub fn scan_running_process(
    patterns: &[String],
    running_processes: &[String],
) -> ProcessRunningEvidence {
    for process in running_processes {
        let normalized = normalize_process_basename(process);
        if patterns.iter().any(|pattern| pattern == &normalized) {
            return ProcessRunningEvidence {
                running: true,
                matched_name: Some(normalized),
            };
        }
    }
    ProcessRunningEvidence {
        running: false,
        matched_name: None,
    }
}

/// Basenames too generic for conservative process attribution.
const AMBIGUOUS_PROCESS_NAMES: &[&str] = &["agent"];

fn is_ambiguous_process_name(name: &str) -> bool {
    AMBIGUOUS_PROCESS_NAMES.contains(&name)
}

fn normalize_process_basename(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let lowercase = basename.to_ascii_lowercase();
    lowercase
        .strip_suffix(".exe")
        .unwrap_or(&lowercase)
        .to_owned()
}

#[cfg(windows)]
fn list_running_process_basenames() -> Vec<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(handle) => handle,
        Err(_) => return Vec::new(),
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut names = Vec::new();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|ch| *ch == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = OsString::from_wide(&entry.szExeFile[..len]);
                if let Ok(text) = name.into_string() {
                    names.push(text);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    names
}

#[cfg(not(windows))]
fn list_running_process_basenames() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_ambiguous_agent_basename() {
        let patterns = process_scan_patterns_for("cursor");
        assert!(patterns.contains(&"cursor".to_string()));
        assert!(!patterns.contains(&"agent".to_string()));
    }

    #[test]
    fn scan_matches_case_insensitive_exe_names() {
        let evidence = scan_running_process(
            &["cursor".into()],
            &["Cursor.exe".into(), "explorer.exe".into()],
        );
        assert!(evidence.running);
        assert_eq!(evidence.matched_name.as_deref(), Some("cursor"));
    }

    #[test]
    fn scan_returns_negative_when_no_patterns() {
        let evidence = scan_running_process(&[], &["Cursor.exe".into()]);
        assert!(!evidence.running);
        assert!(evidence.matched_name.is_none());
    }

    #[test]
    fn scan_does_not_match_unrelated_processes() {
        let evidence = scan_running_process(
            &["codex".into(), "claude".into()],
            &["explorer.exe".into(), "Code.exe".into()],
        );
        assert!(!evidence.running);
    }

    #[test]
    fn agy_pattern_is_allowed_for_antigravity() {
        let patterns = process_scan_patterns_for("antigravity-cli");
        assert!(patterns.contains(&"agy".to_string()));
        let evidence = scan_running_process(&patterns, &["agy.exe".into()]);
        assert!(evidence.running);
    }
}
