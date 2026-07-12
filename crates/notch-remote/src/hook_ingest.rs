//! Normalize bounded hook ingest payloads into relay `SessionEvent` frames.

use notch_protocol::{AttentionKind, MAX_TOOL_NAME_LEN, SessionEventKind};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::{MAX_REMOTE_FRAME_BYTES, RelayPayload};

const MAX_SOURCE_LEN: usize = 32;
const MAX_EVENT_LEN: usize = 32;
const MAX_SESSION_ID_LEN: usize = 256;
const MAX_EVENT_SUMMARY_LEN: usize = 512;
const MAX_ATTENTION_LEN: usize = 32;

/// Bounded hook ingest payload accepted by the relay sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayHookPayload {
    pub source: String,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HookIngestError {
    #[error("hook payload field `{0}` is invalid")]
    InvalidField(&'static str),
    #[error("hook event `{0}` is not forwardable")]
    UnsupportedEvent(String),
    #[error("hook payload could not be normalized")]
    NotForwardable,
}

/// Returns `None` when the hook event is intentionally ignored (for example session remove).
pub fn normalize_hook_payload(
    payload: &RelayHookPayload,
    now_ms: i64,
) -> Result<Option<RelayPayload>, HookIngestError> {
    validate_hook_payload(payload)?;
    let event = payload.event.to_ascii_lowercase();
    if matches!(
        event.as_str(),
        "sessionremove" | "session_remove" | "remove"
    ) {
        return Ok(None);
    }
    if !is_forwardable_event(&event) {
        return Err(HookIngestError::UnsupportedEvent(event));
    }
    let session_id = resolve_session_id(payload)?;
    let kind = parse_event_kind(&event)?;
    let summary = payload
        .summary
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_summary(kind));
    let occurred_at_ms = payload.occurred_at_ms.unwrap_or(now_ms);
    let tool_name = payload.tool_name.clone().filter(|value| !value.is_empty());
    let attention = parse_attention(payload);
    let relay_payload = RelayPayload::SessionEvent {
        session_id,
        source: normalize_source(&payload.source),
        summary,
        occurred_at_ms,
        kind: Some(kind),
        tool_name,
        attention,
    };
    let frame = crate::protocol::RelayFrame {
        sequence: 1,
        payload: relay_payload.clone(),
    };
    let encoded = serde_json::to_vec(&frame).map_err(|_| HookIngestError::NotForwardable)?;
    if encoded.len() > MAX_REMOTE_FRAME_BYTES {
        return Err(HookIngestError::InvalidField("summary"));
    }
    Ok(Some(relay_payload))
}

pub fn validate_hook_payload(payload: &RelayHookPayload) -> Result<(), HookIngestError> {
    validate_bounded(&payload.source, MAX_SOURCE_LEN, "source")?;
    validate_bounded(&payload.event, MAX_EVENT_LEN, "event")?;
    if let Some(value) = &payload.session_id {
        validate_bounded(value, MAX_SESSION_ID_LEN, "sessionId")?;
    }
    if let Some(value) = &payload.external_session_id {
        validate_bounded(value, MAX_SESSION_ID_LEN, "externalSessionId")?;
    }
    if let Some(value) = &payload.summary {
        if value.len() > MAX_EVENT_SUMMARY_LEN {
            return Err(HookIngestError::InvalidField("summary"));
        }
    }
    if let Some(value) = &payload.tool_name {
        if value.is_empty() || value.len() > MAX_TOOL_NAME_LEN {
            return Err(HookIngestError::InvalidField("toolName"));
        }
    }
    if let Some(value) = &payload.attention {
        if value.is_empty() || value.len() > MAX_ATTENTION_LEN {
            return Err(HookIngestError::InvalidField("attention"));
        }
        if parse_attention(payload).is_none() {
            return Err(HookIngestError::InvalidField("attention"));
        }
    }
    Ok(())
}

fn is_forwardable_event(event: &str) -> bool {
    matches!(
        event,
        "sessionevent"
            | "event"
            | "tool"
            | "attention"
            | "status"
            | "statuschange"
            | "status_change"
            | "lifecycle"
            | "sessionstart"
            | "session_start"
            | "start"
            | "sessionend"
            | "session_end"
            | "end"
            | "complete"
            | "fail"
    )
}

fn resolve_session_id(payload: &RelayHookPayload) -> Result<String, HookIngestError> {
    payload
        .external_session_id
        .clone()
        .or_else(|| payload.session_id.clone())
        .filter(|value| !value.is_empty())
        .ok_or(HookIngestError::InvalidField("externalSessionId"))
}

fn parse_event_kind(event: &str) -> Result<SessionEventKind, HookIngestError> {
    match event {
        "tool" => Ok(SessionEventKind::Tool),
        "attention" => Ok(SessionEventKind::Attention),
        "status" | "statuschange" | "status_change" => Ok(SessionEventKind::Status),
        "lifecycle" | "sessionevent" | "event" | "sessionstart" | "session_start" | "start"
        | "sessionend" | "session_end" | "end" | "complete" | "fail" => {
            Ok(SessionEventKind::Lifecycle)
        }
        other => Err(HookIngestError::UnsupportedEvent(other.into())),
    }
}

fn parse_attention(payload: &RelayHookPayload) -> Option<AttentionKind> {
    payload
        .attention
        .as_ref()
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "none" => Some(AttentionKind::None),
            "approval" => Some(AttentionKind::Approval),
            "question" => Some(AttentionKind::Question),
            "permission" => Some(AttentionKind::Permission),
            "error" => Some(AttentionKind::Error),
            _ => None,
        })
}

