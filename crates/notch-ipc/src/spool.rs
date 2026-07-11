//! Bounded atomic spool for hook events when the host is offline.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::error::{IpcError, IpcResult};
use crate::limits::{MAX_SPOOL_BYTES, MAX_SPOOL_FILES, SPOOL_DIRNAME};
use crate::wire::{WireMessage, encode_message};

pub struct EventSpool {
    dir: PathBuf,
}

static SPOOL_COUNTER: AtomicU64 = AtomicU64::new(0);

impl EventSpool {
    pub fn new(runtime_dir: &Path) -> IpcResult<Self> {
        let dir = runtime_dir.join(SPOOL_DIRNAME);
        crate::platform::ensure_runtime_dir(&dir)?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn spool_message(&self, message: &WireMessage) -> IpcResult<PathBuf> {
        self.enforce_limits()?;
        let frame = encode_message(message)?;
        let id = Uuid::new_v4();
        let occurred_at_ms = match message {
            WireMessage::Ingest { payload, .. } => payload.occurred_at_ms.unwrap_or_else(now_ms),
            _ => now_ms(),
        }
        .max(0) as u64;
        let tie_breaker = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let counter = SPOOL_COUNTER.fetch_add(1, Ordering::Relaxed);
        let stem = format!("{occurred_at_ms:020}-{tie_breaker:020}-{counter:020}-{id}");
        let tmp = self.dir.join(format!("{stem}.tmp"));
        let final_path = self.dir.join(format!("{stem}.frame"));
        fs::write(&tmp, frame).map_err(IpcError::Io)?;
        crate::platform::harden_file(&tmp)?;
        fs::rename(&tmp, &final_path).map_err(IpcError::Io)?;
        crate::platform::harden_file(&final_path)?;
        Ok(final_path)
    }

    pub fn list_frames(&self) -> IpcResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.dir).map_err(IpcError::Io)? {
            let entry = entry.map_err(IpcError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("frame") {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn remove(&self, path: &Path) -> IpcResult<()> {
        if path.starts_with(&self.dir) {
            fs::remove_file(path).map_err(IpcError::Io)?;
        }
        Ok(())
    }

    fn enforce_limits(&self) -> IpcResult<()> {
        let files = self.list_frames()?;
        if files.len() >= MAX_SPOOL_FILES {
            return Err(IpcError::SpoolLimitExceeded);
        }
        let total: u64 = files
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|meta| meta.len())
            .sum();
        if total >= MAX_SPOOL_BYTES {
            return Err(IpcError::SpoolLimitExceeded);
        }
        Ok(())
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::IPC_WIRE_VERSION;
    use crate::wire::{IngestPayload, WireMessage, decode_frame_bytes};
    use tempfile::tempdir;

    #[test]
    fn spools_and_lists_frame() {
        let dir = tempdir().expect("tempdir");
        let spool = EventSpool::new(dir.path()).expect("spool");
        let msg = WireMessage::Ack {
            v: IPC_WIRE_VERSION,
            request_id: "r1".into(),
        };
        spool.spool_message(&msg).expect("spool");
        assert_eq!(spool.list_frames().expect("list").len(), 1);
    }

    fn ingest_message(request_id: &str, event: &str, occurred_at_ms: i64) -> WireMessage {
        WireMessage::Ingest {
            v: IPC_WIRE_VERSION,
            request_id: request_id.into(),
            payload: IngestPayload {
                source: "generic".into(),
                event: event.into(),
                session_id: None,
                external_session_id: Some("ordered".into()),
                label: Some("Ordered".into()),
                workspace_label: None,
                status: None,
                attention: None,
                summary: None,
                tool_name: None,
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(occurred_at_ms),
            },
        }
    }

    #[test]
    fn spool_order_follows_event_time() {
        let dir = tempdir().expect("tempdir");
        let spool = EventSpool::new(dir.path()).expect("spool");
        spool
            .spool_message(&ingest_message("end", "sessionEnd", 2))
            .expect("end");
        spool
            .spool_message(&ingest_message("start", "sessionStart", 1))
            .expect("start");

        let frames = spool.list_frames().expect("list");
        let request_ids = frames
            .iter()
            .map(|path| {
                let bytes = fs::read(path).unwrap();
                decode_frame_bytes(&bytes).unwrap().request_id().to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(request_ids, vec!["start", "end"]);
    }
}
