use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use notch_connectors::{ConnectorConfig, ConnectorError as ManagerError, ConnectorManager};
use notch_protocol::{
    AdapterCapabilities, AgentSession, AgentSource, ConnectorApplyError, ConnectorApplyResult,
    ConnectorHealthEntry, ConnectorHealthReport, ConnectorPlanPreview, ConnectorScope,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::warn;

use crate::commands::error::CommandError;
use crate::commands::validation::{validate_agent_source, validate_plan_id};
use crate::runtime::helper_path::resolve_helper_path;
use crate::runtime::integrations_dir::resolve_integrations_dir;
use crate::state::HostState;

type SharedManager = Arc<Mutex<ConnectorManager>>;

pub const CONNECTOR_HEALTH_CHANGED_EVENT: &str = "connector-health-changed";

static MANAGER: OnceLock<SharedManager> = OnceLock::new();
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

fn manager(app: &AppHandle) -> Result<SharedManager, CommandError> {
    if let Some(existing) = MANAGER.get() {
        return Ok(Arc::clone(existing));
    }
    let config = connector_config(app)?;
    let manager = ConnectorManager::new(config).map_err(map_connector_error)?;
    let shared = Arc::new(Mutex::new(manager));
    let _ = MANAGER.set(Arc::clone(&shared));
    Ok(shared)
}

fn connector_config(app: &AppHandle) -> Result<ConnectorConfig, CommandError> {
    let integrations_root = resolve_integrations_dir(app);
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::Internal(format!("app data dir unavailable: {error}")))?;
    std::fs::create_dir_all(&app_data_dir)
        .map_err(|error| CommandError::Internal(format!("app data dir create failed: {error}")))?;
    Ok(ConnectorConfig {
        integrations_root,
        app_data_dir,
        helper_path: resolve_helper_path(app),
        workspace_root: resolve_workspace_root(),
        user_scope_root: None,
    })
}

fn resolve_workspace_root() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LLM_NOTCH_WORKSPACE") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

fn map_connector_error(error: ManagerError) -> CommandError {
    match error {
        ManagerError::InvalidRequest(message) => CommandError::InvalidRequest(message),
        ManagerError::NotFound(message) => CommandError::NotFound(message),
        ManagerError::PlanNotFound => CommandError::NotFound("plan".into()),
        ManagerError::PlanExpired => CommandError::Conflict("plan expired".into()),
        ManagerError::FileChangedSincePreview { expected, actual } => CommandError::Conflict(
            format!("file changed since preview (expected {expected}, actual {actual})"),
        ),
        ManagerError::LockContention => CommandError::Conflict("lock contention".into()),
        ManagerError::PathEscapesScope(message) => CommandError::InvalidRequest(message),
        ManagerError::RollbackHashMismatch => {
            CommandError::Conflict("rollback hash mismatch".into())
        }
        ManagerError::PartialApplyFailure => CommandError::Conflict("partial apply failure".into()),
        ManagerError::Internal(message) => CommandError::Internal(message),
    }
}

