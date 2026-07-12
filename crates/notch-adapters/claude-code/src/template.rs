//! Claude Code settings hooks template with connector placeholder paths.

use serde_json::{Value, json};

/// Absolute bundled helper path substituted by the connector at apply time.
pub const HELPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_HELPER}}";

/// Absolute wrapper script path substituted by the connector at apply time.
pub const WRAPPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_WRAPPER}}";

const MANAGED_SOURCE: &str = "claudeCode";

/// Renders the managed hook command for a Claude Code vendor event.
pub fn render_hook_command(vendor_event: &str) -> String {
    format!(
        "\"{HELPER_PATH_PLACEHOLDER}\" hook --source {MANAGED_SOURCE} --vendor-event {vendor_event} --hook-mode"
    )
}

/// Returns the hooks fragment merged into `.claude/settings.json` or `~/.claude/settings.json`.
pub fn template_settings_hooks() -> Value {
    json!({
        "SessionStart": [matcher_group("startup|resume", "SessionStart")],
        "PreToolUse": [matcher_group(".*", "PreToolUse")],
        "PostToolUse": [matcher_group(".*", "PostToolUse")],
        "PostToolUseFailure": [matcher_group(".*", "PostToolUseFailure")],
        "PermissionRequest": [matcher_group(".*", "PermissionRequest")],
        "Stop": [stop_group("Stop")],
        "SessionEnd": [stop_group("SessionEnd")],
    })
}

fn matcher_group(matcher: &str, vendor_event: &str) -> Value {
    json!({
        "matcher": matcher,
        "hooks": [command_handler(vendor_event)],
    })
}

fn stop_group(vendor_event: &str) -> Value {
    json!({
        "hooks": [command_handler(vendor_event)],
    })
}

fn command_handler(vendor_event: &str) -> Value {
    let timeout = if vendor_event == "PermissionRequest" {
        120
    } else {
        2
    };
    json!({
        "type": "command",
        "command": render_hook_command(vendor_event),
        "timeout": timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_uses_helper_placeholder() {
        let command = render_hook_command("SessionStart");
        assert!(command.contains(HELPER_PATH_PLACEHOLDER));
        assert!(command.contains("--source claudeCode"));
        assert!(command.contains("--vendor-event SessionStart"));
    }

    #[test]
    fn shipped_template_json_matches_rust_template_events() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/claude-code/settings.hooks.template.json");
        let raw = std::fs::read_to_string(path).expect("read template json");
        let file: serde_json::Value = serde_json::from_str(&raw).expect("parse template json");
        assert_eq!(file["hooks"], template_settings_hooks());
    }
}
