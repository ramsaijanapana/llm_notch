use serde_json::{Map, Value};

pub const MANAGED_MARKER: &str = "llm-notch-hook";

pub fn is_managed_command(command: &str) -> bool {
    command.contains(MANAGED_MARKER)
}

/// Merge hooks.json-style configs (Cursor, Codex).
pub fn merge_hooks_json(target: &Value, template: &Value) -> (Value, Vec<String>) {
    let mut merged = target.clone();
    let mut preserved = collect_foreign_hooks_json(&merged);

    let Some(template_hooks) = template.get("hooks").and_then(Value::as_object) else {
        return (merged, preserved);
    };

    let target_hooks = merged
        .as_object_mut()
        .and_then(|root| {
            if !root.contains_key("hooks") {
                root.insert("hooks".into(), Value::Object(Map::new()));
            }
            root.get_mut("hooks").and_then(Value::as_object_mut)
        });

    let Some(target_hooks) = target_hooks else {
        return (merged, preserved);
    };

    for (event, template_entries) in template_hooks {
        let template_entries = match template_entries {
            Value::Array(entries) => entries,
            _ => continue,
        };

        let managed_template: Vec<Value> = template_entries
            .iter()
            .filter(|entry| entry_command(entry).is_some_and(is_managed_command))
            .cloned()
            .collect();

        if managed_template.is_empty() {
            continue;
        }

        let slot = target_hooks
            .entry(event.clone())
            .or_insert_with(|| Value::Array(Vec::new()));

        let target_entries = match slot {
            Value::Array(entries) => entries,
            _ => {
                *slot = Value::Array(Vec::new());
                slot.as_array_mut().expect("array")
            }
        };

        for template_entry in managed_template {
            let Some(template_command) = entry_command(&template_entry) else {
                continue;
            };
            let exists = target_entries.iter().any(|entry| {
                entry_command(entry).is_some_and(|command| command == template_command)
            });
            if !exists {
                target_entries.push(template_entry);
            }
        }
    }

    (merged, preserved)
}

/// Remove llm_notch managed entries from hooks.json-style configs.
pub fn remove_hooks_json(target: &Value) -> (Value, Vec<String>) {
    let mut merged = target.clone();
    let mut removed = Vec::new();

    let Some(target_hooks) = merged
        .as_object_mut()
        .and_then(|root| root.get_mut("hooks"))
        .and_then(Value::as_object_mut)
    else {
        return (merged, removed);
    };

    for (event, entries) in target_hooks.iter_mut() {
        let Some(entries) = entries.as_array_mut() else {
            continue;
        };
        entries.retain(|entry| {
            if entry_command(entry).is_some_and(is_managed_command) {
                removed.push(event.clone());
                false
            } else {
                true
            }
        });
        if entries.is_empty() {
            // keep empty arrays; cleanup happens at serialization
        }
    }

    target_hooks.retain(|_, entries| {
        entries
            .as_array()
            .map(|items| !items.is_empty())
            .unwrap_or(true)
    });

    (merged, removed)
}

