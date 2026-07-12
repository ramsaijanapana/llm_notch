//! Normalize Codex hook JSON into bounded ingest fields with redaction.

use notch_ipc::IngestPayload;
use serde_json::Value;
use thiserror::Error;

use crate::version::{CodexVersionProfile, detect_version};

/// Normalized Codex hook output consumed by connector and ingest pipelines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCodexEvent {
    pub payload: IngestPayload,
    pub profile: CodexVersionProfile,
    pub vendor_event: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodexNormalizeError {
    #[error("vendor payload must be a JSON object")]
    NotObject,
    #[error("vendor session identifier missing")]
    SessionMissing,
    #[error("forbidden vendor field `{0}`")]
    ForbiddenField(String),
}

const FORBIDDEN_KEYS: &[&str] = &[
    "tool_input",
    "toolInput",
    "tool_output",
    "toolOutput",
    "transcript_path",
    "transcriptPath",
    "last_assistant_message",
    "lastAssistantMessage",
    "prompt",
    "command",
    "stdout",
    "stderr",
    "message",
    "messages",
    "content",
    "text",
    "raw",
    "data",
];

/// Strip sensitive Codex hook fields before logging or fixture comparison.
pub fn redact_vendor_json(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut redacted = serde_json::Map::new();
    for (key, field) in object {
        if FORBIDDEN_KEYS
            .iter()
            .any(|forbidden| forbidden.eq_ignore_ascii_case(key))
        {
            continue;
        }
        redacted.insert(key.clone(), field.clone());
    }
    Value::Object(redacted)
}

/// Map Codex vendor hook JSON to a bounded [`IngestPayload`].
pub fn normalize_event(
    vendor_event: &str,
    value: &Value,
) -> Result<NormalizedCodexEvent, CodexNormalizeError> {
    let redacted = redact_vendor_json(value);
    let object = redacted.as_object().ok_or(CodexNormalizeError::NotObject)?;

    let hook_event_name = object
        .get("hook_event_name")
        .or_else(|| object.get("hookEventName"))
        .and_then(Value::as_str);
    let profile = detect_version(vendor_event, hook_event_name);

    let external_session_id = ["session_id", "sessionId", "thread_id", "threadId"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::to_string)
        .ok_or(CodexNormalizeError::SessionMissing)?;

    let tool_name = ["tool_name", "toolName"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::to_string);

    let workspace_label = object
        .get("cwd")
        .and_then(Value::as_str)
        .and_then(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
        })
        .map(str::to_string);

    let occurred_at_ms = [
        "occurred_at_ms",
        "occurredAtMs",
        "timestamp_ms",
        "timestampMs",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_i64));

    let normalized_event = vendor_event
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    let (event, status, attention, summary) = map_vendor_event(&normalized_event, &profile);

    let payload = IngestPayload {
        source: "codex".into(),
        event: event.into(),
        session_id: None,
        external_session_id: Some(external_session_id),
        label: (event == "sessionStart").then(|| "codex session".into()),
        workspace_label,
        status: status.map(str::to_string),
        attention: attention.map(str::to_string),
        summary: summary.map(str::to_string),
        tool_name,
        pid: None,
        process_started_at_ms: None,
        occurred_at_ms,
        terminal_session_id: None,
        tab_id: None,
        pane_id: None,
        window_handle: None,
    };

    Ok(NormalizedCodexEvent {
        payload,
        profile,
        vendor_event: vendor_event.into(),
    })
}

fn map_vendor_event(
    normalized_event: &str,
    profile: &CodexVersionProfile,
) -> (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
) {
    if matches!(profile, CodexVersionProfile::NotifyFallback) {
        return (
            "update",
            Some("waiting"),
            None,
            Some("Agent turn completed (notify fallback)"),
        );
    }

    match normalized_event {
        "sessionstart" | "subagentstart" => (
            "sessionStart",
            Some("running"),
            None,
            Some("Session started"),
        ),
        "stop" | "subagentstop" => (
            "update",
            Some("waiting"),
            None,
            Some("Agent turn completed"),
        ),
        "permissionrequest" => (
            "attention",
            None,
            Some("permission"),
            Some("Permission request observed"),
        ),
        "pretooluse" | "posttooluse" => ("tool", None, None, Some("Tool activity observed")),
        "posttoolusefailure" => ("tool", None, None, Some("Tool activity failed")),
        "userpromptsubmit" => ("update", Some("running"), None, Some("Agent turn started")),
        "notify" => (
            "update",
            Some("waiting"),
            None,
            Some("Agent turn completed (notify fallback)"),
        ),
        _ => (
            "lifecycle",
            None,
            None,
            Some("Agent lifecycle event observed"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::CodexVersionProfile;

    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("integrations/fixtures/codex")
            .join(name);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        serde_json::from_str(&raw).expect("json")
    }

    #[test]
    fn session_start_fixture_normalizes() {
        let value = fixture("session-start-input.json");
        let normalized = normalize_event("SessionStart", &value).expect("normalize");
        assert_eq!(normalized.payload.event, "sessionStart");
        assert_eq!(
            normalized.payload.external_session_id.as_deref(),
            Some("codex-thread-abc")
        );
        assert_eq!(
            normalized.payload.workspace_label.as_deref(),
            Some("llm_notch")
        );
        assert!(matches!(
            normalized.profile,
            CodexVersionProfile::LifecycleHooks { .. }
        ));
    }

    #[test]
    fn stop_fixture_normalizes_without_assistant_text() {
        let value = fixture("stop-input.json");
        let normalized = normalize_event("Stop", &value).expect("normalize");
        assert_eq!(normalized.payload.event, "update");
        assert_eq!(normalized.payload.status.as_deref(), Some("waiting"));
        let encoded = serde_json::to_string(&normalized.payload).expect("serialize");
        assert!(!encoded.contains("Finished updating"));
    }

    #[test]
    fn permission_request_maps_to_attention() {
        let value = fixture("permission-request-input.json");
        let normalized = normalize_event("PermissionRequest", &value).expect("normalize");
        assert_eq!(normalized.payload.event, "attention");
        assert_eq!(normalized.payload.attention.as_deref(), Some("permission"));
        let encoded = serde_json::to_string(&normalized.payload).expect("serialize");
        assert!(!encoded.contains("npm test"));
    }

    #[test]
    fn pre_tool_use_redacts_tool_input() {
        let value = fixture("pre-tool-use-input.json");
        let normalized = normalize_event("PreToolUse", &value).expect("normalize");
        assert_eq!(normalized.payload.event, "tool");
        assert_eq!(normalized.payload.tool_name.as_deref(), Some("Bash"));
        let encoded = serde_json::to_string(&normalized.payload).expect("serialize");
        assert!(!encoded.contains("cargo check"));
    }

    #[test]
    fn post_tool_use_fixture_strips_output() {
        let value = fixture("post-tool-use-input.json");
        let normalized = normalize_event("PostToolUse", &value).expect("normalize");
        let encoded = serde_json::to_string(&normalized.payload).expect("serialize");
        assert!(!encoded.contains("Finished dev"));
    }

    #[test]
    fn unknown_event_is_lifecycle_observation() {
        let value = serde_json::json!({"thread_id": "codex-thread-abc"});
        let normalized = normalize_event("ExperimentalHook", &value).expect("normalize");
        assert_eq!(normalized.payload.event, "lifecycle");
        assert!(matches!(
            normalized.profile,
            CodexVersionProfile::Unknown { .. }
        ));
    }
}
