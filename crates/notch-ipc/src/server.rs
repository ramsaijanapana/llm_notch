//! Authenticated ingest server for hook clients.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use interprocess::local_socket::Name;
#[cfg(unix)]
use interprocess::local_socket::ToFsName;
#[cfg(windows)]
use interprocess::local_socket::ToNsName;
use interprocess::local_socket::tokio::prelude::*;
use tokio::sync::{Semaphore, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};
use tracing::{debug, warn};

use crate::auth::AuthToken;
use crate::descriptor::{
    RuntimeDescriptor, descriptor_path_for, find_descriptor_in, socket_path_for,
};
use crate::error::{IpcError, IpcResult};
use crate::framing::{read_frame_async, write_frame_async};
use crate::limits::{HOST_ACCEPT_WAIT_MS, IPC_WIRE_VERSION, MAX_CLIENTS, MAX_INGEST_QUEUE};
use crate::normalize::{NormalizedIngest, normalize_ingest};
use crate::platform;
use crate::rate::IngestRateLimiters;
use crate::security::{SecurityCapabilities, verify_same_user_peer};
use crate::spool::EventSpool;
use crate::wire::{WireMessage, validate_wire_message};

/// Configuration for [`start_ingest_server`].
#[derive(Debug, Clone, Default)]
pub struct IngestServerConfig {
    /// Override runtime directory (used in tests).
    pub runtime_dir: Option<PathBuf>,
}

/// Handle returned to the host application.
pub struct IngestServerHandle {
    shutdown: oneshot::Sender<()>,
    join: JoinHandle<()>,
    descriptor: RuntimeDescriptor,
    receiver: mpsc::Receiver<PendingIngest>,
    capabilities: SecurityCapabilities,
    runtime_dir: PathBuf,
}

/// A normalized event awaiting durable host acceptance.
pub struct PendingIngest {
    normalized: NormalizedIngest,
    completion: oneshot::Sender<Result<(), String>>,
}

impl PendingIngest {
    pub fn normalized(&self) -> &NormalizedIngest {
        &self.normalized
    }

    pub fn into_parts(self) -> (NormalizedIngest, oneshot::Sender<Result<(), String>>) {
        (self.normalized, self.completion)
    }
}

impl IngestServerHandle {
    pub fn descriptor(&self) -> &RuntimeDescriptor {
        &self.descriptor
    }

    pub fn capabilities(&self) -> &SecurityCapabilities {
        &self.capabilities
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }

    pub fn try_recv(&mut self) -> Option<PendingIngest> {
        self.receiver.try_recv().ok()
    }

    pub async fn recv(&mut self) -> Option<PendingIngest> {
        self.receiver.recv().await
    }

    pub async fn shutdown(self) -> IpcResult<()> {
        let Self {
            shutdown,
            join,
            descriptor,
            runtime_dir,
            ..
        } = self;
        let _ = shutdown.send(());
        join.await.map_err(|err| IpcError::Other(err.into()))?;
        cleanup_runtime_files(&runtime_dir, &descriptor);
        Ok(())
    }
}

/// Start the authenticated local ingest server and return a consumer handle.
pub async fn start_ingest_server(config: IngestServerConfig) -> IpcResult<IngestServerHandle> {
    let runtime_dir = config
        .runtime_dir
        .clone()
        .unwrap_or_else(|| crate::descriptor::default_runtime_dir().expect("runtime dir"));
    platform::ensure_runtime_dir(&runtime_dir)?;

    let token = AuthToken::generate();
    let socket_path = socket_path_for(&runtime_dir);
    let connect_path = crate::descriptor::connect_path_for(&runtime_dir);
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).map_err(IpcError::Io)?;
    }
    let listener_name = listener_name(&socket_path)?;
    let listener = platform::build_listener_options(listener_name)?
        .create_tokio()
        .map_err(IpcError::Io)?;
    platform::post_bind_harden(&socket_path)?;

    let started_at_ms = now_ms();
    let descriptor = RuntimeDescriptor::new(connect_path, &token, started_at_ms);
    descriptor.write_to(&descriptor_path_for(&runtime_dir))?;

    let (tx, rx) = mpsc::channel(MAX_INGEST_QUEUE);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let capabilities = SecurityCapabilities::platform_default();
    let limits = Arc::new(IngestRateLimiters::new());
    let client_semaphore = Arc::new(Semaphore::new(MAX_CLIENTS));
    let expected_token = token.clone();
    let caps = capabilities.clone();
    let runtime_for_drain = runtime_dir.clone();

    let join = tokio::spawn(async move {
        if let Ok(spool) = EventSpool::new(&runtime_for_drain) {
            drain_spool(spool, tx.clone()).await;
        }

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    let Ok(stream) = accepted else {
                        warn!("accept failed");
                        continue;
                    };
                    let permit = match client_semaphore.clone().acquire_owned().await {
                        Ok(permit) => permit,
                        Err(_) => continue,
                    };
                    let tx = tx.clone();
                    let limits = limits.clone();
                    let expected = expected_token.clone();
                    let caps = caps.clone();
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(err) = handle_connection(stream, expected, limits, caps, tx).await {
                            debug!(?err, "connection closed");
                        }
                    });
                }
            }
        }
    });

    Ok(IngestServerHandle {
        shutdown: shutdown_tx,
        join,
        descriptor,
        receiver: rx,
        capabilities,
        runtime_dir,
    })
}

