use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, TryRecvError};
use std::time::{Duration, Instant};

use notch_remote::{
    MAX_REMOTE_FRAME_BYTES, PROTOCOL_VERSION, RelayControl, RelayFrame, RelayHello, RelayPayload,
    ResumeCursor, event_spool, normalize_hook_payload,
};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const LOOP_POLL_INTERVAL: Duration = Duration::from_millis(50);

fn main() {
    if let Err(error) = run(std::env::args().skip(1)) {
        eprintln!("llm-notch-relay: {error}");
        std::process::exit(2);
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let (host_id, resume, event_spool_dir) = parse_args(args)?;
    let hello = RelayHello {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        connection_nonce: hex::encode(rand::random::<[u8; 32]>()),
        resume: ResumeCursor {
            last_sequence: resume,
        },
    };
    hello.validate().map_err(|error| error.to_string())?;

    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    write_json_line(&mut output, &hello)?;

    let (control_tx, control_rx) = mpsc::channel();
    let (hook_tx, hook_rx) = mpsc::channel::<Result<notch_remote::RelayHookPayload, String>>();
    let stdin_hook_tx = hook_tx.clone();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut input = BufReader::new(stdin.lock());
        loop {
            match read_control(&mut input) {
                Ok(Some(control)) => match &control {
                    RelayControl::InjectHook { payload } => {
                        if stdin_hook_tx.send(Ok(payload.clone())).is_err() {
                            break;
                        }
                    }
                    RelayControl::Shutdown => {
                        if control_tx.send(Ok(control)).is_err() {
                            break;
                        }
                        break;
                    }
                    RelayControl::Acknowledge { .. } => {
                        if control_tx.send(Ok(control)).is_err() {
                            break;
                        }
                    }
                },
                Ok(None) => {
                    let _ = control_tx.send(Ok(RelayControl::Shutdown));
                    break;
                }
                Err(error) => {
                    let _ = control_tx.send(Err(error));
                    break;
                }
            }
        }
    });

    if let Some(runtime_dir) = event_spool_dir {
        let _watcher = event_spool::spawn_spool_watcher(runtime_dir, hook_tx.clone());
    }

    let mut sequence = resume;
    let mut last_heartbeat = Instant::now();
    loop {
        drain_hook_events(&mut output, &hook_rx, &mut sequence)?;

        match control_rx.try_recv() {
            Ok(Ok(RelayControl::Shutdown)) => break,
            Ok(Ok(RelayControl::Acknowledge { .. })) => {}
            Ok(Ok(RelayControl::InjectHook { .. })) => {}
            Ok(Err(error)) => return Err(error),
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            emit_frame(&mut output, &mut sequence, RelayPayload::Heartbeat)?;
            last_heartbeat = Instant::now();
        }

        std::thread::sleep(LOOP_POLL_INTERVAL);
    }
    Ok(())
}

fn drain_hook_events(
    output: &mut impl Write,
    hook_rx: &mpsc::Receiver<Result<notch_remote::RelayHookPayload, String>>,
    sequence: &mut u64,
) -> Result<(), String> {
    while let Ok(result) = hook_rx.try_recv() {
        let payload = result?;
        match normalize_hook_payload(&payload, now_ms()) {
            Ok(Some(relay_payload)) => emit_frame(output, sequence, relay_payload)?,
            Ok(None) => {}
            Err(error) => return Err(format!("hook ingest rejected: {error}")),
        }
    }
    Ok(())
}