#[tauri::command]
pub fn integration_health(
    app: AppHandle,
    host: State<'_, Arc<HostState>>,
) -> Result<ConnectorHealthReport, CommandError> {
    let adapters = host.snapshot().adapters;
    let manager = manager(&app)?;
    manager
        .lock()
        .health_report(&adapters)
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn preview_connector_change(
    app: AppHandle,
    source: AgentSource,
    scope: Option<ConnectorScope>,
) -> Result<ConnectorPlanPreview, CommandError> {
    let source = validate_agent_source(source)?;
    let scope = scope.unwrap_or(ConnectorScope::User);
    let manager = manager(&app)?;
    manager
        .lock()
        .preview_install(source, scope)
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn apply_connector_change(
    app: AppHandle,
    plan_id: String,
    selected_display_paths: Option<Vec<String>>,
) -> Result<ConnectorApplyResult, CommandError> {
    validate_plan_id(&plan_id)?;
    let manager = manager(&app)?;
    manager
        .lock()
        .apply(&plan_id, selected_display_paths.as_deref())
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn remove_connector(
    app: AppHandle,
    source: AgentSource,
    scope: Option<ConnectorScope>,
) -> Result<ConnectorApplyResult, CommandError> {
    let source = validate_agent_source(source)?;
    let scope = scope.unwrap_or(ConnectorScope::User);
    let manager = manager(&app)?;
    manager
        .lock()
        .remove(source, scope)
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn repair_connector(
    app: AppHandle,
    source: AgentSource,
    scope: Option<ConnectorScope>,
) -> Result<ConnectorPlanPreview, CommandError> {
    let source = validate_agent_source(source)?;
    let scope = scope.unwrap_or(ConnectorScope::User);
    let manager = manager(&app)?;
    manager
        .lock()
        .preview_repair(source, scope)
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn rollback_connector(
    app: AppHandle,
    backup_id: String,
) -> Result<ConnectorPlanPreview, CommandError> {
    if backup_id.is_empty() {
        return Err(CommandError::InvalidRequest("invalid backup id".into()));
    }
    let manager = manager(&app)?;
    manager
        .lock()
        .preview_rollback(&backup_id)
        .map_err(map_connector_error)
}

#[tauri::command]
pub fn connector_health(
    app: AppHandle,
    source: AgentSource,
    host: State<'_, Arc<HostState>>,
) -> Result<ConnectorHealthEntry, CommandError> {
    let source = validate_agent_source(source)?;
    let capabilities = host
        .snapshot()
        .adapters
        .into_iter()
        .find(|adapter| adapter.source == source)
        .ok_or_else(|| CommandError::NotFound("adapter".into()))?;
    let manager = manager(&app)?;
    manager
        .lock()
        .connector_health(source, capabilities)
        .map_err(map_connector_error)
}

#[allow(dead_code)]
fn connector_apply_error(
    error: ManagerError,
    partial: Option<Vec<notch_protocol::ConnectorFileApplyResult>>,
) -> ConnectorApplyError {
    let (expected_sha256, actual_sha256) = match &error {
        ManagerError::FileChangedSincePreview { expected, actual } => {
            (Some(expected.clone()), Some(actual.clone()))
        }
        _ => (None, None),
    };
    ConnectorApplyError {
        code: error.code(),
        message: error.to_string(),
        expected_sha256,
        actual_sha256,
        partial_results: partial,
    }
}

#[tauri::command]
pub fn detect_connectors(
    app: AppHandle,
) -> Result<Vec<notch_connectors::DetectedConnector>, CommandError> {
    let manager = manager(&app)?;
    manager.lock().detect_all().map_err(map_connector_error)
}

#[tauri::command]
pub fn list_connector_backups(
    app: AppHandle,
) -> Result<Vec<notch_protocol::BackupJournalEntry>, CommandError> {
    let manager = manager(&app)?;
    Ok(manager.lock().list_backups())
}

/// Initializes the connector manager before IPC ingest starts and seeds traffic
/// timestamps from persisted sessions so health probes survive restart.
pub fn initialize_connector_manager(
    app: &AppHandle,
    sessions: &[AgentSession],
) -> Result<(), CommandError> {
    let shared = manager(app)?;
    let traffic = sessions
        .iter()
        .filter(|session| session.source != AgentSource::Unknown)
        .map(|session| (session.source, session.last_event_at_ms))
        .collect::<Vec<_>>();
    shared.lock().seed_traffic_from_sessions(&traffic);
    let _ = APP_HANDLE.set(app.clone());
    Ok(())
}

fn emit_connector_health_changed() {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    if let Err(error) = app.emit(CONNECTOR_HEALTH_CHANGED_EVENT, ()) {
        warn!(%error, "connector health changed emit failed");
    }
}

/// Records IPC ingest traffic for connector health probes (Lane 8 hook).
pub fn record_connector_traffic(source: AgentSource, at_ms: i64) {
    if source == AgentSource::Unknown {
        return;
    }
    if let Some(shared) = MANAGER.get() {
        shared.lock().record_event(source, at_ms);
        emit_connector_health_changed();
    }
}

/// Purges connector journal entries; backups require explicit opt-in.
pub fn purge_connector_data(
    app: &AppHandle,
    include_backups: bool,
) -> Result<(u32, u32), CommandError> {
    let manager = manager(app)?;
    manager
        .lock()
        .purge_journal(include_backups)
        .map_err(map_connector_error)
}
