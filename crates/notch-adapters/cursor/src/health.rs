//! Health probe hints for the connector detection lane.

use notch_protocol::{
    HealthProbeAxis, HealthProbeFailureKind, HealthProbeOutcome, HealthProbeResult,
};

use crate::merge::{MANAGED_EVENTS, is_managed_command};

/// Observed install state for Cursor hooks.json (user or project scope).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CursorInstallState {
    pub hooks_file_exists: bool,
    pub hooks_schema_version: Option<u64>,
    pub managed_events_present: Vec<String>,
    pub managed_events_missing: Vec<String>,
    pub foreign_events_present: Vec<String>,
    pub helper_path_configured: bool,
}

/// Connector-facing hints derived from install state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorHealthHints {
    pub managed_entry_present: bool,
    pub all_managed_events_present: bool,
    pub installation_probe: HealthProbeResult,
    pub trust_probe: HealthProbeResult,
    pub helper_probe: HealthProbeResult,
    pub detail: Option<String>,
}

/// Returns true when at least one managed llm_notch Cursor hook entry is present.
///
/// "Managed entry present" means a `hooks.json` command string matches
/// [`crate::merge::is_managed_command`] for any shipped V1 vendor event.
pub fn managed_entry_present(state: &CursorInstallState) -> bool {
    !state.managed_events_present.is_empty()
}

/// Builds orthogonal health probe hints for the connector manager.
pub fn health_probe_hints(state: &CursorInstallState) -> CursorHealthHints {
    let managed_entry_present = managed_entry_present(state);
    let all_managed_events_present =
        state.managed_events_missing.is_empty() && managed_entry_present && state.hooks_file_exists;

    let installation_probe = if !state.hooks_file_exists {
        HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::NotInstalled),
            detail: Some("Cursor hooks.json not found for this scope".into()),
        }
    } else if !managed_entry_present {
        HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Warn,
            failure_kind: Some(HealthProbeFailureKind::ConfigDrift),
            detail: Some("hooks.json exists but no llm_notch managed entries detected".into()),
        }
    } else if all_managed_events_present {
        HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        }
    } else {
        HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Warn,
            failure_kind: Some(HealthProbeFailureKind::ConfigDrift),
            detail: Some(format!(
                "Partial install: missing events {:?}",
                state.managed_events_missing
            )),
        }
    };

    let trust_probe = HealthProbeResult {
        axis: HealthProbeAxis::Trust,
        outcome: if managed_entry_present {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Warn
        },
        failure_kind: None,
        detail: Some(
            "Cursor hooks require no external trust step; enable in Cursor Settings → Hooks".into(),
        ),
    };

    let helper_probe = HealthProbeResult {
        axis: HealthProbeAxis::Helper,
        outcome: if state.helper_path_configured {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Fail
        },
        failure_kind: if state.helper_path_configured {
            None
        } else {
            Some(HealthProbeFailureKind::HelperUnavailable)
        },
        detail: if state.helper_path_configured {
            None
        } else {
            Some("Managed hook command missing resolved helper path".into())
        },
    };

    let detail = if all_managed_events_present {
        None
    } else {
        Some(format!(
            "managed={managed_entry_present}; present={:?}; missing={:?}; foreign={:?}",
            state.managed_events_present,
            state.managed_events_missing,
            state.foreign_events_present
        ))
    };

    CursorHealthHints {
        managed_entry_present,
        all_managed_events_present,
        installation_probe,
        trust_probe,
        helper_probe,
        detail,
    }
}

/// Classifies parsed hooks.json commands into managed/foreign buckets.
pub fn classify_hooks_commands(
    hooks: &serde_json::Map<String, serde_json::Value>,
) -> CursorInstallState {
    let mut state = CursorInstallState {
        hooks_file_exists: true,
        ..CursorInstallState::default()
    };

    for event in MANAGED_EVENTS {
        let present = hooks.get(*event).is_some_and(|entries| {
            entries.as_array().is_some_and(|array| {
                array.iter().any(|entry| {
                    entry
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(is_managed_command)
                })
            })
        });
        if present {
            state.managed_events_present.push((*event).to_string());
        } else {
            state.managed_events_missing.push((*event).to_string());
        }
    }

    for (event, entries) in hooks {
        if MANAGED_EVENTS.contains(&event.as_str()) {
            continue;
        }
        if entries.as_array().is_some_and(|array| !array.is_empty()) {
            state.foreign_events_present.push(event.clone());
        }
    }

    state.helper_path_configured = hooks.values().any(|entries| {
        entries.as_array().is_some_and(|array| {
            array.iter().any(|entry| {
                entry
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|command| {
                        is_managed_command(command)
                            && !command.contains(crate::template::HELPER_PATH_PLACEHOLDER)
                    })
            })
        })
    });

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::cursor_managed_entries;
    use crate::merge::merge_hooks_json;
    use crate::template::HELPER_PATH_PLACEHOLDER;
    use serde_json::json;

    #[test]
    fn managed_entry_present_when_any_event_installed() {
        let managed = cursor_managed_entries(crate::merge::MergeScope::User);
        let merged = merge_hooks_json(&json!({ "version": 1, "hooks": {} }), &managed[..1]);
        let hooks = merged["hooks"].as_object().unwrap();
        let state = classify_hooks_commands(hooks);
        assert!(managed_entry_present(&state));
        assert!(!state.managed_events_missing.is_empty());
    }

    #[test]
    fn helper_probe_fails_when_placeholder_unresolved() {
        let managed = cursor_managed_entries(crate::merge::MergeScope::Project);
        let merged = merge_hooks_json(&json!({ "version": 1, "hooks": {} }), &managed);
        let hooks = merged["hooks"].as_object().unwrap();
        let state = classify_hooks_commands(hooks);
        let hints = health_probe_hints(&state);
        assert!(!state.helper_path_configured);
        assert_eq!(hints.helper_probe.outcome, HealthProbeOutcome::Fail);
    }

    #[test]
    fn helper_probe_ok_when_helper_path_resolved() {
        let managed = cursor_managed_entries(crate::merge::MergeScope::Project);
        let resolved: Vec<_> = managed
            .into_iter()
            .map(|mut entry| {
                entry.command = entry
                    .command
                    .replace(HELPER_PATH_PLACEHOLDER, "/opt/llm-notch-hook");
                entry
            })
            .collect();
        let merged = merge_hooks_json(&json!({ "version": 1, "hooks": {} }), &resolved);
        let hooks = merged["hooks"].as_object().unwrap();
        let state = classify_hooks_commands(hooks);
        let hints = health_probe_hints(&state);
        assert!(state.helper_path_configured);
        assert_eq!(hints.helper_probe.outcome, HealthProbeOutcome::Ok);
    }
}
