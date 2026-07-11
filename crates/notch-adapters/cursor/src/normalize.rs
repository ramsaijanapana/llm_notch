//! Cursor hook payload normalization with redaction.

use notch_protocol::{
    AgentSource, AttentionKind, EventLevel, MAX_EVENT_SUMMARY_LEN, MAX_TOOL_NAME_LEN,
    SessionEventKind, SessionStatus,
};
use serde_json::Value;
use thiserror::Error;

/// Normalized Cursor hook event ready for protocol ingest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCursorEvent {
    pub source: AgentSource,
    pub ingest_event: String,
    pub external_session_id: String,
    pub session_event_kind: SessionEventKind,
    pub level: EventLevel,
    pub summary: String,
    pub tool_name: Option<String>,
    pub workspace_label: Option<String>,
    pub status: Option<SessionStatus>,
    pub attention: Option<AttentionKind>,
    pub occurred_at_ms: Option<i64>,
    pub vendor_event: String,
    pub cursor_version: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CursorNormalizeError {
    #[error("vendor payload must be a JSON object")]
    NotObject,
    #[error("vendor session identifier missing")]
    MissingSessionId,
    #[error("summary exceeds protocol bounds")]
    SummaryTooLong,
    #[error("tool name exceeds protocol bounds")]
    ToolNameTooLong,
}

/// Maps a Cursor hook payload to a redacted protocol event.
///
/// Prompts, command bodies, tool output, agent messages, and transcript paths are
/// never copied into summaries.
pub fn normalize_event(
    vendor_event: &str,
    payload: &Value,
    now_ms: i64,
) -> Result<NormalizedCursorEvent, CursorNormalizeError> {
    let object = payload.as_object().ok_or(CursorNormalizeError::NotObject)?;

    let external_session_id = session_id(object).ok_or(CursorNormalizeError::MissingSessionId)?;
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("toolName"))
        .and_then(Value::as_str)
        .map(redact_tool_name)
        .transpose()?;

    let workspace_label = workspace_label_from(object);
    let occurred_at_ms = object
        .get("occurred_at_ms")
        .or_else(|| object.get("occurredAtMs"))
        .or_else(|| object.get("timestamp_ms"))
        .or_else(|| object.get("timestampMs"))
        .and_then(Value::as_i64)
        .or(Some(now_ms));

    let cursor_version = object
        .get("cursor_version")
        .or_else(|| object.get("cursorVersion"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let normalized_vendor = normalize_vendor_event_name(vendor_event);
    let (ingest_event, session_event_kind, level, summary, status, attention) =
        map_vendor_event(&normalized_vendor, tool_name.as_deref(), object)?;

    let summary = bounded_summary(summary)?;

    Ok(NormalizedCursorEvent {
        source: AgentSource::Cursor,
        ingest_event,
        external_session_id,
        session_event_kind,
        level,
        summary,
        tool_name,
        workspace_label,
        status,
        attention,
        occurred_at_ms,
        vendor_event: vendor_event.to_string(),
        cursor_version,
    })
}

fn session_id(object: &serde_json::Map<String, Value>) -> Option<String> {
    [
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(str::to_string)
}

fn workspace_label_from(object: &serde_json::Map<String, Value>) -> Option<String> {
    object
        .get("cwd")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .get("workspace_roots")
                .and_then(Value::as_array)
                .and_then(|roots| roots.first())
                .and_then(Value::as_str)
        })
        .and_then(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
        })
        .map(str::to_string)
}

