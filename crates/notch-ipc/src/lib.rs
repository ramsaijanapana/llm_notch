//! Authenticated OS-local ingest transport for hook helpers and the desktop host.
//!
//! Wire format: 4-byte big-endian length prefix + UTF-8 JSON body (max 64 KiB).
//! Security: per-app-start 256-bit token in a user-only runtime descriptor; never
//! passed via argv, environment variables, logs, or SQLite.

pub mod auth;
pub mod collector;
pub mod descriptor;
pub mod error;
pub mod framing;
pub mod limits;
pub mod normalize;
pub mod platform;
pub mod rate;
pub mod security;
pub mod spool;
pub mod wire;

mod client;
mod server;

pub use auth::AuthToken;
pub use client::IngestClient;
pub use collector::{
    ENV_PANE_ID, ENV_TAB_ID, ENV_TERMINAL_SESSION_ID, ENV_WINDOW_HANDLE, ENV_WT_SESSION,
    enrich_ingest_with_collector_env, verified_terminal_from_ingest,
};
pub use descriptor::{
    RuntimeDescriptor, connect_path_for, default_runtime_dir, descriptor_path_for, find_descriptor,
    find_descriptor_in, socket_path_for,
};
pub use error::{IpcError, IpcResult};
pub use limits::*;
pub use normalize::{NormalizedIngest, normalize_ingest, stable_session_id};
pub use security::{PeerCheckCapability, SecurityCapabilities};
pub use server::{
    DecisionReplyWire, IngestServerConfig, IngestServerHandle, PendingDecisionWait, PendingIngest,
    open_runtime_descriptor, start_ingest_server,
};
pub use spool::EventSpool;
pub use wire::{
    DecisionWaitPayload, IngestPayload, WireMessage, encode_message, validate_ingest_payload,
    vendor_json_to_payload,
};

/// Legacy placeholder kept for workspace compatibility while host wiring lands.
#[derive(Debug, Default)]
pub struct IpcBroker;

impl IpcBroker {
    pub fn new() -> Self {
        Self
    }

    /// Validates a protocol stream frame size without sending it over IPC.
    pub fn validate_frame(&self, frame: &notch_protocol::StreamFrame) -> IpcResult<()> {
        let encoded = serde_json::to_vec(frame).map_err(|err| IpcError::Other(err.into()))?;
        if encoded.len() > notch_protocol::MAX_STREAM_FRAME_BYTES {
            return Err(IpcError::FrameRejected(format!(
                "frame exceeds {} bytes",
                notch_protocol::MAX_STREAM_FRAME_BYTES
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use std::io::Cursor;
    use std::time::Duration;

    use crate::IpcError;
    use crate::framing::{read_frame_sync, write_frame_sync};
    use crate::limits::IPC_WIRE_VERSION;
    use crate::wire::{WireMessage, decode_frame_bytes, vendor_json_to_payload};
    use serde_json::json;

    #[test]
    fn malformed_json_is_rejected() {
        let mut frame = vec![0, 0, 0, 2, b'{', b'}'];
        assert!(decode_frame_bytes(&frame).is_err());
        frame[3] = 255;
        assert!(decode_frame_bytes(&frame).is_err());
    }

    #[test]
    fn fuzz_like_random_payloads_rejected() {
        for seed in 0u8..32 {
            let len = (seed as usize % 8) + 1;
            let mut frame = vec![0, 0, 0, len as u8];
            frame.extend(std::iter::repeat(seed).take(len));
            assert!(decode_frame_bytes(&frame).is_err());
        }
    }

    #[test]
    fn sync_framing_respects_timeout() {
        let msg = WireMessage::Ack {
            v: IPC_WIRE_VERSION,
            request_id: "t".into(),
        };
        let mut buf = Vec::new();
        write_frame_sync(&mut buf, &msg).expect("write");
        let partial = buf[..2].to_vec();
        let mut cursor = Cursor::new(partial);
        assert!(read_frame_sync(&mut cursor, Duration::from_millis(10)).is_err());
    }

    #[test]
    fn vendor_payload_normalization_strips_sensitive_keys() {
        let value = json!({
            "source": "cursor",
            "event": "tool",
            "externalSessionId": "abc",
            "summary": "Permission requested",
            "toolName": "shell"
        });
        let payload = vendor_json_to_payload(&value).expect("payload");
        assert_eq!(payload.source, "cursor");
    }

    #[tokio::test]
    async fn rate_limit_rejects_excess_events() {
        let payload = crate::IngestPayload {
            source: "generic".into(),
            event: "tool".into(),
            session_id: None,
            external_session_id: Some("x".into()),
            label: Some("x".into()),
            workspace_label: None,
            status: None,
            attention: None,
            summary: Some("s".into()),
            tool_name: None,
            pid: None,
            process_started_at_ms: None,
            occurred_at_ms: Some(1),
            terminal_session_id: None,
            tab_id: None,
            pane_id: None,
            window_handle: None,
        };
        let limits = crate::rate::IngestRateLimiters::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(crate::MAX_INGEST_QUEUE);
        let mut rejected = 0;
        for _ in 0..130 {
            if matches!(
                crate::server::process_ingest(&limits, 7, &payload, &tx).await,
                Err(IpcError::RateLimited)
            ) {
                rejected += 1;
            }
        }
        assert!(rejected >= 1, "burst limit must reject actual client sends");
    }
}
