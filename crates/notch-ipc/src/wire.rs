//! Length-prefixed JSON wire messages for hook IPC.

use serde::{Deserialize, Serialize};

use crate::{
    error::{IpcError, IpcResult},
    limits::{
        IPC_WIRE_VERSION, MAX_ERROR_CODE_LEN, MAX_ERROR_MESSAGE_LEN, MAX_FRAME_BYTES,
        MAX_REQUEST_ID_LEN,
    },
};

/// Top-level wire envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum WireMessage {
    Auth {
        v: u16,
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "tokenB64")]
        token_b64: String,
    },
    Ingest {
        v: u16,
        #[serde(rename = "requestId")]
        request_id: String,
        payload: IngestPayload,
    },
    Ack {
        v: u16,
        #[serde(rename = "requestId")]
        request_id: String,
    },
    Error {
        v: u16,
        #[serde(rename = "requestId")]
        request_id: String,
        code: String,
        message: String,
    },
}

/// Bounded ingest payload accepted from hook clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IngestPayload {
    pub source: String,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurred_at_ms: Option<i64>,
}

impl WireMessage {
    pub fn version(&self) -> u16 {
        match self {
            Self::Auth { v, .. }
            | Self::Ingest { v, .. }
            | Self::Ack { v, .. }
            | Self::Error { v, .. } => *v,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Auth { request_id, .. }
            | Self::Ingest { request_id, .. }
            | Self::Ack { request_id, .. }
            | Self::Error { request_id, .. } => request_id,
        }
    }
}

pub fn validate_request_id(request_id: &str) -> IpcResult<()> {
    if request_id.is_empty() || request_id.len() > MAX_REQUEST_ID_LEN {
        return Err(IpcError::FrameRejected(format!(
            "requestId length must be 1..={MAX_REQUEST_ID_LEN}"
        )));
    }
    if !request_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(IpcError::FrameRejected(
            "requestId must be ASCII alphanumeric with - or _".into(),
        ));
    }
    Ok(())
}

pub fn validate_wire_message(message: &WireMessage) -> IpcResult<()> {
    validate_request_id(message.request_id())?;
    if message.version() != IPC_WIRE_VERSION {
        return Err(IpcError::FrameRejected(format!(
            "unsupported wire version {}",
            message.version()
        )));
    }
    match message {
        WireMessage::Auth { token_b64, .. } => {
            if token_b64.is_empty() || token_b64.len() > 128 {
                return Err(IpcError::FrameRejected("tokenB64 out of bounds".into()));
            }
        }
        WireMessage::Ingest { payload, .. } => validate_ingest_payload(payload)?,
        WireMessage::Ack { .. } => {}
        WireMessage::Error { code, message, .. } => {
            if code.is_empty() || code.len() > MAX_ERROR_CODE_LEN {
                return Err(IpcError::FrameRejected("error code out of bounds".into()));
            }
            if message.len() > MAX_ERROR_MESSAGE_LEN {
                return Err(IpcError::FrameRejected(
                    "error message out of bounds".into(),
                ));
            }
        }
    }
    Ok(())
}

pub fn validate_ingest_payload(payload: &IngestPayload) -> IpcResult<()> {
    validate_bounded(&payload.source, 32, "source")?;
    validate_bounded(&payload.event, 32, "event")?;
    if let Some(v) = &payload.session_id {
        validate_bounded(v, notch_protocol::MAX_SESSION_ID_LEN, "sessionId")?;
    }
    if let Some(v) = &payload.external_session_id {
        validate_bounded(
            v,
            notch_protocol::MAX_EXTERNAL_SESSION_ID_LEN,
            "externalSessionId",
        )?;
    }
    if let Some(v) = &payload.label {
        validate_bounded(v, notch_protocol::MAX_SESSION_LABEL_LEN, "label")?;
    }
    if let Some(v) = &payload.workspace_label {
        validate_bounded(v, notch_protocol::MAX_WORKSPACE_LABEL_LEN, "workspaceLabel")?;
    }
    if let Some(v) = &payload.status {
        validate_bounded(v, 32, "status")?;
    }
    if let Some(v) = &payload.attention {
        validate_bounded(v, 32, "attention")?;
    }
    if let Some(v) = &payload.summary {
        validate_bounded(v, notch_protocol::MAX_EVENT_SUMMARY_LEN, "summary")?;
    }
    if let Some(v) = &payload.tool_name {
        validate_bounded(v, notch_protocol::MAX_TOOL_NAME_LEN, "toolName")?;
    }
    if payload.pid.is_some() != payload.process_started_at_ms.is_some() {
        return Err(IpcError::FrameRejected(
            "pid and processStartedAtMs must be provided together".into(),
        ));
    }
    if payload
        .process_started_at_ms
        .is_some_and(|value| value <= 0)
    {
        return Err(IpcError::FrameRejected(
            "processStartedAtMs must be positive".into(),
        ));
    }
    Ok(())
}

