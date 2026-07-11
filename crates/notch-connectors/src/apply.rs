use std::path::Path;

use notch_protocol::{
    AdapterCapabilities, BackupJournalEntry, BackupJournalOperation, ConnectorApplyResult,
    ConnectorFileApplyResult, ConnectorFileOutcome, ConnectorJournalEntry,
};

use crate::atomic::atomic_write_with_revalidate;
use crate::error::ConnectorError;
use crate::hash::{read_and_hash, sha256_file, sha256_hex};
use crate::journal::{Journal, backup_timestamp};
use crate::lock::FileLock;
use crate::path_security::{
    assert_parent_chain_safe, reject_hardlink, revalidate_locked_target, secure_backup_path,
    write_exclusive_file,
};
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
        let actual = sha256_file(&file.canonical_path)
            .map_err(|error| ConnectorError::Internal(format!("hash failed: {error}")))?;
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

fn revalidate_for(file: &PlanFileSnapshot) -> impl FnMut() -> Result<(), ConnectorError> + '_ {
    move || {
        revalidate_locked_target(
            &file.scope_canonical,
            &file.relative_path,
            &file.canonical_path,
        )
        .map(|_| ())
    }
}

fn apply_single_file(
    file: &PlanFileSnapshot,
    journal: &Journal,
    plan_id: &str,
    source: notch_protocol::AgentSource,
    now_ms: i64,
) -> Result<ConnectorFileApplyResult, ConnectorError> {
    let _lock = FileLock::acquire(&file.canonical_path)?;
    let mut revalidate = revalidate_for(file);

    revalidate()?;

    // Re-read under lock.
    let current_hash = if file.canonical_path.exists() {
        let (_, hash) = read_and_hash(&file.canonical_path).map_err(|error| {
            ConnectorError::Internal(format!("hash under lock failed: {error}"))
        })?;
        hash
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
        revalidate()?;
        Some(create_backup(file, journal, plan_id, source, now_ms, &mut revalidate)?)
    } else {
        None
    };

    revalidate()?;
    atomic_write_with_revalidate(
        &file.canonical_path,
        file.merged_text.as_bytes(),
        &mut revalidate,
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
    revalidate: &mut impl FnMut() -> Result<(), ConnectorError>,
) -> Result<String, ConnectorError> {
    revalidate()?;
    let timestamp = backup_timestamp(now_ms);
    let backup_path = secure_backup_path(&file.canonical_path, &timestamp)?;

    revalidate()?;
    let (bytes, content_sha256) = read_and_hash(&file.canonical_path).map_err(|error| {
        ConnectorError::Internal(format!("backup read failed: {error}"))
    })?;

    revalidate()?;
    write_exclusive_file(&backup_path, &bytes)?;

    let backup_file_name = backup_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.backup_display_path.clone());
    let backup_display_path = if file.display_path.starts_with("~/") {
        format!("~/{backup_file_name}")
    } else if let Some(parent) = file.display_path.rsplit_once('/') {
        format!("{}/{}", parent.0, backup_file_name)
    } else {
        backup_file_name
    };
    let entry = BackupJournalEntry {
        id: Journal::new_backup_id(),
        plan_id: Some(plan_id.into()),
        source,
        display_path: file.display_path.clone(),
        backup_display_path,
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
        let _lock = FileLock::acquire(&file.canonical_path)?;
        let mut revalidate = revalidate_for(file);
        revalidate()?;

        let current = if file.canonical_path.exists() {
            read_and_hash(&file.canonical_path)
                .map(|(_, hash)| hash)
                .unwrap_or_default()
        } else {
            String::new()
        };
        let Some(applied_hash) = &backup.applied_hash else {
            continue;
        };
        if current != *applied_hash {
            continue;
        }
        restore_from_backup(file, &backup, &mut revalidate)?;
    }
    Ok(())
}