/// Merge Claude Code nested settings hooks.
pub fn merge_claude_settings(target: &Value, template: &Value) -> (Value, Vec<String>) {
    let mut merged = target.clone();
    let preserved = collect_foreign_claude_settings(&merged);

    let Some(template_hooks) = template.get("hooks").and_then(Value::as_object) else {
        return (merged, preserved);
    };

    let target_hooks = merged
        .as_object_mut()
        .and_then(|root| {
            if !root.contains_key("hooks") {
                root.insert("hooks".into(), Value::Object(Map::new()));
            }
            root.get_mut("hooks").and_then(Value::as_object_mut)
        });

    let Some(target_hooks) = target_hooks else {
        return (merged, preserved);
    };

    for (event, template_groups) in template_hooks {
        let template_groups = match template_groups {
            Value::Array(groups) => groups,
            _ => continue,
        };

        let slot = target_hooks
            .entry(event.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        let target_groups = match slot {
            Value::Array(groups) => groups,
            _ => {
                *slot = Value::Array(Vec::new());
                slot.as_array_mut().expect("array")
            }
        };

        for template_group in template_groups {
            let matcher = template_group
                .get("matcher")
                .and_then(Value::as_str)
                .unwrap_or("");
            let template_hooks = template_group
                .get("hooks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            let managed: Vec<Value> = template_hooks
                .into_iter()
                .filter(|hook| {
                    hook.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(is_managed_command)
                })
                .collect();

            if managed.is_empty() {
                continue;
            }

            if let Some(existing) = target_groups.iter_mut().find(|group| {
                group.get("matcher").and_then(Value::as_str).unwrap_or("") == matcher
            }) {
                let hooks_slot = existing
                    .as_object_mut()
                    .and_then(|obj| {
                        if !obj.contains_key("hooks") {
                            obj.insert("hooks".into(), Value::Array(Vec::new()));
                        }
                        obj.get_mut("hooks").and_then(Value::as_array_mut)
                    });
                if let Some(hooks_slot) = hooks_slot {
                    for hook in managed {
                        let command = hook
                            .get("command")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let exists = hooks_slot.iter().any(|entry| {
                            entry
                                .get("command")
                                .and_then(Value::as_str)
                                .is_some_and(|c| c == command)
                        });
                        if !exists {
                            hooks_slot.push(hook);
                        }
                    }
                }
            } else {
                let mut group = Map::new();
                if !matcher.is_empty() {
                    group.insert("matcher".into(), Value::String(matcher.into()));
                }
                group.insert("hooks".into(), Value::Array(managed));
                target_groups.push(Value::Object(group));
            }
        }
    }

    (merged, preserved)
}

fn collect_foreign_hooks_json(value: &Value) -> Vec<String> {
    let mut preserved = Vec::new();
    let Some(hooks) = value.get("hooks").and_then(Value::as_object) else {
        return preserved;
    };
    for (event, entries) in hooks {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            if let Some(command) = entry_command(entry) {
                if !is_managed_command(command) {
                    preserved.push(format!("{event}:{command}"));
                }
            }
        }
    }
    preserved
}

fn collect_foreign_claude_settings(value: &Value) -> Vec<String> {
    let mut preserved = Vec::new();
    let Some(hooks) = value.get("hooks").and_then(Value::as_object) else {
        return preserved;
    };
    for (event, groups) in hooks {
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for group in groups {
            let matcher = group.get("matcher").and_then(Value::as_str).unwrap_or("");
            if let Some(hooks) = group.get("hooks").and_then(Value::as_array) {
                for hook in hooks {
                    if let Some(command) = hook.get("command").and_then(Value::as_str) {
                        if !is_managed_command(command) {
                            preserved.push(format!("{event}:{matcher}:{command}"));
                        }
                    }
                }
            }
        }
    }
    preserved
}

pub fn remove_claude_settings(target: &Value) -> (Value, Vec<String>) {
    let mut merged = target.clone();
    let mut removed = Vec::new();

    let Some(target_hooks) = merged
        .as_object_mut()
        .and_then(|root| root.get_mut("hooks"))
        .and_then(Value::as_object_mut)
    else {
        return (merged, removed);
    };

    for (event, groups) in target_hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                hooks.retain(|hook| {
                    if hook
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(is_managed_command)
                    {
                        removed.push(event.clone());
                        false
                    } else {
                        true
                    }
                });
            }
        }
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .map(|hooks| !hooks.is_empty())
                .unwrap_or(false)
        });
    }

    target_hooks.retain(|_, groups| {
        groups
            .as_array()
            .map(|items| !items.is_empty())
            .unwrap_or(true)
    });

    (merged, removed)
}

fn entry_command(entry: &Value) -> Option<&str> {
    entry.get("command").and_then(Value::as_str).or_else(|| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .and_then(|hooks| hooks.first())
            .and_then(|hook| hook.get("command"))
            .and_then(Value::as_str)
    })
}

pub fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_preserves_foreign_hooks() {
        let target = json!({
            "version": 1,
            "hooks": {
                "beforeShellExecution": [{"command": "hooks/approve.sh"}]
            }
        });
        let template = json!({
            "hooks": {
                "sessionStart": [{"command": "sh wrapper --source cursor --vendor-event sessionStart llm-notch-hook-wrapper"}]
            }
        });
        let (merged, preserved) = merge_hooks_json(&target, &template);
        assert!(merged["hooks"]["beforeShellExecution"].is_array());
        assert!(merged["hooks"]["sessionStart"].is_array());
        assert_eq!(preserved.len(), 1);
    }

    #[test]
    fn merge_is_idempotent() {
        let target = json!({
            "hooks": {
                "sessionStart": [{"command": "llm-notch-hook --source cursor"}]
            }
        });
        let template = json!({
            "hooks": {
                "sessionStart": [{"command": "llm-notch-hook --source cursor"}]
            }
        });
        let (merged, _) = merge_hooks_json(&target, &template);
        assert_eq!(merged, target);
    }

    #[test]
    fn remove_only_managed_entries() {
        let target = json!({
            "hooks": {
                "beforeShellExecution": [{"command": "hooks/approve.sh"}],
                "sessionStart": [{"command": "llm-notch-hook --source cursor"}]
            }
        });
        let (merged, removed) = remove_hooks_json(&target);
        assert!(merged["hooks"]["beforeShellExecution"].is_array());
        assert!(merged["hooks"].get("sessionStart").is_none());
        assert!(!removed.is_empty());
    }
}