fn normalize_source(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "cursor" => "cursor".into(),
        "claudecode" | "claude_code" | "claude-code" => "claudeCode".into(),
        "codex" => "codex".into(),
        "gemini" | "geminicli" | "gemini-cli" => "gemini".into(),
        "antigravitycli" | "antigravity-cli" | "antigravity" | "agy" => "antigravityCli".into(),
        "copilotcli" | "copilot-cli" | "copilot" => "copilotCli".into(),
        "qwen" | "qwen-cli" | "qwencode" => "qwen".into(),
        "generic" => "generic".into(),
        _ => "generic".into(),
    }
}

fn default_summary(kind: SessionEventKind) -> String {
    match kind {
        SessionEventKind::Lifecycle => "Session lifecycle update".into(),
        SessionEventKind::Tool => "Tool activity".into(),
        SessionEventKind::Attention => "Attention required".into(),
        SessionEventKind::Status => "Status update".into(),
    }
}

fn validate_bounded(value: &str, max: usize, field: &'static str) -> Result<(), HookIngestError> {
    if value.is_empty() || value.len() > max {
        Err(HookIngestError::InvalidField(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::AttentionKind;

    fn payload(event: &str) -> RelayHookPayload {
        RelayHookPayload {
            source: "codex".into(),
            event: event.into(),
            session_id: None,
            external_session_id: Some("remote-session-1".into()),
            summary: None,
            occurred_at_ms: Some(1_700_000_000_000),
            tool_name: None,
            attention: None,
        }
    }

    #[test]
    fn normalizes_tool_event_into_session_event_payload() {
        let relay = normalize_hook_payload(&payload("tool"), 0)
            .expect("normalize")
            .expect("forwardable");
        assert_eq!(
            relay,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Tool activity".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: Some(SessionEventKind::Tool),
                tool_name: None,
                attention: None,
            }
        );
    }

    #[test]
    fn normalizes_tool_event_with_tool_name() {
        let mut hook = payload("tool");
        hook.tool_name = Some("run_command".into());
        hook.summary = Some("Executed cargo test".into());
        let relay = normalize_hook_payload(&hook, 0)
            .expect("normalize")
            .expect("forwardable");
        assert_eq!(
            relay,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Executed cargo test".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: Some(SessionEventKind::Tool),
                tool_name: Some("run_command".into()),
                attention: None,
            }
        );
    }

    #[test]
    fn normalizes_attention_event_with_attention_kind() {
        let mut hook = payload("attention");
        hook.attention = Some("permission".into());
        hook.summary = Some("Approve file write".into());
        let relay = normalize_hook_payload(&hook, 0)
            .expect("normalize")
            .expect("forwardable");
        assert_eq!(
            relay,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Approve file write".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: Some(SessionEventKind::Attention),
                tool_name: None,
                attention: Some(AttentionKind::Permission),
            }
        );
    }

    #[test]
    fn attention_event_without_attention_field_forwards_kind_only() {
        let relay = normalize_hook_payload(&payload("attention"), 0)
            .expect("normalize")
            .expect("forwardable");
        assert_eq!(
            relay,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Attention required".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: Some(SessionEventKind::Attention),
                tool_name: None,
                attention: None,
            }
        );
    }

    #[test]
    fn session_remove_is_ignored_without_error() {
        assert_eq!(
            normalize_hook_payload(&payload("sessionRemove"), 0).expect("normalize"),
            None
        );
    }

    #[test]
    fn rejects_unknown_event_kinds() {
        assert_eq!(
            normalize_hook_payload(&payload("decisionWait"), 0),
            Err(HookIngestError::UnsupportedEvent("decisionwait".into()))
        );
    }

    #[test]
    fn rejects_invalid_attention_values() {
        let mut hook = payload("attention");
        hook.attention = Some("not-a-kind".into());
        assert_eq!(
            validate_hook_payload(&hook),
            Err(HookIngestError::InvalidField("attention"))
        );
    }

    #[test]
    fn normalizes_verified_catalog_source_aliases() {
        for (input, expected) in [
            ("antigravityCli", "antigravityCli"),
            ("antigravity-cli", "antigravityCli"),
            ("agy", "antigravityCli"),
            ("copilot", "copilotCli"),
            ("qwen", "qwen"),
        ] {
            let mut hook = payload("tool");
            hook.source = input.into();
            let relay = normalize_hook_payload(&hook, 0)
                .expect("normalize")
                .expect("forwardable");
            if let RelayPayload::SessionEvent { source, .. } = relay {
                assert_eq!(source, expected, "input `{input}`");
            } else {
                panic!("expected session event");
            }
        }
    }

    #[test]
    fn deserializes_legacy_hook_payload_without_optional_fields() {
        let json = r#"{
            "source":"codex",
            "event":"tool",
            "externalSessionId":"remote-session-1",
            "summary":"Legacy tool event",
            "occurredAtMs":1700000000000
        }"#;
        let hook: RelayHookPayload = serde_json::from_str(json).expect("deserialize");
        let relay = normalize_hook_payload(&hook, 0)
            .expect("normalize")
            .expect("forwardable");
        assert_eq!(
            relay,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Legacy tool event".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: Some(SessionEventKind::Tool),
                tool_name: None,
                attention: None,
            }
        );
    }
}
