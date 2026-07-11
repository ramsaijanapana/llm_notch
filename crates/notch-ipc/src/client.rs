//! Hook-side authenticated ingest client.

use std::path::Path;
use std::time::Duration;

use interprocess::local_socket::Name;
#[cfg(unix)]
use interprocess::local_socket::ToFsName;
#[cfg(windows)]
use interprocess::local_socket::ToNsName;
use interprocess::local_socket::tokio::prelude::*;
use tokio::time;

use crate::descriptor::{RuntimeDescriptor, find_descriptor, find_descriptor_in};
use crate::error::{IpcError, IpcResult};
use crate::framing::{read_frame_async, write_frame_async};
use crate::limits::{ACK_WAIT_MS, IPC_WIRE_VERSION};
use crate::wire::{DecisionWaitPayload, IngestPayload, WireMessage};
use notch_protocol::{DECISION_FAIL_OPEN_TIMEOUT_MS, DECISION_HOOK_NEUTRAL_OUTPUT};

/// Client used by `notch-hook` to deliver normalized events to the host.
#[derive(Clone)]
pub struct IngestClient {
    descriptor: RuntimeDescriptor,
}

impl IngestClient {
    pub fn from_descriptor(descriptor: &RuntimeDescriptor) -> IpcResult<Self> {
        Ok(Self {
            descriptor: descriptor.clone(),
        })
    }

    pub fn discover() -> IpcResult<Self> {
        Self::from_descriptor(&find_descriptor()?)
    }

    pub fn discover_in(runtime_dir: &Path) -> IpcResult<Self> {
        Self::from_descriptor(&find_descriptor_in(runtime_dir)?)
    }

    pub async fn send_ingest(&self, request_id: &str, payload: &IngestPayload) -> IpcResult<()> {
        let mut stream = self.connect().await?;
        self.authenticate(&mut stream).await?;
        write_frame_async(
            &mut stream,
            &WireMessage::Ingest {
                v: IPC_WIRE_VERSION,
                request_id: request_id.into(),
                payload: payload.clone(),
            },
        )
        .await?;
        self.wait_for_ack(&mut stream, request_id).await
    }

    /// Wait for an interactive decision reply. Never spooled; fail-open on timeout.
    pub async fn request_decision(&self, payload: &DecisionWaitPayload) -> IpcResult<String> {
        let mut stream = self.connect().await?;
        self.authenticate(&mut stream).await?;
        write_frame_async(
            &mut stream,
            &WireMessage::DecisionWait {
                v: IPC_WIRE_VERSION,
                request_id: payload.nonce.clone(),
                source: payload.source.clone(),
                vendor_event: payload.vendor_event.clone(),
                external_session_id: payload.external_session_id.clone(),
                session_id: payload.session_id.clone(),
                decision_kind: payload.decision_kind.clone(),
                summary: payload.summary.clone(),
                has_actionable_payload: payload.has_actionable_payload,
                respondable_hook: payload.respondable_hook.clone(),
                tool_name: payload.tool_name.clone(),
                connection_id: payload.connection_id.clone(),
                vendor_context_json: payload.vendor_context_json.clone(),
                created_at_ms: payload.created_at_ms,
            },
        )
        .await?;
        let response = time::timeout(
            Duration::from_millis(DECISION_FAIL_OPEN_TIMEOUT_MS),
            read_frame_async(&mut stream),
        )
        .await
        .map_err(|_| IpcError::ReadTimeout)??;
        match response {
            WireMessage::DecisionReply {
                request_id,
                stdout_json,
                ..
            } if request_id == payload.nonce => Ok(stdout_json),
            WireMessage::Error { code, message, .. } => match code.as_str() {
                "decision_expired" | "host_timeout" => Ok(DECISION_HOOK_NEUTRAL_OUTPUT.into()),
                "auth_failed" | "auth_required" => Err(IpcError::AuthFailed),
                _ => Err(IpcError::FrameRejected(message)),
            },
            _ => Ok(DECISION_HOOK_NEUTRAL_OUTPUT.into()),
        }
    }

    async fn connect(&self) -> IpcResult<LocalSocketStream> {
        let name = connect_name(&self.descriptor)?;
        LocalSocketStream::connect(name).await.map_err(IpcError::Io)
    }

    async fn authenticate(&self, stream: &mut LocalSocketStream) -> IpcResult<()> {
        let token = self.descriptor.token()?;
        write_frame_async(
            stream,
            &WireMessage::Auth {
                v: IPC_WIRE_VERSION,
                request_id: "auth".into(),
                token_b64: token.encode_b64(),
            },
        )
        .await?;
        self.wait_for_ack(stream, "auth").await
    }

    async fn wait_for_ack(
        &self,
        stream: &mut LocalSocketStream,
        request_id: &str,
    ) -> IpcResult<()> {
        let response = time::timeout(Duration::from_millis(ACK_WAIT_MS), read_frame_async(stream))
            .await
            .map_err(|_| IpcError::ReadTimeout)??;

        match response {
            WireMessage::Ack { request_id: id, .. } if id == request_id => Ok(()),
            WireMessage::Error { code, message, .. } => match code.as_str() {
                "auth_failed" | "auth_required" => Err(IpcError::AuthFailed),
                "queue_full" => Err(IpcError::QueueFull),
                "rate_limited" => Err(IpcError::RateLimited),
                "host_timeout" | "host_unavailable" => Err(IpcError::ReadTimeout),
                _ => Err(IpcError::FrameRejected(message)),
            },
            _ => Err(IpcError::FrameRejected("unexpected response".into())),
        }
    }
}

fn connect_name(descriptor: &RuntimeDescriptor) -> IpcResult<Name<'_>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::GenericFilePath;
        std::path::Path::new(&descriptor.socket_path)
            .to_fs_name::<GenericFilePath>()
            .map_err(IpcError::Io)
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::GenericNamespaced;
        let _ = descriptor;
        "llm_notch_ingest"
            .to_ns_name::<GenericNamespaced>()
            .map_err(IpcError::Io)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = descriptor;
        Err(IpcError::InvalidConfig(
            "unsupported platform for ingest client".into(),
        ))
    }
}
