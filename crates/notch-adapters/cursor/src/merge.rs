//! hooks.json merge specifications for user and project scopes.

use serde_json::{Map, Value, json};

use crate::template::{HELPER_PATH_PLACEHOLDER, render_hook_command};

/// Install scope for Cursor hooks.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeScope {
    User,
    Project,
}

/// Managed hook entry fingerprint and rendered command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedHookEntry {
    pub event: &'static str,
    pub fingerprint: String,
    pub command: String,
    pub timeout_sec: u64,
}

/// Stable fingerprint prefix for managed Cursor entries.
pub const MANAGED_FINGERPRINT_PREFIX: &str = "llm-notch:cursor:";

/// Shipped V1 lifecycle and tool observation events per Cursor docs.
pub const MANAGED_EVENTS: &[&str] = &[
    "sessionStart",
    "sessionEnd",
    "preToolUse",
    "postToolUse",
    "postToolUseFailure",
    "stop",
];

/// Returns managed entries for a scope using the helper placeholder convention.
pub fn cursor_managed_entries(scope: MergeScope) -> Vec<ManagedHookEntry> {
    MANAGED_EVENTS
        .iter()
        .map(|event| ManagedHookEntry {
            event,
            fingerprint: entry_fingerprint(event),
            command: render_hook_command(scope, event, HELPER_PATH_PLACEHOLDER),
            timeout_sec: 2,
        })
        .collect()
}

/// Fingerprint string used to detect an existing llm_notch managed entry.
pub fn entry_fingerprint(vendor_event: &str) -> String {
    format!("{MANAGED_FINGERPRINT_PREFIX}{vendor_event}")
}

/// Returns true when a command string belongs to llm_notch Cursor managed hooks.
pub fn is_managed_command(command: &str) -> bool {
    command.contains("--source cursor")
        && command.contains("--vendor-event")
        && (command.contains("llm-notch")
            || command.contains(HELPER_PATH_PLACEHOLDER)
            || command.contains("--hook-mode"))
}

/// Merge managed entries into an existing hooks.json value, preserving foreign hooks.
pub fn merge_hooks_json(existing: &Value, managed: &[ManagedHookEntry]) -> Value {
    let mut root = existing.as_object().cloned().unwrap_or_else(Map::new);

    root.entry("version".to_string()).or_insert(json!(1));

    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("hooks object");

    for entry in managed {
        let slot = hooks
            .entry(entry.event.to_string())
            .or_insert_with(|| json!([]));
        let Some(array) = slot.as_array_mut() else {
            continue;
        };

        if array
            .iter()
            .any(|item| command_matches(item, &entry.fingerprint))
        {
            continue;
        }

        array.push(json!({
            "command": entry.command,
            "timeout": entry.timeout_sec,
        }));
    }

    Value::Object(root)
}

fn command_matches(item: &Value, fingerprint: &str) -> bool {
    item.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| {
            is_managed_command(command)
                && command.contains(&fingerprint[MANAGED_FINGERPRINT_PREFIX.len()..])
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::template_hooks_json;

    #[test]
    fn managed_entries_cover_v1_events() {
        let entries = cursor_managed_entries(MergeScope::Project);
        assert_eq!(entries.len(), MANAGED_EVENTS.len());
        assert!(
            entries
                .iter()
                .any(|entry| entry.event == "postToolUseFailure")
        );
    }

    #[test]
    fn merge_preserves_foreign_entries() {
        let existing = json!({
            "version": 1,
            "hooks": {
                "afterFileEdit": [{ "command": "./hooks/format.sh" }],
                "sessionStart": [{ "command": "echo legacy" }]
            }
        });
        let managed = cursor_managed_entries(MergeScope::User);
        let merged = merge_hooks_json(&existing, &managed);
        let hooks = merged["hooks"].as_object().expect("hooks");
        assert_eq!(hooks["afterFileEdit"].as_array().unwrap().len(), 1);
        let session = hooks["sessionStart"].as_array().unwrap();
        assert_eq!(session.len(), 2);
        assert_eq!(session[0]["command"], "echo legacy");
        assert!(is_managed_command(session[1]["command"].as_str().unwrap()));
    }

    #[test]
    fn merge_is_idempotent() {
        let managed = cursor_managed_entries(MergeScope::Project);
        let _template = template_hooks_json(MergeScope::Project, HELPER_PATH_PLACEHOLDER);
        let once = merge_hooks_json(&json!({ "version": 1, "hooks": {} }), &managed);
        let twice = merge_hooks_json(&once, &managed);
        assert_eq!(once, twice);
    }
}
