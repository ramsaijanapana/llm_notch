//! External hook helper: reads vendor JSON from stdin or explicit `emit` CLI,
//! authenticates against the host runtime descriptor, and delivers bounded ingest
//! frames. Vendor hook mode always fails open (exit 0).

use std::env;
use std::io::{self, Read};
use std::process::ExitCode;

use notch_ipc::{
    EventSpool, IPC_WIRE_VERSION, IngestClient, IngestPayload, IpcError, WireMessage,
    default_runtime_dir, vendor_json_to_payload,
};
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookMode {
    NormalizedVendor,
    Vendor {
        source: String,
        vendor_event: String,
    },
    Emit {
        fail_on_error: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .without_time()
        .try_init()
        .ok();

    let args: Vec<String> = env::args().collect();
    let mode = match args.get(1).map(String::as_str) {
        None => HookMode::NormalizedVendor,
        Some("hook") => match parse_hook_mode(&args[2..]) {
            Ok(mode) => mode,
            Err(err) => {
                debug!(?err, "invalid hook arguments");
                return ExitCode::SUCCESS;
            }
        },
        Some("emit") => HookMode::Emit {
            fail_on_error: args.iter().any(|arg| arg == "--fail-on-error"),
        },
        Some("help" | "-h" | "--help") => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            warn!(command = other, "unknown command");
            print_help();
            return ExitCode::from(2);
        }
    };

    let payload = match &mode {
        HookMode::NormalizedVendor => match read_vendor_payload() {
            Ok(payload) => payload,
            Err(err) => {
                debug!(?err, "vendor payload unavailable");
                return fail_open(&mode);
            }
        },
        HookMode::Vendor {
            source,
            vendor_event,
        } => match read_vendor_hook_payload(source, vendor_event) {
            Ok(payload) => payload,
            Err(err) => {
                debug!(?err, "vendor hook payload unavailable");
                return fail_open(&mode);
            }
        },
        HookMode::Emit { .. } => match parse_emit_args(&args[2..]) {
            Ok(payload) => payload,
            Err(err) => return emit_failure(&mode, err),
        },
    };

    let request_id = Uuid::new_v4().simple().to_string();
    match deliver_payload(&request_id, &payload).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            debug!(?err, "deliver failed");
            if is_vendor_mode(&mode) && is_transient_delivery_error(&err) {
                if let Err(spool_error) = spool_payload(&request_id, &payload) {
                    debug!(?spool_error, "transient event could not be spooled");
                }
                return ExitCode::SUCCESS;
            }
            emit_failure(&mode, err)
        }
    }
}

async fn deliver_payload(request_id: &str, payload: &IngestPayload) -> Result<(), IpcError> {
    let client = IngestClient::discover()?;
    deliver_with_client(&client, request_id, payload).await
}

async fn deliver_with_client(
    client: &IngestClient,
    request_id: &str,
    payload: &IngestPayload,
) -> Result<(), IpcError> {
    client.send_ingest(request_id, payload).await
}

fn spool_payload(request_id: &str, payload: &IngestPayload) -> Result<(), IpcError> {
    let runtime_dir = default_runtime_dir()?;
    spool_payload_in(&runtime_dir, request_id, payload)
}

fn spool_payload_in(
    runtime_dir: &std::path::Path,
    request_id: &str,
    payload: &IngestPayload,
) -> Result<(), IpcError> {
    let spool = EventSpool::new(runtime_dir)?;
    let message = WireMessage::Ingest {
        v: IPC_WIRE_VERSION,
        request_id: request_id.into(),
        payload: payload.clone(),
    };
    spool.spool_message(&message)?;
    Ok(())
}

fn parse_hook_mode(args: &[String]) -> Result<HookMode, IpcError> {
    let mut source = None;
    let mut vendor_event = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source" => source = Some(next_value(args, &mut index)?),
            "--vendor-event" => vendor_event = Some(next_value(args, &mut index)?),
            "--hook-mode" => {}
            other => {
                return Err(IpcError::FrameRejected(format!(
                    "unknown hook flag `{other}`"
                )));
            }
        }
        index += 1;
    }
    let source = source.ok_or_else(|| IpcError::FrameRejected("--source required".into()))?;
    if !matches!(
        source.as_str(),
        "cursor" | "claudeCode" | "codex" | "generic"
    ) {
        return Err(IpcError::FrameRejected("unsupported hook source".into()));
    }
    Ok(HookMode::Vendor {
        source,
        vendor_event: vendor_event
            .ok_or_else(|| IpcError::FrameRejected("--vendor-event required".into()))?,
    })
}

