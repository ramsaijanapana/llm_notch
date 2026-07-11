use std::fs;
use std::path::Path;

use notch_protocol::{
    AdapterCapabilities, BackupJournalEntry, BackupJournalOperation, ConnectorApplyResult,
    ConnectorFileApplyResult, ConnectorFileOutcome, ConnectorJournalEntry,
};

use crate::atomic::atomic_write;
use crate::error::ConnectorError;
use crate::hash::{sha256_file, sha256_hex};
use crate::journal::{backup_file_path, backup_timestamp, Journal};
use crate::lock::FileLock;
use crate::path_security::reject_hardlink;
use crate::plan::{PlanFileSnapshot, StoredPlan};

pub fn apply_plan(
    plan: &StoredPlan,
    journal: &Journal,
    now_ms: i64,
    capabilities: AdapterCapabilities,
) -> Result<ConnectorApplyResult, ConnectorError> {
    if plan.files.is_empty() {
        return Ok(ConnectorApplyResult {
            plan_id: plan.plan_id.clone(),
            source: plan.source,
            file_results: Vec::new(),
            capabilities,
        });
    }

    // Preflight all files before any writes.
    for file in &plan.files {
        preflight(file)?;
    }

    let mut file_results = Vec::new();
    let mut applied_backups: Vec<(PlanFileSnapshot, String)> = Vec::new();

    for file in &plan.files {
        if file.baseline_text == file.merged_text {
            file_results.push(ConnectorFileApplyResult {
                display_path: file.display_path.clone(),
                outcome: ConnectorFileOutcome::Skipped,
                backup_journal_id: None,
                applied_hash: Some(file.baseline_sha256.clone()),
                error_code: None,
                message: Some("No changes needed".into()),
            });
            continue;
        }

        match apply_single_file(file, journal, &plan.plan_id, plan.source, now_ms) {
            Ok(result) => {
                if result.outcome == ConnectorFileOutcome::Applied {
                    if let Some(id) = &result.backup_journal_id {
                        applied_backups.push((file.clone(), id.clone()));
                    }
                }
                file_results.push(result);
            }
            Err(error) => {
                compensate(&applied_backups, journal)?;
                file_results.push(ConnectorFileApplyResult {
                    display_path: file.display_path.clone(),
                    outcome: ConnectorFileOutcome::Failed,
                    backup_journal_id: None,
                    applied_hash: None,
                    error_code: Some(error.code()),
                    message: Some(error.to_string()),
                });
                return Err(ConnectorError::PartialApplyFailure);
            }
        }
    }

    let entry = ConnectorJournalEntry {
        id: Journal::new_journal_id(),
        plan_id: plan.plan_id.clone(),
        source: plan.source,
        scope: plan.scope,
        started_at_ms: now_ms,
        completed_at_ms: Some(now_ms),
        file_results: file_results.clone(),
        rollback_available: file_results
            .iter()
            .any(|result| result.outcome == ConnectorFileOutcome::Applied),
    };
    journal.record_apply(entry)?;

    Ok(ConnectorApplyResult {
        plan_id: plan.plan_id.clone(),
        source: plan.source,
        file_results,
        capabilities,
    })
}

fn preflight(file: &PlanFileSnapshot) -> Result<(), ConnectorError> {
    reject_hardlink(&file.canonical_path)?;
    if file.canonical_path.exists() {
        let actual = sha256_file(&file.canonical_path).map_err(|error| {
            ConnectorError::Internal(format!("hash failed: {error}"))
        })?;
        if actual != file.baseline_sha256 {
            return Err(ConnectorError::FileChangedSincePreview {
                expected: file.baseline_sha256.clone(),
                actual,
            });
        }
    } else if !file.is_new_file {
        return Err(ConnectorError::FileChangedSincePreview {
            expected: file.baseline_sha256.clone(),
            actual: sha256_hex(file.baseline_text.as_bytes()),
        });
    }
    Ok(())
}

