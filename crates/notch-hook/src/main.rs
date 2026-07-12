//! External hook helper: reads vendor JSON from stdin or explicit `emit` CLI,
//! authenticates against the host runtime descriptor, and delivers bounded ingest
//! frames. Vendor hook mode always fails open (exit 0).

mod decision;

use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use notch_ipc::{
    EventSpool, IPC_WIRE_VERSION, IngestClient, IngestPayload, IpcError, IpcResult, WireMessage,
    default_runtime_dir, enrich_ingest_with_collector_env, vendor_json_to_payload,
};
use notch_protocol::DECISION_HOOK_NEUTRAL_OUTPUT;
use tracing::{debug, warn};
use uuid::Uuid;

/// When set, hook events are written to `{runtime_dir}/spool/*.frame` instead of local IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeliveryTarget {
    spool_runtime_dir: Option<PathBuf>,
}

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

    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("collect-terminal-env") {
        return run_collect_terminal_env();
    }
    let delivery_target = parse_delivery_target(&mut args);
    let mode = match args.first().map(String::as_str) {
        None => HookMode::NormalizedVendor,
        Some("hook") => match parse_hook_mode(&args[1..]) {
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

    if let HookMode::Vendor {
        source,
        vendor_event,
    } = &mode
    {
        match run_vendor_hook(source, vendor_event, &delivery_target).await {
            Ok(stdout) => {
                println!("{stdout}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                debug!(?err, "vendor hook failed open");
                println!("{DECISION_HOOK_NEUTRAL_OUTPUT}");
                ExitCode::SUCCESS
            }
        }
    } else {
        let payload = match &mode {
            HookMode::NormalizedVendor => match read_vendor_payload() {
                Ok(payload) => payload,
                Err(err) => {
                    debug!(?err, "vendor payload unavailable");
                    return fail_open(&mode);
                }
            },
            HookMode::Emit { .. } => match parse_emit_args(&args[1..]) {
                Ok(payload) => payload,
                Err(err) => return emit_failure(&mode, err),
            },
            HookMode::Vendor { .. } => unreachable!("handled above"),
        };

        let payload = match prepare_hook_payload(payload) {
            Ok(payload) => payload,
            Err(err) => {
                debug!(?err, "hook payload rejected");
                return fail_open(&mode);
            }
        };

        let request_id = Uuid::new_v4().simple().to_string();
        match deliver_with_optional_transient_spool(
            &request_id,
            &payload,
            &delivery_target,
            is_vendor_mode(&mode),
            None,
        )
        .await
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => emit_failure(&mode, err),
        }
    }
}

async fn deliver_payload(
    request_id: &str,
    payload: &IngestPayload,
    target: &DeliveryTarget,
) -> Result<(), IpcError> {
    if let Some(runtime_dir) = &target.spool_runtime_dir {
        return spool_relay_payload_in(runtime_dir, request_id, payload);
    }
    let client = IngestClient::discover()?;
    deliver_with_client(&client, request_id, payload).await
}

async fn deliver_with_optional_transient_spool(
    request_id: &str,
    payload: &IngestPayload,
    target: &DeliveryTarget,
    spool_on_transient: bool,
    fallback_spool_dir: Option<&Path>,
) -> Result<(), IpcError> {
    match deliver_payload(request_id, payload, target).await {
        Ok(()) => Ok(()),
        Err(err) => {
            debug!(?err, "deliver failed");
            if spool_on_transient && is_transient_delivery_error(&err) {
                if let Err(spool_error) =
                    attempt_transient_spool_fallback(request_id, payload, fallback_spool_dir)
                {
                    debug!(?spool_error, "transient event could not be spooled");
                }
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

fn attempt_transient_spool_fallback(
    request_id: &str,
    payload: &IngestPayload,
    fallback_spool_dir: Option<&Path>,
) -> Result<(), IpcError> {
    match fallback_spool_dir {
        Some(runtime_dir) => spool_payload_in(runtime_dir, request_id, payload),
        None => spool_payload(request_id, payload),
    }
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
    runtime_dir: &Path,
    request_id: &str,
    payload: &IngestPayload,
) -> Result<(), IpcError> {
    write_spool_ingest(runtime_dir, request_id, payload)
}

/// Writes a relay-decodable ingest frame (bounded `RelayHookPayload` fields only).
fn spool_relay_payload_in(
    runtime_dir: &Path,
    request_id: &str,
    payload: &IngestPayload,
) -> Result<(), IpcError> {
    write_spool_ingest(runtime_dir, request_id, &relay_compatible_payload(payload))
}

fn write_spool_ingest(
    runtime_dir: &Path,
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

fn relay_compatible_payload(payload: &IngestPayload) -> IngestPayload {
    IngestPayload {
        source: payload.source.clone(),
        event: payload.event.clone(),
        session_id: payload.session_id.clone(),
        external_session_id: payload.external_session_id.clone(),
        summary: payload.summary.clone(),
        occurred_at_ms: payload.occurred_at_ms,
        label: None,
        workspace_label: None,
        status: None,
        attention: payload.attention.clone(),
        tool_name: payload.tool_name.clone(),
        pid: None,
        process_started_at_ms: None,
        terminal_session_id: None,
        tab_id: None,
        pane_id: None,
        window_handle: None,
    }
}

fn prepare_hook_payload(mut payload: IngestPayload) -> Result<IngestPayload, IpcError> {
    enrich_ingest_with_collector_env(&mut payload);
    notch_ipc::validate_ingest_payload(&payload)?;
    Ok(payload)
}

fn parse_delivery_target(args: &mut Vec<String>) -> DeliveryTarget {
    let mut spool_runtime_dir = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--spool-dir" {
            if let Some(path) = args.get(index + 1) {
                spool_runtime_dir = Some(PathBuf::from(path));
                args.drain(index..=index + 1);
                continue;
            }
        }
        index += 1;
    }
    if spool_runtime_dir.is_none() {
        spool_runtime_dir = resolve_event_spool_env();
    }
    DeliveryTarget { spool_runtime_dir }
}

fn resolve_event_spool_env() -> Option<PathBuf> {
    resolve_event_spool_from(env::var("LLM_NOTCH_EVENT_SPOOL").ok().as_deref())
}

fn resolve_event_spool_from(value: Option<&str>) -> Option<PathBuf> {
    match value {
        None | Some("") => None,
        Some("1") | Some("true") | Some("TRUE") | Some("True") => {
            remote_runtime_dir().or_else(|_| default_runtime_dir()).ok()
        }
        Some(path) => Some(PathBuf::from(path)),
    }
}

fn remote_runtime_dir() -> IpcResult<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".llm-notch"))
        .ok_or_else(|| IpcError::InvalidConfig("cannot resolve home directory".into()))
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
    let source = normalize_vendor_hook_source(&source)
        .ok_or_else(|| IpcError::FrameRejected("unsupported hook source".into()))?;
    Ok(HookMode::Vendor {
        source: source.into(),
        vendor_event: vendor_event
            .ok_or_else(|| IpcError::FrameRejected("--vendor-event required".into()))?,
    })
}

fn normalize_vendor_hook_source(source: &str) -> Option<&'static str> {
    match source.to_ascii_lowercase().as_str() {
        "cursor" => Some("cursor"),
        "claudecode" | "claude_code" | "claude-code" => Some("claudeCode"),
        "codex" => Some("codex"),
        "gemini" | "geminicli" | "gemini-cli" => Some("gemini"),
        "qwen" | "qwen-cli" | "qwencode" => Some("qwen"),
        "antigravitycli" | "antigravity-cli" | "antigravity" | "agy" => Some("antigravityCli"),
        "copilotcli" | "copilot-cli" | "copilot" => Some("copilotCli"),
        "generic" => Some("generic"),
        _ => None,
    }
}

