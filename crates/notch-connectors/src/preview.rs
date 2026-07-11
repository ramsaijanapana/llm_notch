use std::path::Path;

use notch_protocol::{
    AdapterCapabilities, ConnectorFileApplyResult, ConnectorFileOutcome, ConnectorFilePreview,
    ConnectorPlanPreview, ConnectorScope, MAX_CONNECTOR_DIFF_LEN,
};
use serde_json::Value;

use crate::adapter::{materialize_template, AdapterDescriptor, AdapterRegistry, PlanOperation};
use crate::diff::unified_diff;
use crate::error::ConnectorError;
use crate::hash::sha256_hex;
use crate::journal::{backup_display_path, backup_timestamp, backup_file_path};
use crate::merge::pretty_json;
use crate::path_security::{reject_hardlink, ScopeRoot};
use crate::plan::{new_plan_id, plan_expires_at, PlanFileSnapshot, StoredPlan};

pub fn build_preview(
    registry: &AdapterRegistry,
    adapter: &AdapterDescriptor,
    scope: ConnectorScope,
    operation: PlanOperation,
    scope_root: &ScopeRoot,
    now_ms: i64,
    workspace_root: Option<&Path>,
) -> Result<(ConnectorPlanPreview, StoredPlan), ConnectorError> {
    let target = adapter.target_for(scope);
    let display_path = scope_root.display_path(&target.relative_path);
    let canonical = scope_root.resolve(&target.relative_path)?;
    reject_hardlink(&canonical)?;

    let template_raw = adapter.load_template().map_err(|error| {
        ConnectorError::Internal(format!("template read failed: {error}"))
    })?;
    let template = materialize_template(&template_raw, registry.helper_path());

    let (baseline_text, baseline_value, is_new_file) = read_baseline(&canonical)?;
    let baseline_sha256 = sha256_hex(baseline_text.as_bytes());

    let (merged_value, foreign_entries_preserved) = match operation {
        PlanOperation::Install | PlanOperation::Repair => adapter.merge(&baseline_value, &template),
        PlanOperation::Remove => adapter.remove_managed(&baseline_value),
        PlanOperation::Rollback => {
            return Err(ConnectorError::InvalidRequest(
                "rollback plans require a journal entry".into(),
            ))
        }
    };

    let merged_text = pretty_json(&merged_value);
    if baseline_text == merged_text {
        let preview = ConnectorPlanPreview {
            plan_id: new_plan_id(),
            source: adapter.source,
            scope,
            expires_at_ms: plan_expires_at(now_ms),
            summary: empty_summary(operation),
            files: vec![ConnectorFilePreview {
                display_path: display_path.clone(),
                baseline_sha256: baseline_sha256.clone(),
                diff_text: String::new(),
                foreign_entries_preserved,
                is_new_file,
            }],
            external_trust_actions: if operation == PlanOperation::Install {
                adapter.external_trust_actions.clone()
            } else {
                Vec::new()
            },
            backup_display_hint: None,
        };
        let stored = StoredPlan {
            plan_id: preview.plan_id.clone(),
            source: adapter.source,
            scope,
            operation,
            expires_at_ms: preview.expires_at_ms,
            summary: preview.summary.clone(),
            files: vec![PlanFileSnapshot {
                canonical_path: canonical,
                display_path,
                baseline_sha256,
                baseline_text,
                merged_text,
                foreign_entries_preserved: preview.files[0].foreign_entries_preserved.clone(),
                is_new_file,
                backup_display_path: String::new(),
            }],
            rollback_backup_id: None,
        };
        return Ok((preview, stored));
    }

    let timestamp = backup_timestamp(now_ms);
    let backup_display = backup_display_path(&display_path, &timestamp);
    let diff_text = unified_diff(
        &format!("a/{display_path}"),
        &format!("b/{display_path}"),
        &baseline_text,
        &merged_text,
    );
    let diff_text = truncate_diff(diff_text);

    let preview = ConnectorPlanPreview {
        plan_id: new_plan_id(),
        source: adapter.source,
        scope,
        expires_at_ms: plan_expires_at(now_ms),
        summary: operation_summary(operation, &display_path),
        files: vec![ConnectorFilePreview {
            display_path: display_path.clone(),
            baseline_sha256: baseline_sha256.clone(),
            diff_text,
            foreign_entries_preserved: foreign_entries_preserved.clone(),
            is_new_file,
        }],
        external_trust_actions: if operation == PlanOperation::Install {
            adapter.external_trust_actions.clone()
        } else {
            Vec::new()
        },
        backup_display_hint: Some(backup_display.clone()),
    };

    let stored = StoredPlan {
        plan_id: preview.plan_id.clone(),
        source: adapter.source,
        scope,
        operation,
        expires_at_ms: preview.expires_at_ms,
        summary: preview.summary.clone(),
        files: vec![PlanFileSnapshot {
            canonical_path: canonical,
            display_path,
            baseline_sha256,
            baseline_text,
            merged_text,
            foreign_entries_preserved,
            is_new_file,
            backup_display_path: backup_display,
        }],
        rollback_backup_id: None,
    };

    let _ = workspace_root;
    Ok((preview, stored))
}

