//! Merge llm_notch entries into existing Codex `hooks.json` without removing foreign hooks.

use serde_json::{Value, json};

/// Install scope for Codex hook templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeScope {
    User,
    Project,
}

/// One managed hook handler fingerprinted for idempotent merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedHookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Returns the managed entries from the shipped Codex template.
pub fn codex_managed_entries(wrapper_command: &str) -> Vec<ManagedHookEntry> {
    [
        ("SessionStart", Some("startup|resume")),
        ("PreToolUse", Some(".*")),
        ("PermissionRequest", Some(".*")),
        ("PostToolUse", Some(".*")),
        ("UserPromptSubmit", None),
        ("Stop", None),
    ]
    .into_iter()
    .map(|(event, matcher)| ManagedHookEntry {
        event: event.into(),
        matcher: matcher.map(|value| value.to_string()),
        command: format!("{wrapper_command} --source codex --vendor-event {event}"),
    })
    .collect()
}

/// Stable fingerprint for `(event, matcher, command)` used during merge dedupe.
pub fn entry_fingerprint(entry: &ManagedHookEntry) -> String {
    format!(
        "{}|{}|{}",
        entry.event,
        entry.matcher.as_deref().unwrap_or("*"),
        entry.command
    )
}

/// Returns true when `command` references the llm_notch Codex wrapper for `event`.
pub fn is_managed_command(event: &str, command: &str) -> bool {
    command.contains("llm-notch-hook-wrapper")
        && command.contains("--source codex")
        && command.contains("--vendor-event")
        && command.contains(event)
}

/// Merge managed Codex hook entries into `target`, preserving unrelated hooks.
pub fn merge_hooks_json(target: &mut Value, entries: &[ManagedHookEntry]) {
    let hooks = target
        .as_object_mut()
        .and_then(|object| {
            object
                .entry("hooks")
                .or_insert_with(|| json!({}))
                .as_object_mut()
        })
        .expect("hooks object");

    for entry in entries {
        let group = hooks
            .entry(entry.event.clone())
            .or_insert_with(|| json!([]));
        let Some(groups) = group.as_array_mut() else {
            continue;
        };

        let new_group = if let Some(matcher) = &entry.matcher {
            json!({
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": entry.command.clone(),
                    "timeout": 2
                }]
            })
        } else {
            json!({
                "hooks": [{
                    "type": "command",
                    "command": entry.command.clone(),
                    "timeout": 2
                }]
            })
        };

        let fingerprint = entry_fingerprint(entry);
        let duplicate = groups.iter().any(|existing| {
            existing
                .get("hooks")
                .and_then(Value::as_array)
                .and_then(|handlers| handlers.first())
                .and_then(|handler| handler.get("command"))
                .and_then(Value::as_str)
                .is_some_and(|command| fingerprint.ends_with(command))
        });

        if !duplicate {
            groups.push(new_group);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_appends_without_removing_foreign_hooks() {
        let mut target = json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/bin/python3 ~/.codex/hooks/policy.py"
                    }]
                }]
            }
        });
        let entries = codex_managed_entries("sh /hooks/llm-notch-hook-wrapper.sh");
        merge_hooks_json(&mut target, &entries);
        let groups = target["hooks"]["PreToolUse"].as_array().expect("groups");
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn merge_is_idempotent() {
        let mut target = json!({ "hooks": {} });
        let entries = codex_managed_entries("sh /hooks/llm-notch-hook-wrapper.sh");
        merge_hooks_json(&mut target, &entries);
        merge_hooks_json(&mut target, &entries);
        let session_groups = target["hooks"]["SessionStart"]
            .as_array()
            .expect("session groups");
        assert_eq!(session_groups.len(), 1);
    }
}