fn apply_single_file(
    file: &PlanFileSnapshot,
    journal: &Journal,
    plan_id: &str,
    source: notch_protocol::AgentSource,
    now_ms: i64,
) -> Result<ConnectorFileApplyResult, ConnectorError> {
    let _lock = FileLock::acquire(&file.canonical_path)?;

    // Re-read under lock.
    let current_hash =     if file.canonical_path.exists() {
        sha256_file(&file.canonical_path).map_err(|error| {
            ConnectorError::Internal(format!("hash under lock failed: {error}"))
        })?
    } else {
        file.baseline_sha256.clone()
    };
    if current_hash != file.baseline_sha256 {
        return Err(ConnectorError::FileChangedSincePreview {
            expected: file.baseline_sha256.clone(),
            actual: current_hash,
        });
    }

    let backup_id = if file.baseline_text != file.merged_text && file.canonical_path.exists() {
        Some(create_backup(file, journal, plan_id, source, now_ms)?)
    } else {
        None
    };

    atomic_write(
        &file.canonical_path,
        file.merged_text.as_bytes(),
    )?;

    let applied_hash = sha256_hex(file.merged_text.as_bytes());
    Ok(ConnectorFileApplyResult {
        display_path: file.display_path.clone(),
        outcome: ConnectorFileOutcome::Applied,
        backup_journal_id: backup_id,
        applied_hash: Some(applied_hash),
        error_code: None,
        message: None,
    })
}

fn create_backup(
    file: &PlanFileSnapshot,
    journal: &Journal,
    plan_id: &str,
    source: notch_protocol::AgentSource,
    now_ms: i64,
) -> Result<String, ConnectorError> {
    let timestamp = backup_timestamp(now_ms);
    let backup_path = backup_file_path(&file.canonical_path, &timestamp);
    fs::copy(&file.canonical_path, &backup_path).map_err(|error| {
        ConnectorError::Internal(format!("backup copy failed: {error}"))
    })?;
    let content_sha256 = sha256_file(&backup_path).map_err(|error| {
        ConnectorError::Internal(format!("backup hash failed: {error}"))
    })?;
    let entry = BackupJournalEntry {
        id: Journal::new_backup_id(),
        plan_id: Some(plan_id.into()),
        source,
        display_path: file.display_path.clone(),
        backup_display_path: file.backup_display_path.clone(),
        content_sha256,
        applied_hash: Some(sha256_hex(file.merged_text.as_bytes())),
        operation: BackupJournalOperation::Create,
        recorded_at_ms: now_ms,
    };
    journal.record_backup(entry)
}

fn compensate(
    applied: &[(PlanFileSnapshot, String)],
    journal: &Journal,
) -> Result<(), ConnectorError> {
    for (file, backup_id) in applied.iter().rev() {
        let Some(backup) = journal.find_backup(backup_id) else {
            continue;
        };
        let current = if file.canonical_path.exists() {
            sha256_file(&file.canonical_path).unwrap_or_default()
        } else {
            String::new()
        };
        let Some(applied_hash) = &backup.applied_hash else {
            continue;
        };
        if current != *applied_hash {
            continue;
        }
        restore_from_backup(&file.canonical_path, &backup)?;
    }
    Ok(())
}

fn restore_from_backup(target: &Path, backup: &BackupJournalEntry) -> Result<(), ConnectorError> {
    let backup_path = infer_backup_path(target, &backup.backup_display_path);
    if !backup_path.exists() {
        return Err(ConnectorError::NotFound("backup file missing".into()));
    }
    let bytes = fs::read(&backup_path).map_err(|error| {
        ConnectorError::Internal(format!("backup read failed: {error}"))
    })?;
    atomic_write(target, &bytes)
}

fn infer_backup_path(target: &Path, backup_display_path: &str) -> std::path::PathBuf {
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
    use crate::adapter::{AdapterRegistry, PlanOperation};
    use crate::path_security::ScopeRoot;
    use crate::preview::build_preview;
    use notch_protocol::{AgentSource, ConnectorScope};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn apply_creates_backup_and_writes_file() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = TempDir::new().expect("tempdir");
        let journal = Journal::open(dir.path()).expect("journal");
        let registry = AdapterRegistry::new(repo.clone(), dir.path().join("llm-notch-hook.exe"));
        let adapter = registry.get(AgentSource::Cursor).expect("cursor");
        let root = ScopeRoot {
            canonical: std::fs::canonicalize(dir.path()).expect("canonicalize"),
            display_prefix: "~".into(),
        };
        let (preview, stored) = build_preview(
            &registry,
            &adapter,
            ConnectorScope::User,
            PlanOperation::Install,
            &root,
            crate::plan::now_ms(),
            None,
        )
        .expect("preview");
        assert!(!preview.files[0].diff_text.is_empty());

        let result = apply_plan(
            &stored,
            &journal,
            crate::plan::now_ms(),
            AdapterCapabilities::template(AgentSource::Cursor),
        )
        .expect("apply");
        assert_eq!(result.file_results[0].outcome, ConnectorFileOutcome::Applied);
        assert!(dir.path().join(".cursor/hooks.json").exists());
    }
}
