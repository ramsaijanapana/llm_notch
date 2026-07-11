use std::sync::Arc;

use notch_protocol::{
    AdapterCapabilities, AgentSource, ConnectorHealthEntry, ConnectorHealthReport,
    ConnectorPlanPreview, ConnectorScope, ConnectorUserStatus, HealthProbeAxis,
    HealthProbeOutcome, HealthProbeResult, map_probes_to_user_status,
};
use tauri::State;
use uuid::Uuid;

use crate::commands::error::CommandError;
use crate::commands::validation::{validate_agent_source, validate_plan_id};
use crate::state::{HostState, SystemClock};
use notch_core::Clock;

#[tauri::command]
pub fn integration_health(
    host: State<'_, Arc<HostState>>,
) -> Result<ConnectorHealthReport, CommandError> {
    let adapters = host
        .snapshot()
        .adapters
        .into_iter()
        .map(health_entry)
        .collect::<Vec<_>>();
    Ok(ConnectorHealthReport {
        checked_at_ms: SystemClock.now_ms(),
        adapters,
    })
}

#[tauri::command]
pub fn preview_connector_change(source: AgentSource) -> Result<ConnectorPlanPreview, CommandError> {
    let source = validate_agent_source(source)?;
    Ok(ConnectorPlanPreview {
        plan_id: format!("preview-{}", Uuid::new_v4().simple()),
        source,
        scope: ConnectorScope::User,
        summary: "Preview only: review the versioned template under integrations/; no file changes were made.".into(),
        expires_at_ms: SystemClock.now_ms() + i64::try_from(notch_protocol::CONNECTOR_PLAN_TTL_MS).expect("CONNECTOR_PLAN_TTL_MS fits in i64"),
        files: Vec::new(),
        external_trust_actions: Vec::new(),
        backup_display_hint: None,
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
) -> Result<ConnectorHealthEntry, CommandError> {
    let source = validate_agent_source(source)?;
    host.snapshot()
        .adapters
        .into_iter()
        .find(|adapter| adapter.source == source)
        .map(health_entry)
        .ok_or_else(|| CommandError::NotFound("adapter".into()))
}

fn health_entry(capabilities: AdapterCapabilities) -> ConnectorHealthEntry {
    let probes = template_loaded_probes(&capabilities);
    let status = map_probes_to_user_status(&probes);
    ConnectorHealthEntry {
        source: capabilities.source,
        status,
        probes,
        capabilities,
        detail: Some(
            "Capability template loaded; connector installation and recent-event health are not independently verified."
                .into(),
        ),
    }
}

fn template_loaded_probes(capabilities: &AdapterCapabilities) -> Vec<HealthProbeResult> {
    vec![
        HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Warn,
            failure_kind: None,
            detail: Some("Template loaded; install state not verified".into()),
        },
        HealthProbeResult {
            axis: HealthProbeAxis::Trust,
            outcome: if capabilities.requires_external_trust {
                HealthProbeOutcome::Warn
            } else {
                HealthProbeOutcome::Ok
            },
            failure_kind: None,
            detail: capabilities.requires_external_trust.then_some(
                "External trust step may be required (e.g. Codex /hooks review)".into(),
            ),
        },
        HealthProbeResult {
            axis: HealthProbeAxis::Traffic,
            outcome: if capabilities.events {
                HealthProbeOutcome::Warn
            } else {
                HealthProbeOutcome::Fail
            },
            failure_kind: None,
            detail: Some("No recent events observed".into()),
        },
        HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        },
    ]
}
