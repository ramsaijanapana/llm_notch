//! Vendor stdout builders gated by adapter capabilities.

use notch_adapters_claude_code::{
    ClaudeRespondableHook, build_decision_response, capabilities, detect_version,
};
use notch_protocol::{
    AgentSource, DECISION_HOOK_NEUTRAL_OUTPUT, DecisionResponse, DecisionResponseAction,
};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdapterBuildError {
    #[error("decision responses are disabled for this adapter")]
    CapabilityDisabled,
    #[error("unsupported respondable hook `{0}`")]
    UnsupportedHook(String),
    #[error("question answers are not supported in protocol v1")]
    QuestionUnsupported,
    #[error("vendor response build failed: {0}")]
    BuildFailed(String),
}

pub fn build_vendor_stdout(
    source: AgentSource,
    respondable_hook: Option<&str>,
    response: &DecisionResponse,
    vendor_context: Option<&Value>,
) -> Result<String, AdapterBuildError> {
    let DecisionResponse::Action { action } = response else {
        return Err(AdapterBuildError::QuestionUnsupported);
    };

    match source {
        AgentSource::ClaudeCode => build_claude_stdout(respondable_hook, *action, vendor_context),
        AgentSource::Cursor | AgentSource::Codex | AgentSource::Generic | AgentSource::Unknown => {
            Err(AdapterBuildError::CapabilityDisabled)
        }
    }
}

fn build_claude_stdout(
    respondable_hook: Option<&str>,
    action: DecisionResponseAction,
    vendor_context: Option<&Value>,
) -> Result<String, AdapterBuildError> {
    let hook = match respondable_hook {
        Some("permissionRequest") => ClaudeRespondableHook::PermissionRequest,
        Some("exitPlanModePreToolUse") => ClaudeRespondableHook::ExitPlanModePreToolUse,
        Some(other) => return Err(AdapterBuildError::UnsupportedHook(other.into())),
        None => return Err(AdapterBuildError::CapabilityDisabled),
    };

    let version = vendor_context
        .and_then(|value| {
            value
                .get("claude_code_version")
                .or_else(|| value.get("claudeCodeVersion"))
                .and_then(Value::as_str)
        });
    let profile = detect_version(version);
    let caps = capabilities(&profile);
    if !caps.respond_decisions && !caps.decision_response {
        return Err(AdapterBuildError::CapabilityDisabled);
    }

    let updated_input = vendor_context
        .and_then(|value| value.get("updated_input").or_else(|| value.get("updatedInput")))
        .cloned();

    let value = build_decision_response(hook, action, updated_input)
        .map_err(|err| AdapterBuildError::BuildFailed(err.to_string()))?;
    serde_json::to_string(&value).map_err(|err| AdapterBuildError::BuildFailed(err.to_string()))
}

pub fn neutral_stdout() -> String {
    DECISION_HOOK_NEUTRAL_OUTPUT.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_permission_allow_builds_documented_shape() {
        let stdout = build_vendor_stdout(
            AgentSource::ClaudeCode,
            Some("permissionRequest"),
            &DecisionResponse::Action {
                action: DecisionResponseAction::Allow,
            },
            Some(&json!({"claude_code_version": "2.1.205"})),
        )
        .expect("stdout");
        let parsed: Value = serde_json::from_str(&stdout).expect("json");
        assert_eq!(
            parsed["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
    }

    #[test]
    fn cursor_capability_disabled_returns_error() {
        let err = build_vendor_stdout(
            AgentSource::Cursor,
            Some("permissionRequest"),
            &DecisionResponse::Action {
                action: DecisionResponseAction::Allow,
            },
            None,
        )
        .unwrap_err();
        assert_eq!(err, AdapterBuildError::CapabilityDisabled);
    }
}