fn read_vendor_hook_payload(source: &str, vendor_event: &str) -> Result<IngestPayload, IpcError> {
    let value = read_stdin_json()?;
    vendor_hook_payload(source, vendor_event, &value)
}

fn vendor_hook_payload(
    source: &str,
    vendor_event: &str,
    value: &serde_json::Value,
) -> Result<IngestPayload, IpcError> {
    let object = value
        .as_object()
        .ok_or_else(|| IpcError::FrameRejected("vendor payload must be an object".into()))?;
    let external_session_id = ["session_id", "sessionId", "thread_id", "threadId"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .ok_or_else(|| IpcError::FrameRejected("vendor session identifier missing".into()))?;
    let tool_name = ["tool_name", "toolName"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string);
    let workspace_label = object
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            object
                .get("workspace_roots")
                .and_then(serde_json::Value::as_array)
                .and_then(|roots| roots.first())
                .and_then(serde_json::Value::as_str)
        })
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
    .find_map(|key| object.get(*key).and_then(serde_json::Value::as_i64));
    let normalized_event = vendor_event
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let (event, status, attention, summary) = match normalized_event.as_str() {
        "sessionstart" => (
            "sessionStart",
            Some("running"),
            None,
            Some("Session started"),
        ),
        "sessionend" => ("sessionEnd", Some("completed"), None, Some("Session ended")),
        "stop" => (
            "update",
            Some("waiting"),
            None,
            Some("Agent turn completed"),
        ),
        "permissionrequest" | "permission" => (
            "attention",
            None,
            Some("permission"),
            Some("Permission request observed"),
        ),
        "pretooluse" | "posttooluse" => ("tool", None, None, Some("Tool activity observed")),
        "posttoolusefailure" => ("tool", None, None, Some("Tool activity failed")),
        "userpromptsubmit" => ("update", Some("running"), None, Some("Agent turn started")),
        _ => (
            "lifecycle",
            None,
            None,
            Some("Agent lifecycle event observed"),
        ),
    };

    let payload = IngestPayload {
        source: source.to_string(),
        event: event.to_string(),
        session_id: None,
        external_session_id: Some(external_session_id),
        label: (event == "sessionStart").then(|| format!("{source} session")),
        workspace_label,
        status: status.map(str::to_string),
        attention: attention.map(str::to_string),
        summary: summary.map(str::to_string),
        tool_name,
        // Shipped vendor hooks do not provide a trustworthy PID/start-time
        // identity pair. Raw PID fields are intentionally ignored.
        pid: None,
        process_started_at_ms: None,
        occurred_at_ms,
    };
    notch_ipc::validate_ingest_payload(&payload)?;
    Ok(payload)
}

fn read_vendor_payload() -> Result<IngestPayload, IpcError> {
    let value = read_stdin_json()?;
    vendor_json_to_payload(&value)
}

fn read_stdin_json() -> Result<serde_json::Value, IpcError> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(IpcError::Io)?;
    if input.trim().is_empty() {
        return Err(IpcError::FrameRejected("empty stdin".into()));
    }
    serde_json::from_str(&input).map_err(|err| IpcError::FrameRejected(err.to_string()))
}

