use notch_protocol::{AttentionKind, MAX_TOOL_NAME_LEN, SessionEventKind};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hook_ingest::{RelayHookPayload, validate_hook_payload};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_REMOTE_FRAME_BYTES: usize = 256 * 1024;
const MAX_HOST_ID_LEN: usize = 64;
const MAX_SESSION_ID_LEN: usize = 256;
const MAX_EVENT_SUMMARY_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayHello {
    pub protocol_version: u16,
    pub host_id: String,
    pub connection_nonce: String,
    pub resume: ResumeCursor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumeCursor {
    pub last_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayFrame {
    pub sequence: u64,
    pub payload: RelayPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum RelayControl {
    Acknowledge {
        cursor: ResumeCursor,
    },
    /// Injects a normalized hook event for immediate forwarding on stdout.
    InjectHook {
        payload: RelayHookPayload,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum RelayPayload {
    SessionEvent {
        session_id: String,
        source: String,
        summary: String,
        occurred_at_ms: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<SessionEventKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attention: Option<AttentionKind>,
    },
    Checkpoint,
    Heartbeat,
    Error {
        code: String,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("remote protocol version is incompatible")]
    IncompatibleVersion,
    #[error("remote frame exceeds the size limit")]
    FrameTooLarge,
    #[error("remote frame contains an invalid field")]
    InvalidField,
    #[error("remote sequence did not advance")]
    SequenceRegression,
}

impl RelayHello {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::IncompatibleVersion);
        }
        if self.host_id.is_empty()
            || self.host_id.len() > MAX_HOST_ID_LEN
            || self.connection_nonce.len() != 64
            || !self
                .connection_nonce
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ProtocolError::InvalidField);
        }
        Ok(())
    }
}

impl RelayFrame {
    pub fn validate_after(&self, cursor: &ResumeCursor) -> Result<(), ProtocolError> {
        if self.sequence <= cursor.last_sequence {
            return Err(ProtocolError::SequenceRegression);
        }
        validate_payload(&self.payload)?;
        let encoded = serde_json::to_vec(self).map_err(|_| ProtocolError::InvalidField)?;
        if encoded.len() > MAX_REMOTE_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(())
    }
}

impl RelayControl {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Acknowledge { .. } | Self::Shutdown => Ok(()),
            Self::InjectHook { payload } => {
                validate_hook_payload(payload).map_err(|_| ProtocolError::InvalidField)
            }
        }
    }
}

fn validate_payload(payload: &RelayPayload) -> Result<(), ProtocolError> {
    match payload {
        RelayPayload::SessionEvent {
            session_id,
            source,
            summary,
            tool_name,
            ..
        } if session_id.is_empty()
            || session_id.len() > MAX_SESSION_ID_LEN
            || source.is_empty()
            || source.len() > 64
            || summary.len() > MAX_EVENT_SUMMARY_LEN
            || tool_name
                .as_ref()
                .is_some_and(|value| value.is_empty() || value.len() > MAX_TOOL_NAME_LEN) =>
        {
            Err(ProtocolError::InvalidField)
        }
        RelayPayload::Error { code } if code.is_empty() || code.len() > 64 => {
            Err(ProtocolError::InvalidField)
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::SessionEventKind;

    #[test]
    fn rejects_oversized_summary_before_transport() {
        let frame = RelayFrame {
            sequence: 1,
            payload: RelayPayload::SessionEvent {
                session_id: "session".into(),
                source: "codex".into(),
                summary: "x".repeat(MAX_EVENT_SUMMARY_LEN + 1),
                occurred_at_ms: 0,
                kind: None,
                tool_name: None,
                attention: None,
            },
        };
        assert_eq!(
            frame.validate_after(&ResumeCursor::default()),
            Err(ProtocolError::InvalidField)
        );
    }

    #[test]
    fn inject_hook_control_validates_bounded_payload() {
        use crate::hook_ingest::RelayHookPayload;

        let valid = RelayControl::InjectHook {
            payload: RelayHookPayload {
                source: "codex".into(),
                event: "tool".into(),
                session_id: None,
                external_session_id: Some("sess-1".into()),
                summary: Some("ok".into()),
                occurred_at_ms: Some(1),
                tool_name: Some("run_command".into()),
                attention: None,
            },
        };
        assert_eq!(valid.validate(), Ok(()));

        let invalid = RelayControl::InjectHook {
            payload: RelayHookPayload {
                source: String::new(),
                event: "tool".into(),
                session_id: None,
                external_session_id: Some("sess-1".into()),
                summary: None,
                occurred_at_ms: None,
                tool_name: None,
                attention: None,
            },
        };
        assert_eq!(invalid.validate(), Err(ProtocolError::InvalidField));
    }

    #[test]
    fn deserializes_legacy_session_event_without_optional_fields() {
        let json = r#"{
            "type":"sessionEvent",
            "session_id":"remote-session-1",
            "source":"codex",
            "summary":"Legacy relay frame",
            "occurred_at_ms":1700000000000
        }"#;
        let payload: RelayPayload = serde_json::from_str(json).expect("deserialize");
        assert_eq!(
            payload,
            RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Legacy relay frame".into(),
                occurred_at_ms: 1_700_000_000_000,
                kind: None,
                tool_name: None,
                attention: None,
            }
        );
    }

    #[test]
    fn rejects_oversized_tool_name_on_session_event() {
        let frame = RelayFrame {
            sequence: 1,
            payload: RelayPayload::SessionEvent {
                session_id: "session".into(),
                source: "codex".into(),
                summary: "ok".into(),
                occurred_at_ms: 0,
                kind: Some(SessionEventKind::Tool),
                tool_name: Some("x".repeat(MAX_TOOL_NAME_LEN + 1)),
                attention: None,
            },
        };
        assert_eq!(
            frame.validate_after(&ResumeCursor::default()),
            Err(ProtocolError::InvalidField)
        );
    }
}