fn normalize_vendor_event_name(vendor_event: &str) -> String {
    vendor_event
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn map_vendor_event(
    normalized_vendor: &str,
    tool_name: Option<&str>,
    object: &serde_json::Map<String, Value>,
) -> Result<
    (
        String,
        SessionEventKind,
        EventLevel,
        String,
        Option<SessionStatus>,
        Option<AttentionKind>,
    ),
    CursorNormalizeError,
> {
    Ok(match normalized_vendor {
        "sessionstart" => (
            "sessionStart".into(),
            SessionEventKind::Lifecycle,
            EventLevel::Info,
            "Session started".into(),
            Some(SessionStatus::Running),
            None,
        ),
        "sessionend" => {
            let reason = object
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("ended");
            (
                "sessionEnd".into(),
                SessionEventKind::Lifecycle,
                EventLevel::Info,
                format!("Session ended ({reason})"),
                Some(map_session_end_status(reason)),
                None,
            )
        }
        "stop" => {
            let status = object
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("completed");
            (
                "update".into(),
                SessionEventKind::Status,
                map_stop_level(status),
                format!("Agent turn {status}"),
                Some(map_stop_status(status)),
                None,
            )
        }
        "pretooluse" => (
            "tool".into(),
            SessionEventKind::Tool,
            EventLevel::Info,
            tool_summary("Tool requested", tool_name),
            None,
            None,
        ),
        "posttooluse" => (
            "tool".into(),
            SessionEventKind::Tool,
            EventLevel::Info,
            tool_summary("Tool completed", tool_name),
            None,
            None,
        ),
        "posttoolusefailure" => {
            let failure = object
                .get("failure_type")
                .or_else(|| object.get("failureType"))
                .and_then(Value::as_str)
                .unwrap_or("error");
            (
                "tool".into(),
                SessionEventKind::Tool,
                EventLevel::Warning,
                tool_summary(&format!("Tool failed ({failure})"), tool_name),
                None,
                None,
            )
        }
        "beforeshellexecution" | "beforemcpexecution" | "beforereadfile" | "beforesubmitprompt" => {
            (
                "tool".into(),
                SessionEventKind::Tool,
                EventLevel::Info,
                format!("Hook gate: {normalized_vendor}"),
                None,
                None,
            )
        }
        "afteragentresponse" | "afteragentthought" | "precompact" => (
            "lifecycle".into(),
            SessionEventKind::Lifecycle,
            EventLevel::Debug,
            format!("Agent signal: {normalized_vendor}"),
            None,
            None,
        ),
        _ => (
            "lifecycle".into(),
            SessionEventKind::Lifecycle,
            EventLevel::Info,
            format!("Cursor hook: {normalized_vendor}"),
            None,
            None,
        ),
    })
}

fn tool_summary(prefix: &str, tool_name: Option<&str>) -> String {
    match tool_name {
        Some(name) => format!("{prefix}: {name}"),
        None => prefix.to_string(),
    }
}

fn map_session_end_status(reason: &str) -> SessionStatus {
    match reason {
        "error" => SessionStatus::Failed,
        "aborted" | "user_close" | "window_close" => SessionStatus::Completed,
        _ => SessionStatus::Completed,
    }
}

fn map_stop_status(status: &str) -> SessionStatus {
    match status {
        "error" => SessionStatus::Failed,
        "aborted" => SessionStatus::Waiting,
        _ => SessionStatus::Running,
    }
}

fn map_stop_level(status: &str) -> EventLevel {
    match status {
        "error" => EventLevel::Error,
        "aborted" => EventLevel::Warning,
        _ => EventLevel::Info,
    }
}

fn redact_tool_name(raw: &str) -> Result<String, CursorNormalizeError> {
    if raw.len() > MAX_TOOL_NAME_LEN {
        return Err(CursorNormalizeError::ToolNameTooLong);
    }
    Ok(raw.to_string())
}

fn bounded_summary(summary: String) -> Result<String, CursorNormalizeError> {
    if summary.len() > MAX_EVENT_SUMMARY_LEN {
        return Err(CursorNormalizeError::SummaryTooLong);
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(name: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../integrations/fixtures/cursor")
            .join(name);
        let text = fs::read_to_string(path).expect("fixture");
        serde_json::from_str(&text).expect("json")
    }

    #[test]
    fn normalizes_session_start_fixture() {
        let payload = fixture("session-start-input.json");
        let event = normalize_event("sessionStart", &payload, 1_700_000_000_000).expect("ok");
        assert_eq!(event.external_session_id, "cursor-session-42");
        assert_eq!(event.session_event_kind, SessionEventKind::Lifecycle);
        assert_eq!(event.summary, "Session started");
        assert!(!event.summary.contains("composer"));
    }

    #[test]
    fn normalizes_pre_tool_use_without_command_body() {
        let payload = fixture("pre-tool-use-input.json");
        let event = normalize_event("preToolUse", &payload, 1_700_000_000_000).expect("ok");
        assert_eq!(event.tool_name.as_deref(), Some("Shell"));
        assert_eq!(event.summary, "Tool requested: Shell");
        assert!(!event.summary.contains("cargo test"));
    }

    #[test]
    fn normalizes_post_tool_use_without_output() {
        let payload = fixture("post-tool-use-input.json");
        let event = normalize_event("postToolUse", &payload, 1_700_000_000_000).expect("ok");
        assert_eq!(event.summary, "Tool completed: Shell");
        assert!(!event.summary.contains("running 4 tests"));
    }

    #[test]
    fn normalizes_post_tool_use_failure() {
        let payload = fixture("post-tool-use-failure-input.json");
        let event = normalize_event("postToolUseFailure", &payload, 1_700_000_000_000).expect("ok");
        assert_eq!(event.level, EventLevel::Warning);
        assert!(event.summary.contains("timeout"));
    }

    #[test]
    fn normalizes_stop_fixture() {
        let payload = fixture("stop-input.json");
        let event = normalize_event("stop", &payload, 1_700_000_000_000).expect("ok");
        assert_eq!(event.session_event_kind, SessionEventKind::Status);
        assert_eq!(event.status, Some(SessionStatus::Running));
    }
}
