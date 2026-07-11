//! Template placeholders and rendered hook commands.

use crate::merge::{MANAGED_EVENTS, MergeScope};
use serde_json::{Value, json};

/// Placeholder substituted by the connector installer with the absolute helper path.
///
/// Example resolved value (macOS):
/// `/Applications/llm_notch.app/Contents/MacOS/llm-notch-hook`
///
/// The connector lane replaces this token in preview/apply; hooks must never rely on PATH.
pub const HELPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_HELPER}}";

/// Placeholder substituted with the absolute wrapper script path when the installer
/// chooses wrapper-based invocation (timeout fail-open outside Cursor's hook timeout).
pub const WRAPPER_PATH_PLACEHOLDER: &str = "{{LLM_NOTCH_WRAPPER}}";

/// Renders a direct helper invocation for installed hooks.
///
/// Cursor's per-entry `timeout` provides bounded execution; the helper always fails open
/// in `--hook-mode`.
pub fn render_hook_command(_scope: MergeScope, vendor_event: &str, helper_path: &str) -> String {
    format!(
        "\"{helper_path}\" hook --source cursor --vendor-event {vendor_event} --hook-mode"
    )
}

/// Renders a wrapper-based command for templates checked into a repository.
pub fn render_wrapper_command(vendor_event: &str, wrapper_path: &str) -> String {
    format!("sh \"{wrapper_path}\" --source cursor --vendor-event {vendor_event}")
}

/// Builds the full hooks.json template object for preview/apply.
pub fn template_hooks_json(scope: MergeScope, helper_path: &str) -> Value {
    let mut hooks = serde_json::Map::new();
    for event in MANAGED_EVENTS {
        hooks.insert(
            (*event).to_string(),
            json!([{
                "command": render_hook_command(scope, event, helper_path),
                "timeout": 2,
            }]),
        );
    }

    json!({
        "version": 1,
        "_comment": "TEMPLATE ONLY — llm_notch Cursor observation hooks. Review diff before applying.",
        "hooks": Value::Object(hooks),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_uses_helper_placeholder() {
        let value = template_hooks_json(MergeScope::Project, HELPER_PATH_PLACEHOLDER);
        let command = value["hooks"]["sessionStart"][0]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains(HELPER_PATH_PLACEHOLDER));
        assert!(command.contains("--vendor-event sessionStart"));
        assert!(command.contains("--hook-mode"));
    }
}
