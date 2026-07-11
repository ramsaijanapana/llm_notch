use std::path::{Path, PathBuf};
use std::sync::Arc;

use notch_protocol::{
    AdapterCapabilities, ConnectorApplyResult, ConnectorHealthEntry, ConnectorHealthReport,
    ConnectorPlanPreview, ConnectorScope, AgentSource,
};
use parking_lot::Mutex;

use crate::adapter::{AdapterRegistry, PlanOperation};
use crate::apply::apply_plan;
use crate::detect::{detect_all, detect_source, DetectedConnector};
use crate::error::ConnectorError;
use crate::health::{helper_exists, probe_connector};
use crate::journal::Journal;
use crate::path_security::ScopeRoot;
use crate::plan::{now_ms, PlanStore};
use crate::preview::{build_preview, capabilities_for};
use crate::rollback::preview_rollback;

#[derive(Debug, Clone)]
pub struct ConnectorConfig {
    pub repo_root: PathBuf,
    pub app_data_dir: PathBuf,
    pub helper_path: PathBuf,
    pub workspace_root: Option<PathBuf>,
    /// Test-only override for user-scope root resolution.
    pub user_scope_root: Option<PathBuf>,
}

pub struct ConnectorManager {
    config: ConnectorConfig,
    registry: AdapterRegistry,
    plans: PlanStore,
    journal: Journal,
    last_event_at: Mutex<std::collections::HashMap<String, i64>>,
}

impl ConnectorManager {
    pub fn new(config: ConnectorConfig) -> Result<Self, ConnectorError> {
        let journal = Journal::open(&config.app_data_dir)?;
        let registry = AdapterRegistry::new(config.repo_root.clone(), config.helper_path.clone());
        Ok(Self {
            config,
            registry,
            plans: PlanStore::new(),
            journal,
            last_event_at: Mutex::new(std::collections::HashMap::new()),
        })
    }

    pub fn detect_all(&self) -> Result<Vec<DetectedConnector>, ConnectorError> {
        detect_all(
            &self.registry,
            self.config.workspace_root.as_deref(),
        )
    }

    pub fn detect_source(&self, source: AgentSource) -> Result<Vec<DetectedConnector>, ConnectorError> {
        detect_source(
            &self.registry,
            source,
            self.config.workspace_root.as_deref(),
        )
    }

    pub fn preview_install(
        &self,
        source: AgentSource,
        scope: ConnectorScope,
    ) -> Result<ConnectorPlanPreview, ConnectorError> {
        self.preview(source, scope, PlanOperation::Install)
    }

    pub fn preview_remove(
        &self,
        source: AgentSource,
        scope: ConnectorScope,
    ) -> Result<ConnectorPlanPreview, ConnectorError> {
        self.preview(source, scope, PlanOperation::Remove)
    }

    pub fn preview_repair(
        &self,
        source: AgentSource,
        scope: ConnectorScope,
    ) -> Result<ConnectorPlanPreview, ConnectorError> {
        self.preview(source, scope, PlanOperation::Repair)
    }

    pub fn preview_rollback(&self, backup_id: &str) -> Result<ConnectorPlanPreview, ConnectorError> {
        let now = now_ms();
        let scope_root = self.scope_root(ConnectorScope::User)?;
        let (preview, stored) =
            preview_rollback(&self.registry, &self.journal, backup_id, &scope_root, now)?;
        self.plans.insert(stored);
        Ok(preview)
    }

    pub fn apply(&self, plan_id: &str) -> Result<ConnectorApplyResult, ConnectorError> {
        let now = now_ms();
        let plan = self.plans.get_valid(plan_id, now)?;
        let capabilities = capabilities_for(plan.source);
        let result = apply_plan(&plan, &self.journal, now, capabilities)?;
        self.plans.remove(plan_id);
        Ok(result)
    }

    pub fn remove(&self, source: AgentSource, scope: ConnectorScope) -> Result<ConnectorApplyResult, ConnectorError> {
        let preview = self.preview_remove(source, scope)?;
        if preview.files.first().map(|file| file.diff_text.is_empty()).unwrap_or(true) {
            return Ok(ConnectorApplyResult {
                plan_id: preview.plan_id,
                source,
                file_results: Vec::new(),
                capabilities: capabilities_for(source),
            });
        }
        self.apply(&preview.plan_id)
    }

