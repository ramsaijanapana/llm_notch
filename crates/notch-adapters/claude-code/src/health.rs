//! Health probe hints for Claude Code managed hook installation.

use serde_json::Value;

use crate::merge::is_managed_command;
use crate::template::template_settings_hooks;

/// Whether Claude Code settings contain at least one managed llm_notch hook entry.
pub fn managed_entry_present(settings: &Value) -> bool {
    let hints = health_probe_hints(settings);
    hints.state == ClaudeInstallState::Installed
}

/// High-level install state derived from managed hook coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeInstallState {
    NotInstalled,
    Partial,
    Installed,
}

/// Connector-facing health hints for Claude Code hook installation checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeHealthHints {
    pub state: ClaudeInstallState,
    pub expected_events: Vec<String>,
    pub managed_events_found: Vec<String>,
    pub managed_events_missing: Vec<String>,
    /// Claude Code hooks do not require an external trust step in official docs.
    pub trust_required: bool,
    pub detail: Option<String>,
}

/// Inspect Claude Code settings JSON for managed hook coverage.
///
/// Trust is always reported as not required unless Anthropic documents otherwise.
pub fn health_probe_hints(settings: &Value) -> ClaudeHealthHints {
    let expected_events: Vec<String> = template_settings_hooks()
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();
    let mut managed_events_found = Vec::new();
    let hooks = settings.get("hooks");
    if let Some(hooks) = hooks.and_then(Value::as_object) {
        for (event, groups) in hooks {
            if groups
                .as_array()
                .is_some_and(|groups| groups.iter().any(|group| group_has_managed_command(group)))
            {
                managed_events_found.push(event.clone());
            }
        }
    }
    managed_events_found.sort();
    managed_events_found.dedup();

    let managed_events_missing: Vec<String> = expected_events
        .iter()
        .filter(|event| !managed_events_found.iter().any(|found| found == *event))
        .cloned()
        .collect();

    let state = if managed_events_found.is_empty() {
        ClaudeInstallState::NotInstalled
    } else if managed_events_missing.is_empty() {
        ClaudeInstallState::Installed
    } else {
        ClaudeInstallState::Partial
    };

    let detail = match state {
        ClaudeInstallState::Installed => Some("Managed Claude Code hooks present for all template events".into()),
        ClaudeInstallState::Partial => Some(format!(
            "Missing managed Claude Code hook events: {}",
            managed_events_missing.join(", ")
        )),
        ClaudeInstallState::NotInstalled => Some("No managed Claude Code hook entries found".into()),
    };

    ClaudeHealthHints {
        state,
        expected_events,
        managed_events_found,
        managed_events_missing,
        trust_required: false,
        detail,
    }
}

fn group_has_managed_command(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|handlers| {
            handlers.iter().any(|handler| {
                handler
                    .get("command")
                    .and_then(Value::as_str)
                    .is_some_and(is_managed_command)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::merge_settings_hooks;
    use crate::template::template_settings_hooks;

    #[test]
    fn empty_settings_are_not_installed() {
        let hints = health_probe_hints(&serde_json::json!({}));
        assert_eq!(hints.state, ClaudeInstallState::NotInstalled);
        assert!(!hints.trust_required);
        assert!(!managed_entry_present(&serde_json::json!({})));
    }

    #[test]
    fn merged_settings_report_installed_without_trust() {
        let merged =
            merge_settings_hooks(&serde_json::json!({}), &template_settings_hooks()).expect("merge");
        let hints = health_probe_hints(&merged);
        assert_eq!(hints.state, ClaudeInstallState::Installed);
        assert!(!hints.trust_required);
        assert!(managed_entry_present(&merged));
        assert_eq!(hints.managed_events_found.len(), hints.expected_events.len());
    }

    #[test]
    fn partial_install_lists_missing_events() {
        let settings = serde_json::json!({
            "hooks": {
                "SessionStart": template_settings_hooks()["SessionStart"].clone()
            }
        });
        let hints = health_probe_hints(&settings);
        assert_eq!(hints.state, ClaudeInstallState::Partial);
        assert!(hints.managed_events_missing.contains(&"PreToolUse".to_string()));
    }
}