fn parse_emit_args(args: &[String]) -> Result<IngestPayload, IpcError> {
    let mut source = None;
    let mut event = None;
    let mut session_id = None;
    let mut external_session_id = None;
    let mut label = None;
    let mut workspace_label = None;
    let mut status = None;
    let mut attention = None;
    let mut summary = None;
    let mut tool_name = None;
    let mut pid = None;
    let mut process_started_at_ms = None;
    let mut occurred_at_ms = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--source" => {
                source = Some(next_value(args, &mut index)?);
            }
            "--event" => {
                event = Some(next_value(args, &mut index)?);
            }
            "--session-id" => {
                session_id = Some(next_value(args, &mut index)?);
            }
            "--external-session-id" => {
                external_session_id = Some(next_value(args, &mut index)?);
            }
            "--label" => {
                label = Some(next_value(args, &mut index)?);
            }
            "--workspace-label" => {
                workspace_label = Some(next_value(args, &mut index)?);
            }
            "--status" => {
                status = Some(next_value(args, &mut index)?);
            }
            "--attention" => {
                attention = Some(next_value(args, &mut index)?);
            }
            "--summary" => {
                summary = Some(next_value(args, &mut index)?);
            }
            "--tool-name" => {
                tool_name = Some(next_value(args, &mut index)?);
            }
            "--pid" => {
                let raw = next_value(args, &mut index)?;
                pid = Some(
                    raw.parse::<u32>()
                        .map_err(|_| IpcError::FrameRejected("invalid pid".into()))?,
                );
            }
            "--process-started-at-ms" => {
                let raw = next_value(args, &mut index)?;
                process_started_at_ms =
                    Some(raw.parse::<i64>().map_err(|_| {
                        IpcError::FrameRejected("invalid processStartedAtMs".into())
                    })?);
            }
            "--occurred-at-ms" => {
                let raw = next_value(args, &mut index)?;
                occurred_at_ms = Some(
                    raw.parse::<i64>()
                        .map_err(|_| IpcError::FrameRejected("invalid occurredAtMs".into()))?,
                );
            }
            "--fail-on-error" => {}
            other => {
                return Err(IpcError::FrameRejected(format!(
                    "unknown emit flag `{other}`"
                )));
            }
        }
        index += 1;
    }

    let payload = IngestPayload {
        source: source.ok_or_else(|| IpcError::FrameRejected("--source required".into()))?,
        event: event.ok_or_else(|| IpcError::FrameRejected("--event required".into()))?,
        session_id,
        external_session_id,
        label,
        workspace_label,
        status,
        attention,
        summary,
        tool_name,
        pid,
        process_started_at_ms,
        occurred_at_ms,
    };
    notch_ipc::validate_ingest_payload(&payload)?;
    Ok(payload)
}

fn next_value(args: &[String], index: &mut usize) -> Result<String, IpcError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| IpcError::FrameRejected("missing flag value".into()))
}

fn fail_open(mode: &HookMode) -> ExitCode {
    match mode {
        HookMode::NormalizedVendor | HookMode::Vendor { .. } => ExitCode::SUCCESS,
        HookMode::Emit { fail_on_error } if *fail_on_error => ExitCode::FAILURE,
        HookMode::Emit { .. } => ExitCode::SUCCESS,
    }
}

fn emit_failure(mode: &HookMode, _err: IpcError) -> ExitCode {
    match mode {
        HookMode::NormalizedVendor | HookMode::Vendor { .. } => ExitCode::SUCCESS,
        HookMode::Emit { .. } => ExitCode::FAILURE,
    }
}

fn is_vendor_mode(mode: &HookMode) -> bool {
    matches!(mode, HookMode::NormalizedVendor | HookMode::Vendor { .. })
}

fn is_transient_delivery_error(error: &IpcError) -> bool {
    matches!(
        error,
        IpcError::NotInitialized
            | IpcError::AuthFailed
            | IpcError::ReadTimeout
            | IpcError::DescriptorUnavailable
            | IpcError::QueueFull
            | IpcError::ConnectionClosed
            | IpcError::Io(_)
    )
}