    pub fn health_report(&self, adapters: &[AdapterCapabilities]) -> Result<ConnectorHealthReport, ConnectorError> {
        let now = now_ms();
        let detected = self.detect_all()?;
        let helper_ok = helper_exists(&self.config.helper_path);
        let entries = adapters
            .iter()
            .filter_map(|capabilities| {
                let source = capabilities.source;
                if matches!(source, AgentSource::Generic | AgentSource::Unknown) {
                    return None;
                }
                let entry = detected
                    .iter()
                    .find(|item| item.source == source)
                    .cloned()
                    .unwrap_or(DetectedConnector {
                        source,
                        scope: ConnectorScope::User,
                        display_path: String::new(),
                        config_present: false,
                        managed_entries_present: false,
                    });
                let last_event = self
                    .last_event_at
                    .lock()
                    .get(&format!("{source:?}"))
                    .copied();
                Some(probe_connector(
                    &self.registry,
                    &entry,
                    capabilities.clone(),
                    helper_ok,
                    last_event,
                    now,
                ))
            })
            .collect();
        Ok(ConnectorHealthReport {
            checked_at_ms: now,
            adapters: entries,
        })
    }

    pub fn connector_health(
        &self,
        source: AgentSource,
        capabilities: AdapterCapabilities,
    ) -> Result<ConnectorHealthEntry, ConnectorError> {
        let now = now_ms();
        let detected = self
            .detect_source(source)?
            .into_iter()
            .find(|entry| entry.scope == ConnectorScope::User)
            .unwrap_or(DetectedConnector {
                source,
                scope: ConnectorScope::User,
                display_path: String::new(),
                config_present: false,
                managed_entries_present: false,
            });
        let last_event = self
            .last_event_at
            .lock()
            .get(&format!("{source:?}"))
            .copied();
        Ok(probe_connector(
            &self.registry,
            &detected,
            capabilities,
            helper_exists(&self.config.helper_path),
            last_event,
            now,
        ))
    }

    pub fn record_event(&self, source: AgentSource, at_ms: i64) {
        self.last_event_at
            .lock()
            .insert(format!("{source:?}"), at_ms);
    }

    pub fn list_backups(&self) -> Vec<notch_protocol::BackupJournalEntry> {
        self.journal.list_backups()
    }

    pub fn purge_journal(&self, include_backups: bool) -> Result<(u32, u32), ConnectorError> {
        self.journal.purge_journal(include_backups)
    }

    pub fn helper_path(&self) -> &Path {
        &self.config.helper_path
    }

    fn preview(
        &self,
        source: AgentSource,
        scope: ConnectorScope,
        operation: PlanOperation,
    ) -> Result<ConnectorPlanPreview, ConnectorError> {
        let adapter = self
            .registry
            .get(source)
            .ok_or_else(|| ConnectorError::NotFound(format!("unsupported source: {source:?}")))?;
        let scope_root = self.scope_root(scope)?;
        let now = now_ms();
        let (preview, stored) = build_preview(
            &self.registry,
            &adapter,
            scope,
            operation,
            &scope_root,
            now,
            self.config.workspace_root.as_deref(),
        )?;
        self.plans.insert(stored);
        Ok(preview)
    }

    fn scope_root(&self, scope: ConnectorScope) -> Result<ScopeRoot, ConnectorError> {
        match scope {
            ConnectorScope::User => {
                if let Some(root) = &self.config.user_scope_root {
                    ScopeRoot::project(root)
                } else {
                    ScopeRoot::user_home()
                }
            }
            ConnectorScope::Project => {
                let workspace = self.config.workspace_root.as_deref().ok_or_else(|| {
                    ConnectorError::InvalidRequest("project scope requires workspace root".into())
                })?;
                ScopeRoot::project(workspace)
            }
        }
    }
}

pub type SharedConnectorManager = Arc<ConnectorManager>;