async fn handle_connection(
    mut stream: LocalSocketStream,
    expected_token: AuthToken,
    limits: Arc<IngestRateLimiters>,
    capabilities: SecurityCapabilities,
    tx: mpsc::Sender<PendingIngest>,
) -> IpcResult<()> {
    let creds = stream.peer_creds().map_err(IpcError::Io)?;
    verify_same_user_peer(&creds, &capabilities)?;

    let auth_frame = read_frame_async(&mut stream).await?;
    let WireMessage::Auth {
        token_b64,
        request_id,
        ..
    } = auth_frame
    else {
        write_error(
            &mut stream,
            "bootstrap",
            "auth_required",
            "first frame must be auth",
        )
        .await?;
        return Err(IpcError::AuthFailed);
    };
    let provided = AuthToken::decode_b64(&token_b64)?;
    if !expected_token.constant_time_eq(&provided) {
        write_error(&mut stream, &request_id, "auth_failed", "invalid token").await?;
        return Err(IpcError::AuthFailed);
    }
    write_frame_async(
        &mut stream,
        &WireMessage::Ack {
            v: IPC_WIRE_VERSION,
            request_id,
        },
    )
    .await?;

    let client_id = creds
        .pid()
        .map(|pid| pid as u64)
        .unwrap_or_else(|| limits.assign_client_id());
    loop {
        let frame = match read_frame_async(&mut stream).await {
            Ok(frame) => frame,
            Err(IpcError::ReadTimeout) => continue,
            Err(IpcError::ConnectionClosed) => break,
            Err(err) => return Err(err),
        };
        validate_wire_message(&frame)?;
        match frame {
            WireMessage::Ingest {
                request_id,
                payload,
                ..
            } => match process_ingest(&limits, client_id, &payload, &tx).await {
                Ok(response) => match timeout(Duration::from_millis(HOST_ACCEPT_WAIT_MS), response)
                    .await
                {
                    Ok(Ok(Ok(()))) => {
                        write_frame_async(
                            &mut stream,
                            &WireMessage::Ack {
                                v: IPC_WIRE_VERSION,
                                request_id,
                            },
                        )
                        .await?;
                    }
                    Ok(Ok(Err(message))) => {
                        write_error(&mut stream, &request_id, "ingest_rejected", &message).await?;
                    }
                    Ok(Err(_)) => {
                        write_error(
                            &mut stream,
                            &request_id,
                            "host_unavailable",
                            "host ingest consumer closed",
                        )
                        .await?;
                    }
                    Err(_) => {
                        write_error(
                            &mut stream,
                            &request_id,
                            "host_timeout",
                            "host did not durably accept ingest before timeout",
                        )
                        .await?;
                    }
                },
                Err(err) => {
                    write_error(&mut stream, &request_id, error_code(&err), &err.to_string())
                        .await?;
                }
            },
            WireMessage::Ack { .. } => {}
            other => {
                write_error(
                    &mut stream,
                    other.request_id(),
                    "invalid_frame",
                    "unexpected frame type",
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub(crate) async fn process_ingest(
    limits: &IngestRateLimiters,
    client_id: u64,
    payload: &crate::wire::IngestPayload,
    tx: &mpsc::Sender<PendingIngest>,
) -> IpcResult<oneshot::Receiver<Result<(), String>>> {
    limits.check(client_id)?;
    let normalized = normalize_ingest(payload, now_ms())?;
    let (completion, response) = oneshot::channel();
    tx.try_send(PendingIngest {
        normalized,
        completion,
    })
    .map_err(|_| IpcError::QueueFull)?;
    Ok(response)
}

async fn write_error(
    stream: &mut LocalSocketStream,
    request_id: &str,
    code: &str,
    message: &str,
) -> IpcResult<()> {
    write_frame_async(
        stream,
        &WireMessage::Error {
            v: IPC_WIRE_VERSION,
            request_id: request_id.into(),
            code: code.into(),
            message: message.into(),
        },
    )
    .await
}

fn error_code(err: &IpcError) -> &'static str {
    match err {
        IpcError::RateLimited => "rate_limited",
        IpcError::QueueFull => "queue_full",
        IpcError::FrameRejected(_) => "invalid_frame",
        IpcError::AuthFailed => "auth_failed",
        _ => "internal_error",
    }
}

async fn drain_spool(spool: EventSpool, tx: mpsc::Sender<PendingIngest>) {
    for path in spool.list_frames().unwrap_or_default() {
        let Ok(bytes) = std::fs::read(&path) else {
            break;
        };
        let Ok(WireMessage::Ingest { payload, .. }) = crate::wire::decode_frame_bytes(&bytes)
        else {
            break;
        };
        let Ok(normalized) = normalize_ingest(&payload, now_ms()) else {
            break;
        };
        let (completion, response) = oneshot::channel();
        if tx
            .try_send(PendingIngest {
                normalized,
                completion,
            })
            .is_err()
        {
            break;
        }
        let accepted = matches!(
            timeout(Duration::from_millis(HOST_ACCEPT_WAIT_MS), response).await,
            Ok(Ok(Ok(())))
        );
        if !accepted {
            break;
        }
        if spool.remove(&path).is_err() {
            break;
        }
    }
}

fn listener_name(socket_path: &Path) -> IpcResult<Name<'_>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::GenericFilePath;
        socket_path
            .to_fs_name::<GenericFilePath>()
            .map_err(IpcError::Io)
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::GenericNamespaced;
        let _ = socket_path;
        "llm_notch_ingest"
            .to_ns_name::<GenericNamespaced>()
            .map_err(IpcError::Io)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = socket_path;
        Err(IpcError::InvalidConfig(
            "unsupported platform for ingest listener".into(),
        ))
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn cleanup_runtime_files(runtime_dir: &Path, descriptor: &RuntimeDescriptor) {
    let descriptor_path = descriptor_path_for(runtime_dir);
    let owns_descriptor = RuntimeDescriptor::read_from(&descriptor_path)
        .map(|current| {
            current.host_pid == descriptor.host_pid
                && current.started_at_ms == descriptor.started_at_ms
        })
        .unwrap_or(false);
    if owns_descriptor {
        let _ = std::fs::remove_file(descriptor_path);
    }

    let socket_path = socket_path_for(runtime_dir);
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }
}

/// Locate an existing runtime descriptor without starting a server.
pub fn open_runtime_descriptor(runtime_dir: &Path) -> IpcResult<RuntimeDescriptor> {
    find_descriptor_in(runtime_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::IngestClient;
    use crate::limits::IPC_WIRE_VERSION;
    use crate::spool::EventSpool;
    use crate::wire::IngestPayload;
    use crate::wire::WireMessage;
    use tempfile::tempdir;
    use tokio::time::Duration;

    #[tokio::test]
    async fn server_accepts_authenticated_ingest() {
        let dir = tempdir().expect("tempdir");
        let mut handle = start_ingest_server(IngestServerConfig {
            runtime_dir: Some(dir.path().to_path_buf()),
        })
        .await
        .expect("start");

        let client = IngestClient::from_descriptor(handle.descriptor()).expect("client");
        let send = tokio::spawn(async move {
            client
                .send_ingest(
                    "req-1",
                    &IngestPayload {
                        source: "generic".into(),
                        event: "sessionStart".into(),
                        session_id: None,
                        external_session_id: Some("ext".into()),
                        label: Some("Test".into()),
                        workspace_label: None,
                        status: Some("running".into()),
                        attention: Some("none".into()),
                        summary: None,
                        tool_name: None,
                        pid: None,
                        process_started_at_ms: None,
                        occurred_at_ms: Some(1),
                    },
                )
                .await
        });
        let pending = tokio::time::timeout(Duration::from_secs(1), handle.recv())
            .await
            .expect("receive timeout")
            .expect("pending ingest");
        tokio::task::yield_now().await;
        assert!(
            !send.is_finished(),
            "client must not receive ACK before host acceptance"
        );
        let (_, completion) = pending.into_parts();
        completion.send(Ok(())).expect("accept");
        send.await.expect("send task").expect("send");
        handle.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn server_returns_rejection_instead_of_ack() {
        let dir = tempdir().expect("tempdir");
        let mut handle = start_ingest_server(IngestServerConfig {
            runtime_dir: Some(dir.path().to_path_buf()),
        })
        .await
        .expect("start");
        let client = IngestClient::from_descriptor(handle.descriptor()).expect("client");
        let send = tokio::spawn(async move {
            client
                .send_ingest(
                    "reject-1",
                    &IngestPayload {
                        source: "generic".into(),
                        event: "sessionStart".into(),
                        session_id: None,
                        external_session_id: Some("reject".into()),
                        label: Some("Reject".into()),
                        workspace_label: None,
                        status: Some("running".into()),
                        attention: None,
                        summary: None,
                        tool_name: None,
                        pid: None,
                        process_started_at_ms: None,
                        occurred_at_ms: Some(1),
                    },
                )
                .await
        });
        let pending = handle.recv().await.expect("pending");
        let (_, completion) = pending.into_parts();
        completion
            .send(Err("core rejected transition".into()))
            .expect("reject");
        assert!(matches!(
            send.await.expect("send task"),
            Err(IpcError::FrameRejected(message)) if message.contains("core rejected")
        ));
        handle.shutdown().await.expect("shutdown");
    }

    fn spooled_start() -> WireMessage {
        WireMessage::Ingest {
            v: IPC_WIRE_VERSION,
            request_id: "spooled-start".into(),
            payload: IngestPayload {
                source: "generic".into(),
                event: "sessionStart".into(),
                session_id: None,
                external_session_id: Some("spooled".into()),
                label: Some("Spooled".into()),
                workspace_label: None,
                status: Some("running".into()),
                attention: None,
                summary: None,
                tool_name: None,
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(1),
            },
        }
    }

    #[tokio::test]
    async fn spool_replay_deletes_only_after_acceptance() {
        let dir = tempdir().expect("tempdir");
        let spool = EventSpool::new(dir.path()).expect("spool");
        spool.spool_message(&spooled_start()).expect("write spool");
        let mut handle = start_ingest_server(IngestServerConfig {
            runtime_dir: Some(dir.path().to_path_buf()),
        })
        .await
        .expect("start");

        let pending = handle.recv().await.expect("pending replay");
        assert_eq!(spool.list_frames().unwrap().len(), 1);
        let (_, completion) = pending.into_parts();
        completion.send(Ok(())).expect("accept replay");
        for _ in 0..20 {
            if spool.list_frames().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(spool.list_frames().unwrap().is_empty());
        handle.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn rejected_spool_replay_is_retained() {
        let dir = tempdir().expect("tempdir");
        let spool = EventSpool::new(dir.path()).expect("spool");
        spool.spool_message(&spooled_start()).expect("write spool");
        let mut handle = start_ingest_server(IngestServerConfig {
            runtime_dir: Some(dir.path().to_path_buf()),
        })
        .await
        .expect("start");

        let pending = handle.recv().await.expect("pending replay");
        let (_, completion) = pending.into_parts();
        completion
            .send(Err("reject replay".into()))
            .expect("reject");
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(spool.list_frames().unwrap().len(), 1);
        handle.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn bounded_ingest_queue_rejects_when_full() {
        let limits = IngestRateLimiters::new();
        let (tx, _rx) = mpsc::channel(1);
        let payload = match spooled_start() {
            WireMessage::Ingest { payload, .. } => payload,
            _ => unreachable!(),
        };
        process_ingest(&limits, 1, &payload, &tx)
            .await
            .expect("first enqueue");
        assert!(matches!(
            process_ingest(&limits, 1, &payload, &tx).await,
            Err(IpcError::QueueFull)
        ));
    }
}