fn emit_frame(
    output: &mut impl Write,
    sequence: &mut u64,
    payload: RelayPayload,
) -> Result<(), String> {
    *sequence = sequence
        .checked_add(1)
        .ok_or_else(|| "relay sequence exhausted".to_string())?;
    let frame = RelayFrame {
        sequence: *sequence,
        payload,
    };
    frame
        .validate_after(&ResumeCursor {
            last_sequence: sequence.saturating_sub(1),
        })
        .map_err(|error| error.to_string())?;
    write_json_line(output, &frame)
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<(String, u64, Option<PathBuf>), String> {
    let mut host_id = None;
    let mut resume = None;
    let mut event_spool_dir = None;
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--host-id" if host_id.is_none() => host_id = Some(value),
            "--resume" if resume.is_none() => {
                resume = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| "resume must be an unsigned integer".to_string())?,
                )
            }
            "--event-spool" if event_spool_dir.is_none() => {
                event_spool_dir = Some(PathBuf::from(value))
            }
            _ => return Err(format!("unknown or duplicate argument: {flag}")),
        }
    }
    Ok((
        host_id.ok_or_else(|| "--host-id is required".to_string())?,
        resume.ok_or_else(|| "--resume is required".to_string())?,
        event_spool_dir,
    ))
}

fn read_control<R: BufRead>(reader: &mut R) -> Result<Option<RelayControl>, String> {
    let bytes =
        read_bounded_line(reader, MAX_REMOTE_FRAME_BYTES).map_err(|error| error.to_string())?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let control: RelayControl =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid control JSON: {error}"))?;
    control.validate().map_err(|error| error.to_string())?;
    Ok(Some(control))
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    loop {
        let buffer = reader.fill_buf().map_err(|error| error.to_string())?;
        if buffer.is_empty() {
            return Ok(if bytes.is_empty() {
                None
            } else {
                Err("control frame is oversized or unterminated".to_string())?
            });
        }
        if let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
            bytes.extend_from_slice(&buffer[..=position]);
            reader.consume(position + 1);
            break;
        }
        if bytes.len() + buffer.len() > max_bytes {
            return Err("control frame is oversized or unterminated".into());
        }
        let consumed = buffer.len();
        bytes.extend_from_slice(buffer);
        reader.consume(consumed);
    }
    if bytes.len() > max_bytes || !bytes.ends_with(b"\n") {
        return Err("control frame is oversized or unterminated".into());
    }
    Ok(Some(bytes))
}

fn write_json_line(writer: &mut impl Write, value: &impl serde::Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_strict_and_complete() {
        let parsed = parse_args(
            ["--host-id", "build-1", "--resume", "42"]
                .into_iter()
                .map(str::to_string),
        )
        .unwrap();
        assert_eq!(parsed, ("build-1".into(), 42, None));
        assert!(
            parse_args(
                ["--host-id", "build-1", "--extra", "value"]
                    .into_iter()
                    .map(str::to_string)
            )
            .is_err()
        );
    }

    #[test]
    fn control_reader_accepts_acknowledgements() {
        let mut input =
            std::io::Cursor::new(b"{\"type\":\"acknowledge\",\"cursor\":{\"lastSequence\":7}}\n");
        assert_eq!(
            read_control(&mut input).unwrap(),
            Some(RelayControl::Acknowledge {
                cursor: ResumeCursor { last_sequence: 7 }
            })
        );
    }

    #[test]
    fn control_reader_accepts_hook_injection() {
        let mut input = std::io::Cursor::new(
            b"{\"type\":\"injectHook\",\"payload\":{\"source\":\"codex\",\"event\":\"tool\",\"externalSessionId\":\"sess-1\",\"summary\":\"Ran tests\",\"occurredAtMs\":42}}\n"
                .to_vec(),
        );
        let control = read_control(&mut input).expect("read");
        assert!(matches!(control, Some(RelayControl::InjectHook { .. })));
    }

    #[test]
    fn emit_frame_advances_sequence_monotonically() {
        let mut buffer = Vec::new();
        let mut sequence = 3;
        emit_frame(
            &mut buffer,
            &mut sequence,
            RelayPayload::SessionEvent {
                session_id: "sess-1".into(),
                source: "codex".into(),
                summary: "Tool activity".into(),
                occurred_at_ms: 42,
                kind: None,
                tool_name: None,
                attention: None,
            },
        )
        .expect("emit");
        assert_eq!(sequence, 4);
        let line = std::str::from_utf8(&buffer).expect("utf8");
        assert!(line.contains("\"sequence\":4"));
        assert!(line.contains("\"type\":\"sessionEvent\""));
    }
}
