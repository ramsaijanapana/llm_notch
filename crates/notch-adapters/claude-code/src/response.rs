//! Verified Claude Code hook response builders and fail-open neutral output.

use notch_protocol::{DECISION_HOOK_NEUTRAL_OUTPUT, DecisionResponseAction};
use serde_json::{Value, json};

/// Vendor hooks that have a documented allow/deny or approve response path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeRespondableHook {
    PermissionRequest,
    ExitPlanModePreToolUse,
}

/// Permission decision for Claude Code `PermissionRequest` hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudePermissionDecision {
    Allow,
    Deny {
        message: Option<String>,
        interrupt: bool,
    },
}

/// Fail-open neutral stdout for helper/wrapper paths when no decision is delivered.
pub fn hook_response() -> Value {
    serde_json::from_str(DECISION_HOOK_NEUTRAL_OUTPUT).expect("neutral hook output parses")
}

/// Build a verified `PermissionRequest` response. Never fabricates vendor success metadata.
pub fn build_permission_response(
    decision: ClaudePermissionDecision,
) -> Result<Value, ResponseBuildError> {
    let behavior = match &decision {
        ClaudePermissionDecision::Allow => "allow",
        ClaudePermissionDecision::Deny { .. } => "deny",
    };
    let mut decision_object = json!({ "behavior": behavior });
    if let ClaudePermissionDecision::Deny { message, interrupt } = decision {
        if let Some(message) = message {
            decision_object["message"] = json!(message);
        }
        if interrupt {
            decision_object["interrupt"] = json!(true);
        }
    }
    Ok(json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": decision_object,
        }
    }))
}

/// Build a verified `ExitPlanMode` approval via `PreToolUse` decision control.
///
/// Claude Code requires `updatedInput` alongside `permissionDecision: "allow"` for
/// interactive tools such as `ExitPlanMode`.
pub fn build_exit_plan_approve_response(updated_input: Value) -> Result<Value, ResponseBuildError> {
    if !updated_input.is_object() {
        return Err(ResponseBuildError::ExitPlanUpdatedInputMustBeObject);
    }
    Ok(json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": updated_input,
        }
    }))
}

/// Map a broker action to a verified Claude Code response when the hook supports it.
pub fn build_decision_response(
    hook: ClaudeRespondableHook,
    action: DecisionResponseAction,
    updated_input: Option<Value>,
) -> Result<Value, ResponseBuildError> {
    match (hook, action) {
        (ClaudeRespondableHook::PermissionRequest, DecisionResponseAction::Allow) => {
            build_permission_response(ClaudePermissionDecision::Allow)
        }
        (ClaudeRespondableHook::PermissionRequest, DecisionResponseAction::Deny) => {
            build_permission_response(ClaudePermissionDecision::Deny {
                message: Some("Denied by llm_notch".into()),
                interrupt: false,
            })
        }
        (ClaudeRespondableHook::ExitPlanModePreToolUse, DecisionResponseAction::Allow) => {
            build_exit_plan_approve_response(updated_input.unwrap_or_else(|| json!({})))
        }
        (ClaudeRespondableHook::ExitPlanModePreToolUse, DecisionResponseAction::Deny) => {
            Ok(json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Plan not approved",
                }
            }))
        }
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResponseBuildError {
    #[error("ExitPlanMode approval requires updatedInput to be a JSON object")]
    ExitPlanUpdatedInputMustBeObject,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_response_is_fail_open_neutral_json() {
        assert_eq!(hook_response(), json!({}));
    }

    #[test]
    fn permission_allow_uses_documented_shape() {
        let response = build_permission_response(ClaudePermissionDecision::Allow).expect("build");
        assert_eq!(
            response["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(
            response["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
    }

    #[test]
    fn permission_deny_includes_message() {
        let response = build_permission_response(ClaudePermissionDecision::Deny {
            message: Some("Blocked".into()),
            interrupt: false,
        })
        .expect("build");
        assert_eq!(
            response["hookSpecificOutput"]["decision"]["behavior"],
            "deny"
        );
        assert_eq!(
            response["hookSpecificOutput"]["decision"]["message"],
            "Blocked"
        );
    }

    #[test]
    fn exit_plan_approve_requires_updated_input_object() {
        let err = build_exit_plan_approve_response(json!("nope")).unwrap_err();
        assert_eq!(err, ResponseBuildError::ExitPlanUpdatedInputMustBeObject);

        let response = build_exit_plan_approve_response(json!({})).expect("build");
        assert_eq!(
            response["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert!(response["hookSpecificOutput"]["updatedInput"].is_object());
    }

    #[test]
    fn broker_action_maps_only_to_verified_hooks() {
        let permission = build_decision_response(
            ClaudeRespondableHook::PermissionRequest,
            DecisionResponseAction::Allow,
            None,
        )
        .expect("permission");
        assert_eq!(
            permission["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );

        let plan = build_decision_response(
            ClaudeRespondableHook::ExitPlanModePreToolUse,
            DecisionResponseAction::Allow,
            None,
        )
        .expect("plan");
        assert_eq!(plan["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    }
}
