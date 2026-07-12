//! Reads bounded hook spool frames produced by `notch-ipc` when the relay is the sink.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::hook_ingest::{HookIngestError, RelayHookPayload, validate_hook_payload};

const SPOOL_DIRNAME: &str = "spool";
const MAX_FRAME_BYTES: usize = 256 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum SpoolEvent {
    Forwardable(RelayHookPayload),
    Ignored,
}

pub struct EventSpoolReader {
    runtime_dir: PathBuf,
}

impl EventSpoolReader {
    pub fn new(runtime_dir: impl Into<PathBuf>) -> Self {
        Self {
            runtime_dir: runtime_dir.into(),
        }
    }

    pub fn spool_dir(&self) -> PathBuf {
        self.runtime_dir.join(SPOOL_DIRNAME)
    }

    pub fn list_frames(&self) -> Result<Vec<PathBuf>, String> {
        let dir = self.spool_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("frame") {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn decode_frame(&self, path: &Path) -> Result<Option<SpoolEvent>, String> {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        let payload = decode_ingest_payload(&bytes).map_err(|error| error.to_string())?;
        let Some(payload) = payload else {
            return Ok(None);
        };
        match classify_hook_payload(&payload) {
            HookDisposition::Forward => Ok(Some(SpoolEvent::Forwardable(payload))),
            HookDisposition::Ignore => Ok(Some(SpoolEvent::Ignored)),
            HookDisposition::Reject(error) => Err(error),
        }
    }

    pub fn remove(&self, path: &Path) -> Result<(), String> {
        if path.starts_with(self.spool_dir()) {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

pub fn spawn_spool_watcher(
    runtime_dir: PathBuf,
    sender: std::sync::mpsc::Sender<Result<RelayHookPayload, String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let reader = EventSpoolReader::new(runtime_dir);
        loop {
            let frames = match reader.list_frames() {
                Ok(frames) => frames,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    thread::sleep(POLL_INTERVAL);
                    continue;
                }
            };
            for path in frames {
                match reader.decode_frame(&path) {
                    Ok(Some(SpoolEvent::Forwardable(payload))) => {
                        let _ = sender.send(Ok(payload));
                        let _ = reader.remove(&path);
                    }
                    Ok(Some(SpoolEvent::Ignored)) => {
                        let _ = reader.remove(&path);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = sender.send(Err(error));
                    }
                }
            }
            thread::sleep(POLL_INTERVAL);
        }
    })
}

enum HookDisposition {
    Forward,
    Ignore,
    Reject(String),
}

fn classify_hook_payload(payload: &RelayHookPayload) -> HookDisposition {
    if let Err(error) = validate_hook_payload(payload) {
        return HookDisposition::Reject(error.to_string());
    }
    let event = payload.event.to_ascii_lowercase();
    if matches!(
        event.as_str(),
        "sessionremove" | "session_remove" | "remove" | "decisionwait"
    ) {
        return HookDisposition::Ignore;
    }
    if matches!(
        event.as_str(),
        "sessionevent"
            | "event"
            | "tool"
            | "attention"
            | "status"
            | "statuschange"
            | "status_change"
            | "lifecycle"
            | "sessionstart"
            | "session_start"
            | "start"
            | "sessionend"
            | "session_end"
            | "end"
            | "complete"
            | "fail"
    ) {
        if payload
            .external_session_id
            .as_ref()
            .or(payload.session_id.as_ref())
            .is_some_and(|value| !value.is_empty())
        {
            HookDisposition::Forward
        } else {
            HookDisposition::Reject(HookIngestError::InvalidField("externalSessionId").to_string())
        }
    } else {
        HookDisposition::Ignore
    }
}

#[derive(Debug, Deserialize)]
struct WireIngestFrame {
    #[serde(rename = "type")]
    frame_type: String,
    payload: RelayHookPayload,
}

fn decode_ingest_payload(bytes: &[u8]) -> Result<Option<RelayHookPayload>, HookIngestError> {
    if bytes.len() < 4 {
        return Err(HookIngestError::InvalidField("frame"));
    }
    let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if length > MAX_FRAME_BYTES || bytes.len() != 4 + length {
        return Err(HookIngestError::InvalidField("frame"));
    }
    let body = std::str::from_utf8(&bytes[4..]).map_err(|_| HookIngestError::InvalidField("frame"))?;
    let frame: WireIngestFrame =
        serde_json::from_str(body).map_err(|_| HookIngestError::InvalidField("frame"))?;
    if frame.frame_type != "ingest" {
        return Ok(None);
    }
    crate::hook_ingest::validate_hook_payload(&frame.payload)?;
    Ok(Some(frame.payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_ingest::RelayHookPayload;

    #[test]
    fn decodes_length_prefixed_ingest_frames() {
        let payload = RelayHookPayload {
            source: "codex".into(),
            event: "tool".into(),
            session_id: None,
            external_session_id: Some("sess-1".into()),
            summary: Some("Ran tests".into()),
            occurred_at_ms: Some(42),
            tool_name: Some("run_command".into()),
            attention: None,
        };
        let body = serde_json::json!({
            "type": "ingest",
            "v": 1,
            "requestId": "req-1",
            "payload": payload,
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let mut frame = (body_bytes.len() as u32).to_be_bytes().to_vec();
        frame.extend_from_slice(&body_bytes);

        let decoded = decode_ingest_payload(&frame).expect("decode");
        assert_eq!(decoded.expect("payload"), payload);
    }
}
