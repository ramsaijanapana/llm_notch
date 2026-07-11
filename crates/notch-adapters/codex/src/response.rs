//! Codex hook stdout responses for permission and lifecycle events.
//!
//! Shipped llm_notch templates are observation-only: they always fail open with `{}`.
//! This module documents Codex-supported decision shapes for future broker wiring.

use serde_json::{Value, json};

/// Codex permission decision behavior documented at developers.openai.com/codex/hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexPermissionBehavior {
    Allow,
    Deny,
}

/// Hook events where Codex accepts structured stdout responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexRespondableHook {
    PermissionRequest,
    PreToolUse,
    Stop,
}

/// Build a documented Codex `PermissionRequest` response.
///
/// llm_notch V1 templates never emit this — observation only.
pub fn build_permission_response(behavior: CodexPermissionBehavior) -> Value {
    let behavior_str = match behavior {
        CodexPermissionBehavior::Allow => "allow",
        CodexPermissionBehavior::Deny => "deny",
    };
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {
                "behavior": behavior_str
            }
        }
    })
}

/// Fail-open hook stdout for observation-only integrations.
pub fn hook_response(_hook: CodexRespondableHook) -> Value {
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_template_returns_empty_object() {
        assert_eq!(
            hook_response(CodexRespondableHook::PermissionRequest),
            json!({})
        );
    }

    #[test]
    fn documented_allow_shape_matches_codex_hooks() {
        let value = build_permission_response(CodexPermissionBehavior::Allow);
        assert_eq!(value["hookSpecificOutput"]["decision"]["behavior"], "allow");
    }

    #[test]
    fn documented_deny_shape_matches_codex_hooks() {
        let value = build_permission_response(CodexPermissionBehavior::Deny);
        assert_eq!(value["hookSpecificOutput"]["decision"]["behavior"], "deny");
    }
}
