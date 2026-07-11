//! Connector plan, preview, apply result, error, and journal contracts.
//!
//! Renderer apply/remove commands accept only `planId`. Paths shown to the UI are
//! display-only redactions; the connector manager keeps canonical file identities
//! backend-only and never accepts arbitrary paths from the frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::health::{ConnectorUserStatus, HealthProbeResult};
use crate::types::{AdapterCapabilities, AgentSource};

/// Install scope for connector templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ConnectorScope {
    User,
    Project,
}

/// External trust steps the user must complete outside llm_notch (e.g. Codex `/hooks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ExternalTrustActionKind {
    CodexHooksReview,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ExternalTrustAction {
    pub kind: ExternalTrustActionKind,
    /// User-facing instructions; never includes secrets or raw vendor payloads.
    pub instructions: String,
}

/// Per-file preview entry. `display_path` is redacted for UI; canonical identity is backend-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorFilePreview {
    /// Display-only redacted path label for diff review UI.
    pub display_path: String,
    pub baseline_sha256: String,
    /// Unified diff text for display; may be truncated by transport limits.
    pub diff_text: String,
    /// Hook or config entries from other tools preserved by merge.
    pub foreign_entries_preserved: Vec<String>,
    pub is_new_file: bool,
}

/// Short-lived plan returned by preview; apply accepts only `plan_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorPlanPreview {
    pub plan_id: String,
    pub source: AgentSource,
    pub scope: ConnectorScope,
    #[ts(type = "number")]
    pub expires_at_ms: i64,
    pub summary: String,
    pub files: Vec<ConnectorFilePreview>,
    pub external_trust_actions: Vec<ExternalTrustAction>,
    /// Display-only hint for where a backup will be written; not an apply input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub backup_display_hint: Option<String>,
}

/// Outcome for a single file within a multi-file apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ConnectorFileOutcome {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorFileApplyResult {
    pub display_path: String,
    pub outcome: ConnectorFileOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub backup_journal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub applied_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<ConnectorErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
}

/// Apply result for a confirmed plan. Multi-file apply is per-file atomic with honest partial success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorApplyResult {
    pub plan_id: String,
    pub source: AgentSource,
    pub file_results: Vec<ConnectorFileApplyResult>,
    pub capabilities: AdapterCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ConnectorErrorCode {
    PlanExpired,
    PlanNotFound,
    FileChangedSincePreview,
    LockContention,
    PathEscapesScope,
    PartialApplyFailure,
    ActiveConnectorBlocked,
    RollbackHashMismatch,
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorApplyError {
    pub code: ConnectorErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expected_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub actual_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub partial_results: Option<Vec<ConnectorFileApplyResult>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum BackupJournalOperation {
    Create,
    Restore,
    Recompute,
}

/// Journal entry for backup create/restore operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct BackupJournalEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub plan_id: Option<String>,
    pub source: AgentSource,
    /// Display-only target path label.
    pub display_path: String,
    /// Display-only backup path label.
    pub backup_display_path: String,
    pub content_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub applied_hash: Option<String>,
    pub operation: BackupJournalOperation,
    #[ts(type = "number")]
    pub recorded_at_ms: i64,
}

/// Connector apply/remove journal entry for rollback and audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorJournalEntry {
    pub id: String,
    pub plan_id: String,
    pub source: AgentSource,
    pub scope: ConnectorScope,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub completed_at_ms: Option<i64>,
    pub file_results: Vec<ConnectorFileApplyResult>,
    pub rollback_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorHealthEntry {
    pub source: AgentSource,
    pub status: ConnectorUserStatus,
    pub probes: Vec<HealthProbeResult>,
    pub capabilities: AdapterCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ConnectorHealthReport {
    #[ts(type = "number")]
    pub checked_at_ms: i64,
    pub adapters: Vec<ConnectorHealthEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{HealthProbeAxis, HealthProbeOutcome};
    use crate::types::{AttributionQuality, ContextOpenTier};

    fn template_capabilities(source: AgentSource) -> AdapterCapabilities {
        AdapterCapabilities::template(source)
    }

    #[test]
    fn connector_plan_preview_round_trips() {
        let preview = ConnectorPlanPreview {
            plan_id: "plan-abc".into(),
            source: AgentSource::Cursor,
            scope: ConnectorScope::User,
            expires_at_ms: 1_700_000_030_000,
            summary: "Add observation hooks".into(),
            files: vec![ConnectorFilePreview {
                display_path: "~/.cursor/hooks.json".into(),
                baseline_sha256: "abc123".into(),
                diff_text: "+ sessionStart".into(),
                foreign_entries_preserved: vec!["beforeShellExecution".into()],
                is_new_file: false,
            }],
            external_trust_actions: vec![],
            backup_display_hint: Some("~/.cursor/hooks.json.llm-notch.bak".into()),
        };

        let value = serde_json::to_value(&preview).expect("serialize");
        assert_eq!(value["planId"], "plan-abc");
        assert_eq!(
            value["files"][0]["foreignEntriesPreserved"][0],
            "beforeShellExecution"
        );

        let decoded: ConnectorPlanPreview = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, preview);
    }

    #[test]
    fn connector_apply_error_includes_hash_mismatch_fields() {
        let error = ConnectorApplyError {
            code: ConnectorErrorCode::FileChangedSincePreview,
            message: "Target changed after preview".into(),
            expected_sha256: Some("aaa".into()),
            actual_sha256: Some("bbb".into()),
            partial_results: None,
        };
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["code"], "fileChangedSincePreview");
        assert_eq!(value["expectedSha256"], "aaa");
    }

    #[test]
    fn connector_health_report_carries_probe_vector() {
        let report = ConnectorHealthReport {
            checked_at_ms: 1,
            adapters: vec![ConnectorHealthEntry {
                source: AgentSource::Cursor,
                status: ConnectorUserStatus::WaitingFirstEvent,
                probes: vec![HealthProbeResult {
                    axis: HealthProbeAxis::Traffic,
                    outcome: HealthProbeOutcome::Fail,
                    failure_kind: None,
                    detail: Some("No events in 15m".into()),
                }],
                capabilities: template_capabilities(AgentSource::Cursor),
                detail: None,
            }],
        };
        let value = serde_json::to_value(&report).expect("serialize");
        assert_eq!(value["adapters"][0]["status"], "waitingFirstEvent");
    }
}