async fn run_vendor_hook(
    source: &str,
    vendor_event: &str,
    target: &DeliveryTarget,
) -> Result<String, IpcError> {
    let value = read_stdin_json()?;
    if let Some(plan) = decision::plan_interactive_decision(source, vendor_event, &value)? {
        return Ok(decision::execute_interactive_decision(plan).await);
    }

    let payload = vendor_hook_payload(source, vendor_event, &value)?;
    let payload = prepare_hook_payload(payload)?;
    let request_id = Uuid::new_v4().simple().to_string();
    deliver_with_optional_transient_spool(&request_id, &payload, target, true, None).await?;
    Ok(DECISION_HOOK_NEUTRAL_OUTPUT.into())
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

fn vendor_hook_payload(
    source: &str,
    vendor_event: &str,
    value: &serde_json::Value,
) -> Result<IngestPayload, IpcError> {
    let object = value
        .as_object()
        .ok_or_else(|| IpcError::FrameRejected("vendor payload must be an object".into()))?;
    let external_session_id = [
        "session_id",
        "sessionId",
        "thread_id",
        "threadId",
        "conversation_id",
        "conversationId",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
    .map(str::to_string)
    .ok_or_else(|| IpcError::FrameRejected("vendor session identifier missing".into()))?;
    let tool_name = ["tool_name", "toolName"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .or_else(|| {
            object
                .get("toolCall")
                .and_then(|tool_call| tool_call.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
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
        .or_else(|| {
            object
                .get("workspacePaths")
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
        "timestamp",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(serde_json::Value::as_i64));
    let normalized_event = vendor_event
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let (event, status, attention, summary) = match (source, normalized_event.as_str()) {
        (_, "sessionstart") => (
            "sessionStart",
            Some("running"),
            None,
            Some("Session started".into()),
        ),
        (_, "sessionend") => (
            "sessionEnd",
            Some("completed"),
            None,
            Some("Session ended".into()),
        ),
        (_, "stop" | "agentstop") => (
            "update",
            Some("waiting"),
            None,
            Some("Agent turn completed".into()),
        ),
        ("gemini", "notification") => map_gemini_notification(object),
        (_, "permissionrequest" | "permission" | "notification") => (
            "attention",
            None,
            Some("permission"),
            Some("Permission request observed".into()),
        ),
        (_, "pretooluse" | "posttooluse" | "beforetool" | "aftertool") => {
            ("tool", None, None, Some("Tool activity observed".into()))
        }
        (_, "posttoolusefailure") => ("tool", None, None, Some("Tool activity failed".into())),
        (_, "userpromptsubmit" | "userpromptsubmitted") => (
            "update",
            Some("running"),
            None,
            Some("Agent turn started".into()),
        ),
        _ => (
            "lifecycle",
            None,
            None,
            Some("Agent lifecycle event observed".into()),
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
        summary: summary.map(|text| text.to_string()),
        tool_name,
        // Shipped vendor hooks do not provide a trustworthy PID/start-time
        // identity pair. Raw PID fields are intentionally ignored.
        pid: None,
        process_started_at_ms: None,
        occurred_at_ms,
        terminal_session_id: read_terminal_field_from_vendor(
            object,
            &["terminal_session_id", "terminalSessionId"],
        ),
        tab_id: read_terminal_field_from_vendor(object, &["tab_id", "tabId"]),
        pane_id: read_terminal_field_from_vendor(object, &["pane_id", "paneId"]),
        window_handle: read_window_handle_from_vendor(object),
    };
    Ok(payload)
}

fn read_terminal_field_from_vendor(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_window_handle_from_vendor(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<u64> {
    ["window_handle", "windowHandle"]
        .iter()
        .find_map(|key| object.get(*key))
        .and_then(|value| match value {
            serde_json::Value::Number(number) => number.as_u64(),
            serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        })
        .filter(|value| *value > 0)
}

fn map_gemini_notification(
    object: &serde_json::Map<String, serde_json::Value>,
) -> (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    Option<String>,
) {
    let attention = object
        .get("notification_type")
        .and_then(serde_json::Value::as_str)
        .map(|notification_type| match notification_type {
            "ToolPermission" => "approval",
            _ => "question",
        })
        .unwrap_or("question");
    let summary = object
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(|message| format!("Notification: {message}"))
        .unwrap_or_else(|| "Notification observed".into());
    ("attention", None, Some(attention), Some(summary))
}

fn read_vendor_payload() -> Result<IngestPayload, IpcError> {
    let value = read_stdin_json()?;
    vendor_json_to_payload(&value)
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
    let mut terminal_session_id = None;
    let mut tab_id = None;
    let mut pane_id = None;
    let mut window_handle = None;

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
            "--terminal-session-id" => {
                terminal_session_id = Some(next_value(args, &mut index)?);
            }
            "--tab-id" => {
                tab_id = Some(next_value(args, &mut index)?);
            }
            "--pane-id" => {
                pane_id = Some(next_value(args, &mut index)?);
            }
            "--window-handle" => {
                let raw = next_value(args, &mut index)?;
                window_handle = Some(
                    raw.parse::<u64>()
                        .map_err(|_| IpcError::FrameRejected("invalid windowHandle".into()))?,
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
        terminal_session_id,
        tab_id,
        pane_id,
        window_handle,
    };
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

fn run_collect_terminal_env() -> ExitCode {
    use notch_platform::{collect_wt_metadata_from_env, collector_env_exports};

    let snapshot = collect_wt_metadata_from_env();
    for (name, value) in collector_env_exports(&snapshot) {
        if env::var(name)
            .ok()
            .is_none_or(|existing| existing.trim().is_empty())
        {
            // SAFETY: collector exports only bounded ASCII env values for the current process.
            unsafe {
                env::set_var(name, value);
            }
        }
    }

    let output = serde_json::json!({
        "terminalSessionId": snapshot.terminal_session_id,
        "tabId": snapshot.tab_id,
        "paneId": snapshot.pane_id,
        "windowHandle": snapshot.window_handle.map(|handle| handle.to_string()),
        "wtProfileId": snapshot.wt_profile_id,
        "wtProfileName": snapshot.wt_profile_name,
    });
    println!("{output}");
    ExitCode::SUCCESS
}

fn print_help() {
    eprintln!(
        "llm-notch-hook — local authenticated ingest helper\n\
         \n\
         Vendor hook mode (fail-open):\n\
           llm-notch-hook hook --source cursor --vendor-event sessionStart --hook-mode\n\
         Normalized mode (default): read bounded ingest JSON from stdin, fail open on error.\n\
         \n\
         Remote relay spool mode (no local IPC; pairs with llm-notch-relay --event-spool DIR):\n\
           LLM_NOTCH_EVENT_SPOOL=1 llm-notch-hook hook --source codex --vendor-event SessionStart\n\
           llm-notch-hook --spool-dir ~/.llm-notch hook --source codex --vendor-event SessionStart\n\
         \n\
         Explicit emit:\n\
           llm-notch-hook emit --source generic --event tool \\\n\
             --external-session-id ID --summary TEXT [--fail-on-error]\n\
         \n\
         Terminal env collector (stdout JSON; exports LLM_NOTCH_* when absent):\n\
           llm-notch-hook collect-terminal-env\n\
         \n\
         Flags: --session-id --label --workspace-label --status --attention \\\n\
                --tool-name --pid --process-started-at-ms --occurred-at-ms \\\n\
                --terminal-session-id --tab-id --pane-id --window-handle --spool-dir DIR"
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
    fn gemini_hooks_map_tools_and_notifications_without_sensitive_fields() {
        let tool = vendor_hook_payload(
            "gemini",
            "BeforeTool",
            &serde_json::json!({
                "session_id": "gemini-1",
                "tool_name": "run_shell_command",
                "tool_input": {"command": "secret"}
            }),
        )
        .expect("tool payload");
        assert_eq!(tool.event, "tool");
        assert_eq!(tool.tool_name.as_deref(), Some("run_shell_command"));
        assert!(!serde_json::to_string(&tool).unwrap().contains("secret"));

        let notification = vendor_hook_payload(
            "gemini",
            "Notification",
            &serde_json::json!({
                "session_id": "gemini-1",
                "notification_type": "ToolPermission"
            }),
        )
        .expect("notification payload");
        assert_eq!(notification.event, "attention");
        assert_eq!(notification.attention.as_deref(), Some("approval"));
    }

    fn gemini_fixture(name: &str) -> serde_json::Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/gemini")
            .join(name);
        let raw = std::fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn gemini_session_start_maps_to_running_session() {
        let payload = vendor_hook_payload(
            "gemini",
            "SessionStart",
            &gemini_fixture("session-start-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "sessionStart");
        assert_eq!(payload.status.as_deref(), Some("running"));
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("gemini-session-7c4a")
        );
    }

    #[test]
    fn gemini_tool_hooks_do_not_set_attention() {
        let payload = vendor_hook_payload(
            "gemini",
            "BeforeTool",
            &gemini_fixture("before-tool-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "tool");
        assert_eq!(payload.tool_name.as_deref(), Some("write_file"));
        assert!(payload.attention.is_none());
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("export const value"));
    }

    #[test]
    fn gemini_notification_is_observation_only_attention() {
        let payload = vendor_hook_payload(
            "gemini",
            "Notification",
            &gemini_fixture("notification-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "attention");
        assert_eq!(payload.attention.as_deref(), Some("approval"));
        assert!(
            payload
                .summary
                .as_deref()
                .unwrap_or("")
                .starts_with("Notification:")
        );
    }

    #[test]
    fn relay_compatible_payload_omits_extra_wire_fields() {
        let payload = vendor_hook_payload(
            "cursor",
            "SessionStart",
            &serde_json::json!({"session_id": "sess-1"}),
        )
        .expect("payload");
        let relay = relay_compatible_payload(&payload);
        assert!(relay.label.is_none());
        assert!(relay.workspace_label.is_none());
        assert!(relay.status.is_none());
        assert!(relay.attention.is_none());
        assert!(relay.tool_name.is_none());
        let encoded = serde_json::to_string(&relay).expect("serialize");
        assert!(!encoded.contains("workspaceLabel"));
        assert!(!encoded.contains("toolName"));
    }

    #[test]
    fn event_spool_env_resolves_truthy_and_explicit_paths() {
        assert!(resolve_event_spool_from(None).is_none());
        assert!(resolve_event_spool_from(Some("")).is_none());
        assert_eq!(
            resolve_event_spool_from(Some("/tmp/remote-runtime")),
            Some(PathBuf::from("/tmp/remote-runtime"))
        );
        assert!(resolve_event_spool_from(Some("1")).is_some());
        assert!(resolve_event_spool_from(Some("true")).is_some());
    }

    #[test]
    fn spool_dir_flag_overrides_env_when_present() {
        let mut args = vec!["--spool-dir".into(), "/cli/runtime".into(), "hook".into()];
        let target = parse_delivery_target(&mut args);
        assert_eq!(
            target.spool_runtime_dir,
            Some(PathBuf::from("/cli/runtime"))
        );
        assert_eq!(args, vec!["hook".to_string()]);
    }

    #[test]
    fn spool_frame_roundtrip_matches_relay_decoder() {
        use notch_remote::event_spool::{EventSpoolReader, SpoolEvent};

        let directory = tempfile::tempdir().unwrap();
        let payload = vendor_hook_payload(
            "codex",
            "SessionStart",
            &serde_json::json!({"session_id": "remote-roundtrip"}),
        )
        .unwrap();
        spool_relay_payload_in(directory.path(), "roundtrip-1", &payload).unwrap();

        let reader = EventSpoolReader::new(directory.path());
        let frames = reader.list_frames().unwrap();
        assert_eq!(frames.len(), 1);

        let bytes = std::fs::read(&frames[0]).unwrap();
        assert!(bytes.len() >= 4);
        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        assert_eq!(bytes.len(), 4 + length);

        let decoded = reader
            .decode_frame(&frames[0])
            .unwrap()
            .expect("forwardable");
        match decoded {
            SpoolEvent::Forwardable(relay_payload) => {
                assert_eq!(relay_payload.source, "codex");
                assert_eq!(relay_payload.event, "sessionStart");
                assert_eq!(
                    relay_payload.external_session_id.as_deref(),
                    Some("remote-roundtrip")
                );
                assert_eq!(relay_payload.summary.as_deref(), Some("Session started"));
            }
            SpoolEvent::Ignored => panic!("expected forwardable spool event"),
        }
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
    async fn vendor_hook_transient_delivery_failure_spools_event() {
        let spool_dir = tempfile::tempdir().unwrap();
        let payload = prepare_hook_payload(
            vendor_hook_payload(
                "cursor",
                "SessionStart",
                &serde_json::json!({"session_id": "offline"}),
            )
            .unwrap(),
        )
        .unwrap();
        let request_id = "vendor-offline-1";
        let target = DeliveryTarget {
            spool_runtime_dir: None,
        };

        deliver_with_optional_transient_spool(
            request_id,
            &payload,
            &target,
            true,
            Some(spool_dir.path()),
        )
        .await
        .expect("vendor hook should fail open after transient delivery failure");

        let spool = EventSpool::new(spool_dir.path()).unwrap();
        assert_eq!(spool.list_frames().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn helper_delivery_reaches_authenticated_server() {
        let directory = tempfile::tempdir().expect("temp runtime");
        let mut server = notch_ipc::start_ingest_server(notch_ipc::IngestServerConfig {
            runtime_dir: Some(directory.path().to_path_buf()),
            decision_wait_tx: None,
            ..Default::default()
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

    fn antigravity_fixture(name: &str) -> serde_json::Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/antigravity-cli")
            .join(name);
        let raw = std::fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn antigravity_pre_tool_use_maps_conversation_and_tool_call() {
        let payload = vendor_hook_payload(
            "antigravityCli",
            "PreToolUse",
            &antigravity_fixture("pre-tool-use-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "tool");
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("ec33ebf9-0cba-4100-8142-c61503f6c587")
        );
        assert_eq!(payload.tool_name.as_deref(), Some("run_command"));
        assert_eq!(payload.workspace_label.as_deref(), Some("llm_notch"));
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("CommandLine"));
    }

    #[test]
    fn antigravity_post_tool_use_maps_without_sensitive_fields() {
        let payload = vendor_hook_payload(
            "antigravityCli",
            "PostToolUse",
            &antigravity_fixture("post-tool-use-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "tool");
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("ec33ebf9-0cba-4100-8142-c61503f6c587")
        );
        assert!(payload.tool_name.is_none());
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("transcriptPath"));
    }

    #[test]
    fn antigravity_stop_maps_to_waiting_update() {
        let payload = vendor_hook_payload(
            "antigravityCli",
            "Stop",
            &antigravity_fixture("stop-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "update");
        assert_eq!(payload.status.as_deref(), Some("waiting"));
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("ec33ebf9-0cba-4100-8142-c61503f6c587")
        );
    }

    fn copilot_fixture(name: &str) -> serde_json::Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/copilot")
            .join(name);
        let raw = std::fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn copilot_session_start_maps_session_id_and_workspace() {
        let payload = vendor_hook_payload(
            "copilotCli",
            "sessionStart",
            &copilot_fixture("session-start-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "sessionStart");
        assert_eq!(payload.status.as_deref(), Some("running"));
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("copilot-session-8f2a")
        );
        assert_eq!(payload.workspace_label.as_deref(), Some("llm_notch"));
        assert_eq!(payload.occurred_at_ms, Some(1_718_123_456_789));
    }

    #[test]
    fn copilot_pre_tool_use_maps_tool_name_without_args() {
        let payload = vendor_hook_payload(
            "copilotCli",
            "preToolUse",
            &copilot_fixture("pre-tool-use-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "tool");
        assert_eq!(payload.tool_name.as_deref(), Some("bash"));
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("toolArgs"));
        assert!(!encoded.contains("cargo test"));
    }

    #[test]
    fn copilot_permission_request_maps_attention_only() {
        let payload = vendor_hook_payload(
            "copilotCli",
            "permissionRequest",
            &copilot_fixture("permission-request-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "attention");
        assert_eq!(payload.attention.as_deref(), Some("permission"));
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("rm -rf"));
    }

    #[test]
    fn copilot_agent_stop_maps_to_waiting_update() {
        let payload = vendor_hook_payload(
            "copilotCli",
            "agentStop",
            &copilot_fixture("agent-stop-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.event, "update");
        assert_eq!(payload.status.as_deref(), Some("waiting"));
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("copilot-session-8f2a")
        );
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("transcriptPath"));
    }

    fn qwen_fixture(name: &str) -> serde_json::Value {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/qwen")
            .join(name);
        let raw = std::fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn qwen_session_start_maps_session_id_and_workspace() {
        let payload = vendor_hook_payload(
            "qwen",
            "SessionStart",
            &qwen_fixture("session-start-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.source, "qwen");
        assert_eq!(payload.event, "sessionStart");
        assert_eq!(payload.status.as_deref(), Some("running"));
        assert_eq!(
            payload.external_session_id.as_deref(),
            Some("qwen-session-3b91")
        );
        assert_eq!(payload.workspace_label.as_deref(), Some("llm_notch"));
    }

    #[test]
    fn qwen_pre_tool_use_maps_tool_name_without_args() {
        let payload = vendor_hook_payload(
            "qwen",
            "PreToolUse",
            &qwen_fixture("pre-tool-use-input.json"),
        )
        .expect("payload");
        assert_eq!(payload.source, "qwen");
        assert_eq!(payload.event, "tool");
        assert_eq!(payload.tool_name.as_deref(), Some("bash"));
        let encoded = serde_json::to_string(&payload).expect("serialize");
        assert!(!encoded.contains("tool_input"));
    }

    #[test]
    fn vendor_hook_without_collector_env_leaves_terminal_fields_absent() {
        let preserved = preserve_collector_env();
        clear_collector_env();
        let payload = prepare_hook_payload(
            vendor_hook_payload(
                "cursor",
                "SessionStart",
                &serde_json::json!({"session_id": "cursor-1"}),
            )
            .expect("payload"),
        )
        .expect("prepare");
        assert!(payload.terminal_session_id.is_none());
        assert!(payload.tab_id.is_none());
        assert!(payload.pane_id.is_none());
        // HWND may still be auto-discovered on Windows from a verified process-tree walk.
        #[cfg(not(windows))]
        assert!(payload.window_handle.is_none());
        restore_collector_env(preserved);
    }

    fn preserve_collector_env() -> Vec<(String, Option<String>)> {
        [
            "WT_SESSION",
            "LLM_NOTCH_TERMINAL_SESSION_ID",
            "LLM_NOTCH_TAB_ID",
            "LLM_NOTCH_PANE_ID",
            "LLM_NOTCH_WINDOW_HANDLE",
        ]
        .into_iter()
        .map(|name| (name.to_string(), env::var(name).ok()))
        .collect()
    }

    fn clear_collector_env() {
        for name in [
            "WT_SESSION",
            "LLM_NOTCH_TERMINAL_SESSION_ID",
            "LLM_NOTCH_TAB_ID",
            "LLM_NOTCH_PANE_ID",
            "LLM_NOTCH_WINDOW_HANDLE",
        ] {
            unsafe {
                env::remove_var(name);
            }
        }
    }

    fn restore_collector_env(preserved: Vec<(String, Option<String>)>) {
        for (name, value) in preserved {
            match value {
                Some(value) => unsafe {
                    env::set_var(&name, value);
                },
                None => unsafe {
                    env::remove_var(&name);
                },
            }
        }
    }

    #[test]
    fn vendor_hook_collects_terminal_metadata_from_env() {
        unsafe {
            std::env::set_var("WT_SESSION", "5720ee6d-6474-47b0-88db-fa7e10e60d37");
            std::env::set_var("LLM_NOTCH_TAB_ID", "1");
            std::env::set_var("LLM_NOTCH_PANE_ID", "0");
        }
        let payload = prepare_hook_payload(
            vendor_hook_payload(
                "cursor",
                "SessionStart",
                &serde_json::json!({"session_id": "cursor-1"}),
            )
            .expect("payload"),
        )
        .expect("prepare");
        assert_eq!(
            payload.terminal_session_id.as_deref(),
            Some("5720ee6d-6474-47b0-88db-fa7e10e60d37")
        );
        assert_eq!(payload.tab_id.as_deref(), Some("1"));
        assert_eq!(payload.pane_id.as_deref(), Some("0"));
        unsafe {
            std::env::remove_var("WT_SESSION");
            std::env::remove_var("LLM_NOTCH_TAB_ID");
            std::env::remove_var("LLM_NOTCH_PANE_ID");
        }
    }

    #[test]
    fn vendor_hook_reads_terminal_fields_from_vendor_json_when_present() {
        let payload = prepare_hook_payload(
            vendor_hook_payload(
                "cursor",
                "SessionStart",
                &serde_json::json!({
                    "session_id": "cursor-1",
                    "terminalSessionId": "0",
                    "tabId": "2",
                    "paneId": "1"
                }),
            )
            .expect("payload"),
        )
        .expect("prepare");
        assert_eq!(payload.terminal_session_id.as_deref(), Some("0"));
        assert_eq!(payload.tab_id.as_deref(), Some("2"));
        assert_eq!(payload.pane_id.as_deref(), Some("1"));
    }

    #[test]
    fn antigravity_source_aliases_normalize_for_hook_mode() {
        for alias in ["agy", "antigravity", "antigravity-cli", "antigravityCli"] {
            assert_eq!(
                normalize_vendor_hook_source(alias),
                Some("antigravityCli"),
                "alias {alias}"
            );
        }
    }
}