fn validate_bounded(value: &str, max: usize, field: &str) -> IpcResult<()> {
    if value.is_empty() || value.len() > max {
        return Err(IpcError::FrameRejected(format!(
            "{field} length must be 1..={max}"
        )));
    }
    Ok(())
}

pub fn encode_message(message: &WireMessage) -> IpcResult<Vec<u8>> {
    validate_wire_message(message)?;
    let body = serde_json::to_vec(message).map_err(|err| IpcError::Other(err.into()))?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(IpcError::FrameRejected(format!(
            "frame exceeds {MAX_FRAME_BYTES} bytes"
        )));
    }
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

pub fn decode_frame_bytes(frame: &[u8]) -> IpcResult<WireMessage> {
    if frame.len() < 4 {
        return Err(IpcError::FrameRejected(
            "frame shorter than length prefix".into(),
        ));
    }
    let length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(IpcError::FrameRejected(format!(
            "declared length {length} exceeds max {MAX_FRAME_BYTES}"
        )));
    }
    if frame.len() != 4 + length {
        return Err(IpcError::FrameRejected(format!(
            "frame size {} != declared {}",
            frame.len(),
            4 + length
        )));
    }
    let body = std::str::from_utf8(&frame[4..])
        .map_err(|_| IpcError::FrameRejected("frame body is not valid UTF-8".into()))?;
    let message: WireMessage = serde_json::from_str(body)
        .map_err(|err| IpcError::FrameRejected(format!("invalid JSON: {err}")))?;
    validate_wire_message(&message)?;
    Ok(message)
}

/// Reject vendor JSON that carries raw prompt/command/tool output fields.
pub fn reject_sensitive_vendor_keys(value: &serde_json::Value) -> IpcResult<()> {
    const FORBIDDEN: &[&str] = &[
        "prompt",
        "output",
        "command",
        "body",
        "content",
        "message",
        "args",
        "input",
        "toolOutput",
        "toolCall",
        "raw",
        "data",
        "stderr",
        "stdout",
    ];
    let Some(obj) = value.as_object() else {
        return Err(IpcError::FrameRejected(
            "vendor payload must be a JSON object".into(),
        ));
    };
    for key in FORBIDDEN {
        if obj.contains_key(*key) {
            return Err(IpcError::FrameRejected(format!(
                "forbidden vendor field `{key}`"
            )));
        }
    }
    Ok(())
}

pub fn vendor_json_to_payload(value: &serde_json::Value) -> IpcResult<IngestPayload> {
    reject_sensitive_vendor_keys(value)?;
    let payload: IngestPayload = serde_json::from_value(value.clone())
        .map_err(|err| IpcError::FrameRejected(format!("vendor payload invalid: {err}")))?;
    validate_ingest_payload(&payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_auth_frame() {
        let msg = WireMessage::Auth {
            v: IPC_WIRE_VERSION,
            request_id: "req-1".into(),
            token_b64: "abcd".into(),
        };
        let encoded = encode_message(&msg).expect("encode");
        let decoded = decode_frame_bytes(&encoded).expect("decode");
        assert_eq!(decoded, msg);
    }

    #[test]
    fn rejects_oversized_declared_length() {
        let mut frame = vec![0, 0, 1, 0];
        frame.extend_from_slice(b"{}");
        assert!(decode_frame_bytes(&frame).is_err());
    }

    #[test]
    fn rejects_forbidden_vendor_keys() {
        let value = json!({"source":"generic","event":"tool","prompt":"secret"});
        assert!(vendor_json_to_payload(&value).is_err());
    }
}
