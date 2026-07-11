use std::fs;

use notch_protocol::{
    AgentSource, BackupJournalEntry, BackupJournalOperation, ConnectorPlanPreview, ConnectorScope,
};

use crate::adapter::{AdapterRegistry, PlanOperation};
use crate::error::ConnectorError;
use crate::hash::sha256_file;
use crate::journal::Journal;
use crate::path_security::ScopeRoot;
use crate::plan::{new_plan_id, plan_expires_at, StoredPlan};
use crate::preview::build_preview;

pub fn preview_rollback(
    registry: &AdapterRegistry,
    journal: &Journal,
    backup_id: &str,
    scope_root: &ScopeRoot,
    now_ms: i64,
) -> Result<(ConnectorPlanPreview, StoredPlan), ConnectorError> {
    let backup = journal
        .find_backup(backup_id)
        .ok_or_else(|| ConnectorError::NotFound(format!("backup {backup_id}")))?;

    let adapter = registry
        .get(backup.source)
        .ok_or_else(|| ConnectorError::NotFound("adapter".into()))?;

    let target = adapter.target_for(ConnectorScope::User);
    let canonical = scope_root.resolve(&target.relative_path)?;
    let display_path = scope_root.display_path(&target.relative_path);

    let current_hash = if canonical.exists() {
        sha256_file(&canonical).map_err(|error| {
            ConnectorError::Internal(format!("hash current failed: {error}"))
        })?
    } else {
        String::new()
    };

    let backup_path = infer_backup_path(&canonical, &backup.backup_display_path);
    if !backup_path.exists() {
        return Err(ConnectorError::NotFound("backup file missing".into()));
    }

    let backup_bytes = fs::read(&backup_path).map_err(|error| {
        ConnectorError::Internal(format!("read backup failed: {error}"))
    })?;
    let backup_text = String::from_utf8_lossy(&backup_bytes).into_owned();
    let backup_hash = sha256_file(&backup_path).map_err(|error| {
        ConnectorError::Internal(format!("hash backup failed: {error}"))
    })?;

    if let Some(applied_hash) = &backup.applied_hash {
        if current_hash == *applied_hash {
            return build_exact_restore_preview(
                backup,
                backup_id,
                display_path,
                canonical,
                current_hash,
                backup_text,
                now_ms,
            );
        }
    }

    // Hash mismatch: recomputed additive recovery (remove managed entries only).
    build_preview(
        registry,
        &adapter,
        ConnectorScope::User,
        PlanOperation::Remove,
        scope_root,
        now_ms,
        None,
    )
}

fn build_exact_restore_preview(
    backup: BackupJournalEntry,
    backup_id: &str,
    display_path: String,
    canonical: std::path::PathBuf,
    current_hash: String,
    backup_text: String,
    now_ms: i64,
) -> Result<(ConnectorPlanPreview, StoredPlan), ConnectorError> {
    use crate::diff::unified_diff;
    use crate::plan::PlanFileSnapshot;

    let diff_text = unified_diff(
        &format!("a/{display_path}"),
        &format!("b/{display_path}"),
        &fs::read_to_string(&canonical).unwrap_or_default(),
        &backup_text,
    );

    let preview = ConnectorPlanPreview {
        plan_id: new_plan_id(),
        source: backup.source,
        scope: ConnectorScope::User,
        expires_at_ms: plan_expires_at(now_ms),
        summary: format!("Restore {display_path} from backup {backup_id}"),
        files: vec![notch_protocol::ConnectorFilePreview {
            display_path: display_path.clone(),
            baseline_sha256: current_hash.clone(),
            diff_text,
            foreign_entries_preserved: Vec::new(),
            is_new_file: false,
        }],
        external_trust_actions: Vec::new(),
        backup_display_hint: Some(backup.backup_display_path.clone()),
    };

    let baseline_text = fs::read_to_string(&canonical).unwrap_or_default();
    let stored = StoredPlan {
        plan_id: preview.plan_id.clone(),
        source: backup.source,
        scope: ConnectorScope::User,
        operation: PlanOperation::Rollback,
        expires_at_ms: preview.expires_at_ms,
        summary: preview.summary.clone(),
        files: vec![PlanFileSnapshot {
            canonical_path: canonical,
            display_path,
            baseline_sha256: current_hash,
            baseline_text,
            merged_text: backup_text,
            foreign_entries_preserved: Vec::new(),
            is_new_file: false,
            backup_display_path: backup.backup_display_path,
        }],
        rollback_backup_id: Some(backup_id.into()),
    };

    Ok((preview, stored))
}

fn infer_backup_path(target: &std::path::Path, backup_display_path: &str) -> std::path::PathBuf {
    let file_name = backup_display_path
        .rsplit('/')
        .next()
        .or_else(|| backup_display_path.rsplit('\\').next())
        .unwrap_or(backup_display_path);
    target.with_file_name(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256_hex;
    use crate::journal::{backup_file_path, backup_timestamp};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn exact_rollback_when_hash_matches() {
        let dir = TempDir::new().expect("tempdir");
        let journal = Journal::open(dir.path()).expect("journal");
        let hooks = dir.path().join(".cursor/hooks.json");
        std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
        std::fs::write(&hooks, r#"{"version":1}"#).expect("write");
        let now = crate::plan::now_ms();
        let backup_path = backup_file_path(&hooks, &backup_timestamp(now));
        std::fs::copy(&hooks, &backup_path).expect("backup");
        let entry = BackupJournalEntry {
            id: Journal::new_backup_id(),
            plan_id: Some("plan".into()),
            source: AgentSource::Cursor,
            display_path: "~/.cursor/hooks.json".into(),
            backup_display_path: format!(
                "~/.cursor/hooks.json.llm-notch.bak.{}",
                backup_timestamp(now)
            ),
            content_sha256: sha256_hex(b"{\"version\":1}"),
            applied_hash: Some(sha256_hex(b"{\"version\":1}")),
            operation: BackupJournalOperation::Create,
            recorded_at_ms: now,
        };
        journal.record_backup(entry.clone()).expect("record");

        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = AdapterRegistry::new(repo, dir.path().join("llm-notch-hook.exe"));
        let root = ScopeRoot {
            canonical: std::fs::canonicalize(dir.path()).expect("canonicalize"),
            display_prefix: "~".into(),
        };
        let (preview, _) = preview_rollback(&registry, &journal, &entry.id, &root, now)
            .expect("rollback preview");
        assert!(preview.summary.contains("Restore"));
    }
}
