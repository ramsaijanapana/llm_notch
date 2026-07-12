//! Versioned generic ingest protocol constants and validation.

use notch_ipc::{IngestPayload, validate_ingest_payload};
use serde_json::Value;
use thiserror::Error;

/// Wire version for third-party generic protocol examples shipped with llm_notch.
pub const GENERIC_PROTOCOL_VERSION: u16 = 1;

/// One documented generic command example for SDK consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericCommandExample {
    pub name: &'static str,
    pub description: &'static str,
    pub argv: &'static [&'static str],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GenericProtocolError {
    #[error("unsupported protocol version `{0}`")]
    UnsupportedVersion(u16),
    #[error("generic source required")]
    WrongSource,
    #[error("event `{0}` is not documented for protocol v1")]
    UnknownEvent(String),
    #[error("payload invalid: {0}")]
    InvalidPayload(String),
}

const DOCUMENTED_EVENTS: &[&str] = &[
    "sessionStart",
    "sessionEnd",
    "update",
    "tool",
    "attention",
    "lifecycle",
    "remove",
];

/// Shipped CLI examples for `llm-notch-hook emit`.
pub fn example_commands() -> Vec<GenericCommandExample> {
    vec![
        GenericCommandExample {
            name: "sessionStart",
            description: "Start or upsert a generic session",
            argv: &[
                "emit",
                "--source",
                "generic",
                "--event",
                "sessionStart",
                "--external-session-id",
                "generic-cli-7",
                "--label",
                "Generic CLI agent",
                "--workspace-label",
                "llm_notch",
                "--status",
                "running",
            ],
        },
        GenericCommandExample {
            name: "tool",
            description: "Append a redacted tool event",
            argv: &[
                "emit",
                "--source",
                "generic",
                "--event",
                "tool",
                "--external-session-id",
                "generic-cli-7",
                "--summary",
                "Build step finished",
                "--tool-name",
                "cargo",
            ],
        },
        GenericCommandExample {
            name: "attention",
            description: "Set observation-only attention",
            argv: &[
                "emit",
                "--source",
                "generic",
                "--event",
                "attention",
                "--external-session-id",
                "generic-cli-7",
                "--attention",
                "question",
                "--summary",
                "Agent waiting for input",
            ],
        },
    ]
}

/// Validate a normalized ingest payload against the generic protocol v1 surface.
pub fn validate_ingest_example(payload: &IngestPayload) -> Result<(), GenericProtocolError> {
    if payload.source != "generic" {
        return Err(GenericProtocolError::WrongSource);
    }
    if !DOCUMENTED_EVENTS
        .iter()
        .any(|event| event.eq_ignore_ascii_case(&payload.event))
    {
        return Err(GenericProtocolError::UnknownEvent(payload.event.clone()));
    }
    validate_ingest_payload(payload)
        .map_err(|err| GenericProtocolError::InvalidPayload(err.to_string()))
}

/// Validate a protocol fixture object from `integrations/fixtures/protocol`.
pub fn validate_protocol_fixture(value: &Value) -> Result<(), GenericProtocolError> {
    let version = value
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u16;
    if version != GENERIC_PROTOCOL_VERSION {
        return Err(GenericProtocolError::UnsupportedVersion(version));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_ipc::IngestPayload;

    #[test]
    fn example_commands_use_generic_source() {
        for example in example_commands() {
            assert!(
                example
                    .argv
                    .windows(2)
                    .any(|window| window == ["--source", "generic"])
            );
        }
    }

    #[test]
    fn validates_generic_session_start_payload() {
        let payload = IngestPayload {
            source: "generic".into(),
            event: "sessionStart".into(),
            session_id: None,
            external_session_id: Some("generic-cli-7".into()),
            label: Some("Generic CLI agent".into()),
            workspace_label: Some("llm_notch".into()),
            status: Some("running".into()),
            attention: None,
            summary: None,
            tool_name: None,
            pid: None,
            process_started_at_ms: None,
            occurred_at_ms: None,
            terminal_session_id: None,
            tab_id: None,
            pane_id: None,
            window_handle: None,
        };
        validate_ingest_example(&payload).expect("valid");
    }
}
