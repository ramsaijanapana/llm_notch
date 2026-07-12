//! Codex hooks.json template rendering with absolute-path placeholders.

/// Placeholder replaced with the signed helper binary after install review.
pub const HELPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_HELPER}}";

/// Placeholder replaced with the copied wrapper script after install review (Unix manual installs).
pub const WRAPPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_WRAPPER_ABSOLUTE_PATH}}";

const MANAGED_EVENTS: &[(&str, Option<&str>)] = &[
    ("SessionStart", Some("startup|resume")),
    ("PreToolUse", Some(".*")),
    ("PermissionRequest", Some(".*")),
    ("PostToolUse", Some(".*")),
    ("UserPromptSubmit", None),
    ("Stop", None),
];

/// Render a managed hook command for a Codex lifecycle event.
pub fn render_hook_command(vendor_event: &str) -> String {
    format!(
        "\"{HELPER_PATH_PLACEHOLDER}\" hook --source codex --vendor-event {vendor_event}"
    )
}

/// Build the shipped Codex `hooks.json` template with path placeholders.
pub fn template_hooks_json() -> serde_json::Value {
    let mut hooks = serde_json::Map::new();
    for (event, matcher) in MANAGED_EVENTS {
        let entry = if let Some(matcher) = matcher {
            serde_json::json!([{
                "matcher": matcher,
                "hooks": [managed_handler(event)]
            }])
        } else {
            serde_json::json!([{
                "hooks": [managed_handler(event)]
            }])
        };
        hooks.insert((*event).into(), entry);
    }

    serde_json::json!({
        "_comment": "TEMPLATE ONLY — requires explicit user trust via Codex /hooks. Enable with: codex -c features.hooks=true (features.codex_hooks is deprecated). {{LLM_NOTCH_HELPER}} is replaced with the bundled helper absolute path at install time.",
        "hooks": hooks
    })
}

fn managed_handler(vendor_event: &str) -> serde_json::Value {
    let status = match vendor_event {
        "SessionStart" => Some("llm_notch: session observe"),
        "PreToolUse" => Some("llm_notch: tool observe"),
        "PermissionRequest" => Some("llm_notch: permission observe"),
        "Stop" => Some("llm_notch: turn observe"),
        _ => None,
    };

    let timeout = if vendor_event == "PermissionRequest" {
        120
    } else {
        2
    };
    let mut handler = serde_json::json!({
        "type": "command",
        "command": render_hook_command(vendor_event),
        "timeout": timeout
    });
    if let Some(message) = status {
        handler["statusMessage"] = message.into();
    }
    handler
}

/// Inline TOML snippet for Codex config.toml `[hooks]` tables (documentation).
pub fn inline_hooks_toml_snippet() -> &'static str {
    r#"# Equivalent inline TOML (prefer hooks.json OR inline [hooks], not both)
[features]
hooks = true

[[hooks.SessionStart]]
matcher = "startup|resume"

[[hooks.SessionStart.hooks]]
type = "command"
command = "\"{{LLM_NOTCH_HELPER}}\" hook --source codex --vendor-event SessionStart"
timeout = 2
statusMessage = "llm_notch: session observe"
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_includes_permission_request_and_placeholders() {
        let template = template_hooks_json();
        let hooks = template
            .get("hooks")
            .and_then(|value| value.as_object())
            .expect("hooks");
        assert!(hooks.contains_key("PermissionRequest"));
        let encoded = template.to_string();
        assert!(encoded.contains(HELPER_PATH_PLACEHOLDER));
        assert!(!encoded.contains("sh "));
        assert!(encoded.contains("features.hooks"));
        assert!(!encoded.contains("codex_hooks=true"));
    }

    #[test]
    fn render_hook_command_uses_helper_placeholder() {
        let command = render_hook_command("Stop");
        assert!(command.contains(HELPER_PATH_PLACEHOLDER));
        assert!(!command.starts_with("sh "));
        assert!(command.contains("--vendor-event Stop"));
    }

    #[test]
    fn shipped_template_json_matches_rust_template_events() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/codex/hooks.json.template");
        let raw = std::fs::read_to_string(path).expect("read template json");
        let file: serde_json::Value = serde_json::from_str(&raw).expect("parse template json");
        assert_eq!(file["hooks"], template_hooks_json()["hooks"]);
    }
}
