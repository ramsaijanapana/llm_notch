use std::sync::Arc;

use notch_protocol::{AdapterCapabilities, AgentSource};
use tauri::State;
use uuid::Uuid;

use crate::commands::error::CommandError;
use crate::commands::types::{
    ConnectorPreview, IntegrationHealthEntry, IntegrationHealthReport, IntegrationHealthStatus,
};
use crate::commands::validation::{validate_agent_source, validate_plan_id};
use crate::state::{HostState, SystemClock};
use notch_core::Clock;

#[tauri::command]
pub fn integration_health(
    host: State<'_, Arc<HostState>>,
) -> Result<IntegrationHealthReport, CommandError> {
    let adapters = host
        .snapshot()
        .adapters
        .into_iter()
        .map(health_entry)
        .collect::<Vec<_>>();
    let overall = if adapters.is_empty()
        || adapters
            .iter()
            .all(|entry| entry.status == IntegrationHealthStatus::Unavailable)
    {
        IntegrationHealthStatus::Unavailable
    } else if adapters
        .iter()
        .all(|entry| entry.status == IntegrationHealthStatus::Healthy)
    {
        IntegrationHealthStatus::Healthy
    } else {
        IntegrationHealthStatus::Degraded
    };
    Ok(IntegrationHealthReport {
        checked_at_ms: SystemClock.now_ms(),
        overall,
        adapters,
    })
}

#[tauri::command]
pub fn preview_connector_change(source: AgentSource) -> Result<ConnectorPreview, CommandError> {
    let source = validate_agent_source(source)?;
    Ok(ConnectorPreview {
        plan_id: format!("preview-{}", Uuid::new_v4().simple()),
        source,
        summary: "Preview only: review the versioned template under integrations/; no file changes were made.".into(),
        expires_at_ms: SystemClock.now_ms() + 300_000,
    })
}

#[tauri::command]
pub fn apply_connector_change(plan_id: String) -> Result<AdapterCapabilities, CommandError> {
    validate_plan_id(&plan_id)?;
    Err(CommandError::NotAvailable(
        "automatic connector writes are intentionally disabled; apply a reviewed template manually"
            .into(),
    ))
}

#[tauri::command]
pub fn remove_connector(source: AgentSource) -> Result<(), CommandError> {
    validate_agent_source(source)?;
    Err(CommandError::NotAvailable(
        "automatic connector removal is intentionally disabled; remove the reviewed template manually"
            .into(),
    ))
}

#[tauri::command]
pub fn connector_health(
    source: AgentSource,
    host: State<'_, Arc<HostState>>,
) -> Result<IntegrationHealthEntry, CommandError> {
    let source = validate_agent_source(source)?;
    host.snapshot()
        .adapters
        .into_iter()
        .find(|adapter| adapter.source == source)
        .map(health_entry)
        .ok_or_else(|| CommandError::NotFound("adapter".into()))
}

fn health_entry(capabilities: AdapterCapabilities) -> IntegrationHealthEntry {
    let status = if capabilities.events {
        IntegrationHealthStatus::Degraded
    } else {
        IntegrationHealthStatus::Unavailable
    };
    IntegrationHealthEntry {
        source: capabilities.source,
        status,
        capabilities,
        detail: Some(
            "Capability template loaded; connector installation and recent-event health are not independently verified."
                .into(),
        ),
    }
}
