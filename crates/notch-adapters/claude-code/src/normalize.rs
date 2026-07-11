//! Normalize Claude Code hook payloads into bounded protocol ingest fields.

use notch_protocol::{AgentSource, DecisionKind};
use serde_json::Value;
use thiserror::Error;

/// Normalized Claude Code hook event safe for ingest and decision brokering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedClaudeEvent {
    pub source: AgentSource,
    pub vendor_event: String,
    pub protocol_event: String,
    pub external_session_id: String,
    pub workspace_label: Option<String>,
    pub status: Option<String>,
    pub attention: Option<String>,
    pub summary: String,
    pub tool_name: Option<String>,
    pub decision_kind: Option<DecisionKind>,
    pub respondable_hook: Option<super::response::ClaudeRespondableHook>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClaudeNormalizeError {
    #[error("vendor payload must be a JSON object")]
    NotObject,
    #[error("vendor session identifier missing")]
    MissingSessionId,
    #[error("unsupported vendor event `{0}`")]
    UnsupportedEvent(String),
}

/// Normalize a Claude Code hook payload with default redaction.
///
/// Raw prompts, command bodies, tool output, transcript paths, and plan content
/// are never copied into the normalized event.
pub fn normalize_event(
    vendor_event: &str,
    payload: &Value,
) -> Result<NormalizedClaudeEvent, ClaudeNormalizeError> {
    let object = payload.as_object().ok_or(ClaudeNormalizeError::NotObject)?;
    let external_session_id = ["session_id", "sessionId"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::to_string)
        .ok_or(ClaudeNormalizeError::MissingSessionId)?;
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("toolName"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let workspace_label = object
        .get("cwd")
        .and_then(Value::as_str)
        .and_then(workspace_basename);
    let normalized_vendor = normalize_vendor_event_name(vendor_event);

    let (protocol_event, status, attention, summary, decision_kind, respondable_hook) =
        match normalized_vendor.as_str() {
            "sessionstart" => (
                "sessionStart".into(),
                Some("running".into()),
                None,
                "Session started".into(),
                None,
                None,
            ),
            "sessionend" => (
                "sessionEnd".into(),
                Some("completed".into()),
                None,
                "Session ended".into(),
                None,
                None,
            ),
            "stop" => (
                "update".into(),
                Some("waiting".into()),
                None,
                "Agent turn completed".into(),
                None,
                None,
            ),
            "permissionrequest" => (
                "attention".into(),
                None,
                Some("permission".into()),
                permission_summary(tool_name.as_deref()),
                Some(DecisionKind::Permission),
                Some(super::response::ClaudeRespondableHook::PermissionRequest),
            ),
            "pretooluse" if tool_name.as_deref() == Some("ExitPlanMode") => (
                "attention".into(),
                None,
                Some("approval".into()),
                "Plan approval requested".into(),
                Some(DecisionKind::Approval),
                Some(super::response::ClaudeRespondableHook::ExitPlanModePreToolUse),
            ),
            "pretooluse" => (
                "tool".into(),
                None,
                None,
                "Tool activity observed".into(),
                None,
                None,
            ),
            "posttooluse" => (
                "tool".into(),
                None,
                None,
                "Tool activity observed".into(),
                None,
                None,
            ),
            "posttoolusefailure" => (
                "tool".into(),
                None,
                None,
                "Tool activity failed".into(),
                None,
                None,
            ),
            other if other.is_empty() => {
                return Err(ClaudeNormalizeError::UnsupportedEvent(vendor_event.into()));
            }
            _ => (
                "lifecycle".into(),
                None,
                None,
                "Agent lifecycle event observed".into(),
                None,
                None,
            ),
        };

    Ok(NormalizedClaudeEvent {
        source: AgentSource::ClaudeCode,
        vendor_event: normalized_vendor,
        protocol_event,
        external_session_id,
        workspace_label,
        status,
        attention,
        summary,
        tool_name,
        decision_kind,
        respondable_hook,
    })
}

fn normalize_vendor_event_name(raw: &str) -> String {
    raw.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn workspace_basename(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

fn permission_summary(tool_name: Option<&str>) -> String {
    match tool_name {
        Some(name) => format!("Permission request for {name}"),
        None => "Permission request observed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(name: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/fixtures/claude-code")
            .join(name);
        let raw = fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn session_start_fixture_normalizes_without_raw_paths() {
        let payload = fixture("session-start-input.json");
        let normalized = normalize_event("SessionStart", &payload).expect("normalize");
        assert_eq!(normalized.protocol_event, "sessionStart");
        assert_eq!(normalized.external_session_id, "claude-thread-9f2a");
        assert_eq!(normalized.workspace_label.as_deref(), Some("llm_notch"));
        assert!(!normalized.summary.contains("transcript"));
    }

    #[test]
    fn permission_request_fixture_maps_to_attention_and_decision_kind() {
        let payload = fixture("permission-request-input.json");
        let normalized = normalize_event("PermissionRequest", &payload).expect("normalize");
        assert_eq!(normalized.protocol_event, "attention");
        assert_eq!(normalized.attention.as_deref(), Some("permission"));
        assert_eq!(normalized.decision_kind, Some(DecisionKind::Permission));
        assert_eq!(
            normalized.respondable_hook,
            Some(super::super::response::ClaudeRespondableHook::PermissionRequest)
        );
        assert!(!normalized.summary.contains("npm run build"));
    }

    #[test]
    fn exit_plan_mode_fixture_maps_to_approval_attention() {
        let payload = fixture("exit-plan-mode-pretooluse-input.json");
        let normalized = normalize_event("PreToolUse", &payload).expect("normalize");
        assert_eq!(normalized.protocol_event, "attention");
        assert_eq!(normalized.attention.as_deref(), Some("approval"));
        assert_eq!(normalized.decision_kind, Some(DecisionKind::Approval));
        assert_eq!(
            normalized.respondable_hook,
            Some(super::super::response::ClaudeRespondableHook::ExitPlanModePreToolUse)
        );
        assert!(!normalized.summary.contains("Refactor auth"));
    }

    #[test]
    fn ordinary_pre_tool_use_stays_tool_observation() {
        let payload = fixture("pre-tool-use-input.json");
        let normalized = normalize_event("PreToolUse", &payload).expect("normalize");
        assert_eq!(normalized.protocol_event, "tool");
        assert!(normalized.decision_kind.is_none());
        assert_eq!(normalized.tool_name.as_deref(), Some("Bash"));
    }
}