fn print_help() {
    eprintln!(
        "llm-notch-hook — local authenticated ingest helper\n\
         \n\
         Vendor hook mode (fail-open):\n\
           llm-notch-hook hook --source cursor --vendor-event sessionStart --hook-mode\n\
         Normalized mode (default): read bounded ingest JSON from stdin, fail open on error.\n\
         \n\
         Explicit emit:\n\
           llm-notch-hook emit --source generic --event tool \\\n\
             --external-session-id ID --summary TEXT [--fail-on-error]\n\
         \n\
         Flags: --session-id --label --workspace-label --status --attention \\\n\
                --tool-name --pid --process-started-at-ms --occurred-at-ms"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_parser_requires_source_and_event() {
        let err = parse_emit_args(&[
            "--source".into(),
            "generic".into(),
            "--event".into(),
            "tool".into(),
            "--external-session-id".into(),
            "abc".into(),
            "--summary".into(),
            "hello".into(),
        ])
        .expect("payload");
        assert_eq!(err.source, "generic");
    }

    #[test]
    fn vendor_mapping_ignores_raw_tool_input_and_output() {
        let value = serde_json::json!({
            "session_id": "cursor-session-42",
            "tool_name": "Shell",
            "tool_input": {"command": "secret command"},
            "tool_output": "secret output"
        });
        let payload = vendor_hook_payload("cursor", "PostToolUse", &value).expect("payload");
        assert_eq!(payload.event, "tool");
        assert_eq!(payload.tool_name.as_deref(), Some("Shell"));
        assert_eq!(payload.summary.as_deref(), Some("Tool activity observed"));
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn ordinary_pre_tool_use_does_not_latch_attention_or_pid() {
        let value = serde_json::json!({
            "session_id": "cursor-session-42",
            "tool_name": "Shell",
            "pid": 42
        });
        let payload = vendor_hook_payload("cursor", "PreToolUse", &value).expect("payload");
        assert_eq!(payload.event, "tool");
        assert!(payload.attention.is_none());
        assert!(payload.pid.is_none());
        assert!(payload.process_started_at_ms.is_none());
    }

    #[test]
    fn only_explicit_permission_event_maps_to_attention() {
        let payload = vendor_hook_payload(
            "claudeCode",
            "PermissionRequest",
            &serde_json::json!({"session_id": "claude-1"}),
        )
        .expect("payload");
        assert_eq!(payload.event, "attention");
        assert_eq!(payload.attention.as_deref(), Some("permission"));
    }

    #[test]
    fn transient_delivery_failures_are_spoolable_but_rejections_are_not() {
        assert!(is_transient_delivery_error(&IpcError::AuthFailed));
        assert!(is_transient_delivery_error(&IpcError::ReadTimeout));
        assert!(is_transient_delivery_error(
            &IpcError::DescriptorUnavailable
        ));
        assert!(!is_transient_delivery_error(&IpcError::RateLimited));
        assert!(!is_transient_delivery_error(&IpcError::FrameRejected(
            "core rejected".into()
        )));
    }

    #[test]
    fn transient_vendor_failure_spools_bounded_wire_event() {
        let directory = tempfile::tempdir().unwrap();
        let payload = vendor_hook_payload(
            "cursor",
            "SessionStart",
            &serde_json::json!({"session_id": "offline"}),
        )
        .unwrap();
        let error = IpcError::AuthFailed;
        assert!(is_transient_delivery_error(&error));
        spool_payload_in(directory.path(), "offline-1", &payload).unwrap();
        let spool = EventSpool::new(directory.path()).unwrap();
        assert_eq!(spool.list_frames().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn helper_delivery_reaches_authenticated_server() {
        let directory = tempfile::tempdir().expect("temp runtime");
        let mut server = notch_ipc::start_ingest_server(notch_ipc::IngestServerConfig {
            runtime_dir: Some(directory.path().to_path_buf()),
        })
        .await
        .expect("server");
        let client = IngestClient::from_descriptor(server.descriptor()).expect("client");
        let payload = vendor_hook_payload(
            "cursor",
            "SessionStart",
            &serde_json::json!({"session_id": "helper-integration"}),
        )
        .expect("payload");

        let delivery =
            tokio::spawn(
                async move { deliver_with_client(&client, "helper-test", &payload).await },
            );
        let pending = tokio::time::timeout(std::time::Duration::from_secs(2), server.recv())
            .await
            .expect("server timeout")
            .expect("normalized ingest");
        assert!(matches!(
            pending.normalized(),
            notch_ipc::NormalizedIngest::SessionUpsert(_)
        ));
        let (_, completion) = pending.into_parts();
        completion.send(Ok(())).expect("accept");
        delivery.await.expect("delivery task").expect("delivery");
        server.shutdown().await.expect("shutdown");
    }
}
