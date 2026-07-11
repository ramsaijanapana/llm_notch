//! Permission and hook stdout response builders.

use notch_protocol::AdapterCapabilities;
use serde_json::{Value, json};

/// Hooks where Cursor docs verify allow/deny JSON on stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorRespondableHook {
    PreToolUse,
    BeforeShellExecution,
    BeforeMcpExecution,
    BeforeReadFile,
    SubagentStart,
}

impl CursorRespondableHook {
    pub fn vendor_name(self) -> &'static str {
        match self {
            Self::PreToolUse => "preToolUse",
            Self::BeforeShellExecution => "beforeShellExecution",
            Self::BeforeMcpExecution => "beforeMCPExecution",
            Self::BeforeReadFile => "beforeReadFile",
            Self::SubagentStart => "subagentStart",
        }
    }

    pub fn from_vendor_event(vendor_event: &str) -> Option<Self> {
        let normalized: String = vendor_event
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        match normalized.as_str() {
            "pretooluse" => Some(Self::PreToolUse),
            "beforeshellexecution" => Some(Self::BeforeShellExecution),
            "beforemcpexecution" => Some(Self::BeforeMcpExecution),
            "beforereadfile" => Some(Self::BeforeReadFile),
            "subagentstart" => Some(Self::SubagentStart),
            _ => None,
        }
    }
}

/// Permission decision supported by Cursor hook stdout (allow/deny only in V1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPermissionDecision {
    Allow,
    Deny,
}

/// Builds a verified Cursor permission JSON response.
///
/// Returns `None` when capabilities disable decision response (V1 default).
pub fn build_permission_response(
    hook: CursorRespondableHook,
    decision: CursorPermissionDecision,
    capabilities: &AdapterCapabilities,
) -> Option<Value> {
    if !capabilities.respond_decisions && !capabilities.decision_response {
        return None;
    }

    let permission = match decision {
        CursorPermissionDecision::Allow => "allow",
        CursorPermissionDecision::Deny => "deny",
    };

    Some(match hook {
        CursorRespondableHook::PreToolUse => json!({ "permission": permission }),
        CursorRespondableHook::BeforeShellExecution
        | CursorRespondableHook::BeforeMcpExecution
        | CursorRespondableHook::BeforeReadFile
        | CursorRespondableHook::SubagentStart => json!({
            "continue": true,
            "permission": permission,
        }),
    })
}

/// Fail-open hook stdout for observation-only templates.
///
/// V1 always returns `{}` regardless of vendor event. Question/plan responses are
/// intentionally unsupported.
pub fn hook_response(vendor_event: &str, capabilities: &AdapterCapabilities) -> Value {
    let _ = vendor_event;
    let _ = capabilities;
    json!({})
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{AdapterCapabilities, AgentSource};

    #[test]
    fn v1_hook_response_is_always_empty_object() {
        let caps = AdapterCapabilities::template(AgentSource::Cursor);
        assert_eq!(hook_response("preToolUse", &caps), json!({}));
        assert_eq!(hook_response("beforeShellExecution", &caps), json!({}));
    }

    #[test]
    fn permission_response_disabled_in_v1() {
        let caps = AdapterCapabilities::template(AgentSource::Cursor);
        assert!(build_permission_response(
            CursorRespondableHook::PreToolUse,
            CursorPermissionDecision::Deny,
            &caps,
        )
        .is_none());
    }

    #[test]
    fn permission_response_available_when_capability_enabled() {
        let mut caps = AdapterCapabilities::template(AgentSource::Cursor);
        caps.respond_decisions = true;
        let value = build_permission_response(
            CursorRespondableHook::BeforeShellExecution,
            CursorPermissionDecision::Allow,
            &caps,
        )
        .expect("response");
        assert_eq!(value["permission"], "allow");
        assert_eq!(value["continue"], true);
    }

    #[test]
    fn maps_vendor_events_to_respondable_hooks() {
        assert_eq!(
            CursorRespondableHook::from_vendor_event("preToolUse"),
            Some(CursorRespondableHook::PreToolUse)
        );
        assert!(CursorRespondableHook::from_vendor_event("sessionStart").is_none());
    }
}
