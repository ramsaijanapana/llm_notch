//! Merge llm_notch managed hooks into Claude Code `settings.json` without touching other keys.

use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::template::{self, template_settings_hooks};

/// Install scope for Claude Code settings files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeScope {
    User,
    Project,
}

/// Managed hook entry fingerprint used by the connector merge planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedHookEntry {
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
}

/// Returns the managed entries the shipped template would install.
pub fn claude_managed_entries() -> Vec<ManagedHookEntry> {
    managed_entries_from_hooks(&template_settings_hooks())
}

/// Stable fingerprint for `(event, matcher, command)` triples.
pub fn entry_fingerprint(event: &str, matcher: Option<&str>, command: &str) -> String {
    format!(
        "{}::{}::{}",
        event,
        matcher.unwrap_or("*"),
        normalize_command(command)
    )
}

/// Whether a hook command string belongs to llm_notch's Claude Code integration.
pub fn is_managed_command(command: &str) -> bool {
    let normalized = normalize_command(command);
    normalized.contains("--source claudeCode")
        && (normalized.contains("llm-notch-hook")
            || normalized.contains(template::HELPER_PATH_PLACEHOLDER)
            || normalized.contains("llm-notch-hook-wrapper")
            || normalized.contains(template::WRAPPER_PATH_PLACEHOLDER))
}

/// Merge only the `hooks` object from the template into existing settings JSON.
///
/// Preserves `permissions`, `model`, env overrides, and unrelated settings keys.
pub fn merge_settings_hooks(existing: &Value, template_hooks: &Value) -> Result<Value, MergeError> {
    let template_object = template_hooks
        .as_object()
        .ok_or(MergeError::InvalidTemplate)?;
    let mut merged = match existing {
        Value::Object(map) => Value::Object(map.clone()),
        Value::Null => json!({}),
        _ => return Err(MergeError::InvalidExisting),
    };
    let root = merged.as_object_mut().expect("merged settings root object");
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks_map = hooks
        .as_object_mut()
        .ok_or(MergeError::InvalidExistingHooks)?;

    for (event, template_groups) in template_object {
        let template_groups = template_groups
            .as_array()
            .ok_or(MergeError::InvalidTemplateEvent(event.clone()))?;
        let target_groups = hooks_map
            .entry(event.clone())
            .or_insert_with(|| Value::Array(vec![]));
        let target_array = target_groups
            .as_array_mut()
            .ok_or(MergeError::InvalidExistingEvent(event.clone()))?;

        for template_group in template_groups {
            if group_present(target_array, event, template_group)? {
                continue;
            }
            target_array.push(template_group.clone());
        }
    }

    Ok(merged)
}

fn group_present(
    target_array: &[Value],
    event: &str,
    template_group: &Value,
) -> Result<bool, MergeError> {
    let template_matcher = template_group.get("matcher").and_then(Value::as_str);
    let template_commands = inner_commands(template_group)?;
    for command in template_commands {
        let fingerprint = entry_fingerprint(event, template_matcher, command);
        for existing_group in target_array {
            let existing_matcher = existing_group.get("matcher").and_then(Value::as_str);
            for existing_command in inner_commands(existing_group)? {
                if entry_fingerprint(event, existing_matcher, existing_command) == fingerprint {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn inner_commands(group: &Value) -> Result<Vec<&str>, MergeError> {
    let hooks = group
        .get("hooks")
        .and_then(Value::as_array)
        .ok_or(MergeError::InvalidTemplateGroup)?;
    let mut commands = Vec::new();
    for handler in hooks {
        if let Some(command) = handler.get("command").and_then(Value::as_str) {
            commands.push(command);
        }
    }
    if commands.is_empty() {
        return Err(MergeError::InvalidTemplateGroup);
    }
    Ok(commands)
}

fn managed_entries_from_hooks(hooks: &Value) -> Vec<ManagedHookEntry> {
    let mut entries = Vec::new();
    let Some(object) = hooks.as_object() else {
        return entries;
    };
    for (event, groups) in object {
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for group in groups {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Ok(commands) = inner_commands(group) {
                for command in commands {
                    entries.push(ManagedHookEntry {
                        event: event.clone(),
                        matcher: matcher.clone(),
                        command: command.to_string(),
                    });
                }
            }
        }
    }
    entries
}

fn normalize_command(command: &str) -> String {
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MergeError {
    #[error("existing settings must be a JSON object or null")]
    InvalidExisting,
    #[error("template hooks must be a JSON object")]
    InvalidTemplate,
    #[error("existing hooks.{0} must be an array")]
    InvalidExistingEvent(String),
    #[error("existing hooks must be a JSON object")]
    InvalidExistingHooks,
    #[error("template event {0} must be an array")]
    InvalidTemplateEvent(String),
    #[error("template hook group is missing command handlers")]
    InvalidTemplateGroup,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_preserves_permissions_and_model() {
        let existing = json!({
            "model": "claude-sonnet-4-5",
            "permissions": {
                "defaultMode": "acceptEdits"
            },
            "env": {
                "FOO": "bar"
            }
        });
        let merged = merge_settings_hooks(&existing, &template_settings_hooks()).expect("merge");
        assert_eq!(merged["model"], "claude-sonnet-4-5");
        assert_eq!(merged["permissions"]["defaultMode"], "acceptEdits");
        assert_eq!(merged["env"]["FOO"], "bar");
        assert!(merged["hooks"].is_object());
    }

    #[test]
    fn merge_appends_missing_managed_entries_without_duplicates() {
        let existing = json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "startup|resume",
                    "hooks": [{
                        "type": "command",
                        "command": template::render_hook_command("SessionStart"),
                        "timeout": 2
                    }]
                }]
            }
        });
        let merged = merge_settings_hooks(&existing, &template_settings_hooks()).expect("merge");
        let session_start = merged["hooks"]["SessionStart"]
            .as_array()
            .expect("session start array");
        assert_eq!(session_start.len(), 1);
        assert!(merged["hooks"]["PreToolUse"].is_array());
    }

    #[test]
    fn managed_command_detection_matches_placeholder_and_resolved_paths() {
        assert!(is_managed_command(&template::render_hook_command("Stop")));
        assert!(is_managed_command(
            "/Applications/llm_notch.app/Contents/MacOS/llm-notch-hook hook --source claudeCode --vendor-event Stop --hook-mode"
        ));
        assert!(!is_managed_command("echo hello"));
    }
}