fn restore_from_backup(
    file: &PlanFileSnapshot,
    backup: &BackupJournalEntry,
    revalidate: &mut impl FnMut() -> Result<(), ConnectorError>,
) -> Result<(), ConnectorError> {
    revalidate()?;
    let backup_path = infer_backup_path(&file.canonical_path, &backup.backup_display_path);
    if let Some(parent) = backup_path.parent() {
        assert_parent_chain_safe(parent)?;
    }
    if !backup_path.exists() {
        return Err(ConnectorError::NotFound("backup file missing".into()));
    }

    revalidate()?;
    let (bytes, backup_hash) = read_and_hash(&backup_path).map_err(|error| {
        ConnectorError::Internal(format!("backup read failed: {error}"))
    })?;
    if backup_hash != backup.content_sha256 {
        return Err(ConnectorError::RollbackHashMismatch);
    }

    revalidate()?;
    atomic_write_with_revalidate(&file.canonical_path, &bytes, revalidate)
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
    use crate::path_security::{ScopeRoot, write_exclusive_file};
    use crate::preview::build_preview;
    use notch_protocol::{AgentSource, ConnectorScope};
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    #[test]
    fn apply_creates_backup_and_writes_file() {
        let integrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let dir = TempDir::new().expect("tempdir");
        let journal = Journal::open(dir.path()).expect("journal");
        let registry = AdapterRegistry::new(integrations.clone(), dir.path().join("llm-notch-hook.exe"));
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
        assert_eq!(
            result.file_results[0].outcome,
            ConnectorFileOutcome::Applied
        );
        assert!(dir.path().join(".cursor/hooks.json").exists());
    }

    #[test]
    fn backup_exclusive_create_rejects_preexisting_path() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        fs::write(&target, b"original").expect("write");
        let backup_path = secure_backup_path(&target, "20260711T120000").expect("path");
        fs::write(&backup_path, b"stale").expect("pre-create backup");

        let err = write_exclusive_file(&backup_path, b"new").unwrap_err();
        assert!(matches!(err, ConnectorError::PathEscapesScope(_)));
    }

    #[test]
    fn restore_rejects_backup_bytes_that_differ_from_journal_hash() {
        let dir = TempDir::new().expect("tempdir");
        let target = dir.path().join("hooks.json");
        fs::write(&target, b"applied").expect("write");
        let backup_path = dir.path().join("hooks.json.llm-notch.bak.test");
        fs::write(&backup_path, b"tampered-after-hash").expect("write");

        let file = PlanFileSnapshot {
            scope_canonical: std::fs::canonicalize(dir.path()).expect("canonicalize"),
            relative_path: PathBuf::from("hooks.json"),
            canonical_path: target.clone(),
            display_path: "~/hooks.json".into(),
            baseline_sha256: sha256_hex(b"applied"),
            baseline_text: "applied".into(),
            merged_text: "applied".into(),
            foreign_entries_preserved: Vec::new(),
            is_new_file: false,
            backup_display_path: backup_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        };
        let backup = BackupJournalEntry {
            id: Journal::new_backup_id(),
            plan_id: None,
            source: AgentSource::Cursor,
            display_path: file.display_path.clone(),
            backup_display_path: file.backup_display_path.clone(),
            content_sha256: sha256_hex(b"original"),
            applied_hash: Some(sha256_hex(b"applied")),
            operation: BackupJournalOperation::Create,
            recorded_at_ms: 0,
        };

        let _lock = FileLock::acquire(&target).expect("lock");
        let mut revalidate = revalidate_for(&file);
        let err = restore_from_backup(&file, &backup, &mut revalidate).unwrap_err();
        assert!(matches!(err, ConnectorError::RollbackHashMismatch));
    }

    #[test]
    fn apply_revalidates_parent_chain_at_each_mutating_stage() {
        use crate::path_security::TEST_REVALIDATE_COUNT;

        let dir = TempDir::new().expect("tempdir");
        let hooks = dir.path().join(".cursor/hooks.json");
        fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
        let baseline = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/connectors/cursor-user-baseline.json");
        fs::copy(baseline, &hooks).expect("seed");

        let journal = Journal::open(dir.path()).expect("journal");
        let integrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry = AdapterRegistry::new(integrations, dir.path().join("llm-notch-hook.exe"));
        let adapter = registry.get(AgentSource::Cursor).expect("cursor");
        let root = ScopeRoot {
            canonical: std::fs::canonicalize(dir.path()).expect("canonicalize"),
            display_prefix: "~".into(),
        };
        let (_, stored) = build_preview(
            &registry,
            &adapter,
            ConnectorScope::User,
            PlanOperation::Install,
            &root,
            crate::plan::now_ms(),
            None,
        )
        .expect("preview");

        TEST_REVALIDATE_COUNT.store(0, Ordering::SeqCst);
        let result = apply_plan(
            &stored,
            &journal,
            crate::plan::now_ms(),
            AdapterCapabilities::template(AgentSource::Cursor),
        )
        .expect("apply");
        assert_eq!(result.file_results[0].outcome, ConnectorFileOutcome::Applied);

        let revalidations = TEST_REVALIDATE_COUNT.load(Ordering::SeqCst);
        assert!(
            revalidations >= 5,
            "expected revalidation before each mutating apply stage, got {revalidations}"
        );
    }
}
