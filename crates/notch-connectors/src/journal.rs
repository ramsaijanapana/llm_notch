use std::fs;
use std::path::{Path, PathBuf};

use notch_protocol::{
    AgentSource, BackupJournalEntry, BackupJournalOperation, ConnectorJournalEntry,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic::durable_replace;
use crate::error::ConnectorError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JournalStore {
    pub backups: Vec<BackupJournalEntry>,
    pub applies: Vec<ConnectorJournalEntry>,
}

pub struct Journal {
    path: PathBuf,
    state: Mutex<JournalStore>,
}

impl Journal {
    pub fn open(app_data_dir: &Path) -> Result<Self, ConnectorError> {
        fs::create_dir_all(app_data_dir).map_err(|error| {
            ConnectorError::Internal(format!("journal dir create failed: {error}"))
        })?;
        let path = app_data_dir.join("connector-journal.json");
        let state = if path.exists() {
            let raw = fs::read_to_string(&path).map_err(|error| {
                ConnectorError::Internal(format!("journal read failed: {error}"))
            })?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            JournalStore::default()
        };
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn record_backup(&self, entry: BackupJournalEntry) -> Result<String, ConnectorError> {
        let id = entry.id.clone();
        let mut state = self.state.lock();
        state.backups.push(entry);
        self.persist(&state)?;
        Ok(id)
    }

    pub fn record_apply(&self, entry: ConnectorJournalEntry) -> Result<(), ConnectorError> {
        let mut state = self.state.lock();
        state.applies.push(entry);
        self.persist(&state)?;
        Ok(())
    }

    pub fn find_backup(&self, id: &str) -> Option<BackupJournalEntry> {
        self.state
            .lock()
            .backups
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
    }

    pub fn list_backups(&self) -> Vec<BackupJournalEntry> {
        self.state.lock().backups.clone()
    }

    pub fn latest_backup_for_display_path(
        &self,
        source: AgentSource,
        display_path: &str,
    ) -> Option<BackupJournalEntry> {
        self.state
            .lock()
            .backups
            .iter()
            .filter(|entry| entry.source == source && entry.display_path == display_path)
            .max_by_key(|entry| entry.recorded_at_ms)
            .cloned()
    }

    pub fn new_backup_id() -> String {
        format!("bak-{}", Uuid::new_v4().simple())
    }

    pub fn new_journal_id() -> String {
        format!("jrnl-{}", Uuid::new_v4().simple())
    }

    fn persist(&self, state: &JournalStore) -> Result<(), ConnectorError> {
        let raw = serde_json::to_string_pretty(state).map_err(|error| {
            ConnectorError::Internal(format!("journal serialize failed: {error}"))
        })?;
        let temp = self
            .path
            .with_extension(format!("tmp.{}", Uuid::new_v4().simple()));
        fs::write(&temp, raw.as_bytes()).map_err(|error| {
            ConnectorError::Internal(format!("journal temp write failed: {error}"))
        })?;
        durable_replace(&temp, &self.path).map_err(|error| {
            ConnectorError::Internal(format!("journal replace failed: {error}"))
        })?;
        Ok(())
    }

    pub fn purge_journal(&self, include_backups: bool) -> Result<(u32, u32), ConnectorError> {
        let mut state = self.state.lock();
        let applies_removed = state.applies.len() as u32;
        state.applies.clear();
        let mut backups_removed = 0_u32;
        if include_backups {
            backups_removed = state.backups.len() as u32;
            state.backups.clear();
        }
        self.persist(&state)?;
        Ok((applies_removed, backups_removed))
    }

    #[cfg(test)]
    pub fn apply_count(&self) -> usize {
        self.state.lock().applies.len()
    }
}

pub fn backup_display_path(display_path: &str, timestamp: &str) -> String {
    format!("{display_path}.llm-notch.bak.{timestamp}")
}

pub fn backup_file_path(target: &Path, timestamp: &str) -> PathBuf {
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    target.with_file_name(format!("{file_name}.llm-notch.bak.{timestamp}"))
}

pub fn backup_timestamp(now_ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .map(|dt| dt.format("%Y%m%dT%H%M%S").to_string())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{
        AgentSource, BackupJournalOperation, ConnectorJournalEntry, ConnectorScope,
    };
    use tempfile::TempDir;

    #[test]
    fn purge_journal_keeps_backups_by_default() {
        let dir = TempDir::new().expect("tempdir");
        let journal = Journal::open(dir.path()).expect("open");
        let entry = BackupJournalEntry {
            id: Journal::new_backup_id(),
            plan_id: Some("plan-1".into()),
            source: AgentSource::Cursor,
            display_path: "~/.cursor/hooks.json".into(),
            backup_display_path: "~/.cursor/hooks.json.llm-notch.bak.20260711T110300".into(),
            content_sha256: "abc".into(),
            applied_hash: Some("def".into()),
            operation: BackupJournalOperation::Create,
            recorded_at_ms: 1,
        };
        journal.record_backup(entry.clone()).expect("record");
        journal
            .record_apply(ConnectorJournalEntry {
                id: Journal::new_journal_id(),
                plan_id: "plan-1".into(),
                source: AgentSource::Cursor,
                scope: ConnectorScope::User,
                started_at_ms: 1,
                completed_at_ms: Some(2),
                file_results: Vec::new(),
                rollback_available: true,
            })
            .expect("apply");
        let (applies, backups) = journal.purge_journal(false).expect("purge");
        assert_eq!(applies, 1);
        assert_eq!(backups, 0);
        assert_eq!(journal.find_backup(&entry.id), Some(entry));
    }

    #[test]
    fn journal_replace_updates_multiple_times() {
        let dir = TempDir::new().expect("tempdir");
        let journal = Journal::open(dir.path()).expect("open");
        for index in 0..5 {
            journal
                .record_apply(ConnectorJournalEntry {
                    id: Journal::new_journal_id(),
                    plan_id: format!("plan-{index}"),
                    source: AgentSource::Cursor,
                    scope: ConnectorScope::User,
                    started_at_ms: index,
                    completed_at_ms: Some(index),
                    file_results: Vec::new(),
                    rollback_available: false,
                })
                .expect("record");
        }
        assert_eq!(journal.apply_count(), 5);
    }
}