fn read_baseline(path: &Path) -> Result<(String, Value, bool), ConnectorError> {
    if !path.exists() {
        return Ok(("{}".into(), Value::Object(Default::default()), true));
    }
    let text = std::fs::read_to_string(path).map_err(|error| {
        ConnectorError::Internal(format!("read baseline failed: {error}"))
    })?;
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()));
    Ok((text, value, false))
}

fn truncate_diff(mut diff: String) -> String {
    if diff.len() > MAX_CONNECTOR_DIFF_LEN {
        diff.truncate(MAX_CONNECTOR_DIFF_LEN);
        diff.push_str("\n... [truncated]");
    }
    diff
}

fn operation_summary(operation: PlanOperation, display_path: &str) -> String {
    match operation {
        PlanOperation::Install => format!("Add llm_notch hooks to {display_path}"),
        PlanOperation::Remove => format!("Remove llm_notch hooks from {display_path}"),
        PlanOperation::Repair => format!("Repair llm_notch hooks in {display_path}"),
        PlanOperation::Rollback => format!("Rollback {display_path}"),
    }
}

fn empty_summary(operation: PlanOperation) -> String {
    match operation {
        PlanOperation::Install => {
            "Already up to date — llm_notch hooks are present; no changes needed.".into()
        }
        PlanOperation::Remove => {
            "No llm_notch hooks found — nothing to remove.".into()
        }
        PlanOperation::Repair => "Configuration matches expected llm_notch hooks.".into(),
        PlanOperation::Rollback => "Nothing to rollback.".into(),
    }
}

pub fn capabilities_for(source: notch_protocol::AgentSource) -> AdapterCapabilities {
    AdapterCapabilities::template(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterRegistry;
    use notch_protocol::AgentSource;
    use crate::path_security::ScopeRoot;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn idempotent_install_yields_empty_diff() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = TempDir::new().expect("tempdir");
        let hooks = dir.path().join(".cursor/hooks.json");
        std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
        let registry = AdapterRegistry::new(repo.clone(), dir.path().join("llm-notch-hook.exe"));
        let adapter = registry.get(AgentSource::Cursor).expect("cursor");
        let template = materialize_template(
            &adapter.load_template().expect("template"),
            registry.helper_path(),
        );
        let merged = adapter.merge(&serde_json::json!({}), &template).0;
        std::fs::write(&hooks, pretty_json(&merged)).expect("write");

        let root = ScopeRoot {
            canonical: std::fs::canonicalize(dir.path()).expect("canonicalize"),
            display_prefix: dir.path().display().to_string(),
        };
        let (preview, _) = build_preview(
            &registry,
            &adapter,
            ConnectorScope::User,
            PlanOperation::Install,
            &root,
            crate::plan::now_ms(),
            None,
        )
        .expect("preview");
        assert!(preview.files[0].diff_text.is_empty());
    }
}
