use std::path::Path;

use notch_protocol::{
    AdapterCapabilities, ConnectorHealthEntry, ConnectorUserStatus, HealthProbeAxis,
    HealthProbeFailureKind, HealthProbeOutcome, HealthProbeResult, map_probes_to_user_status,
};

use crate::adapter::AdapterRegistry;
use crate::detect::DetectedConnector;
use crate::error::ConnectorError;
use crate::merge::is_managed_command;

pub fn probe_connector(
    registry: &AdapterRegistry,
    detected: &DetectedConnector,
    capabilities: AdapterCapabilities,
    helper_exists: bool,
    last_event_at_ms: Option<i64>,
    now_ms: i64,
) -> ConnectorHealthEntry {
    let probes = build_probes(registry, detected, helper_exists, last_event_at_ms, now_ms);
    let status = map_probes_to_user_status(&probes);
    ConnectorHealthEntry {
        source: detected.source,
        status,
        probes,
        capabilities,
        detail: detail_for(status),
    }
}

fn build_probes(
    registry: &AdapterRegistry,
    detected: &DetectedConnector,
    helper_exists: bool,
    last_event_at_ms: Option<i64>,
    now_ms: i64,
) -> Vec<HealthProbeResult> {
    let installation = installation_probe(detected);
    let trust = trust_probe(registry, detected);
    let helper = helper_probe(helper_exists);
    let traffic = traffic_probe(detected, last_event_at_ms, now_ms);
    vec![installation, trust, helper, traffic]
}

fn installation_probe(detected: &DetectedConnector) -> HealthProbeResult {
    if !detected.config_present {
        return HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::NotInstalled),
            detail: Some(format!("No config at {}", detected.display_path)),
        };
    }
    if !detected.managed_entries_present {
        return HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Warn,
            failure_kind: Some(HealthProbeFailureKind::ConfigDrift),
            detail: Some("Config present but llm_notch hooks missing".into()),
        };
    }
    HealthProbeResult {
        axis: HealthProbeAxis::Installation,
        outcome: HealthProbeOutcome::Ok,
        failure_kind: None,
        detail: None,
    }
}

fn trust_probe(registry: &AdapterRegistry, detected: &DetectedConnector) -> HealthProbeResult {
    let Some(adapter) = registry.get(detected.source) else {
        return HealthProbeResult {
            axis: HealthProbeAxis::Trust,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        };
    };
    if adapter.external_trust_actions.is_empty() {
        return HealthProbeResult {
            axis: HealthProbeAxis::Trust,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        };
    }
    HealthProbeResult {
        axis: HealthProbeAxis::Trust,
        outcome: HealthProbeOutcome::Warn,
        failure_kind: Some(HealthProbeFailureKind::TrustRequired),
        detail: Some(adapter.external_trust_actions[0].instructions.clone()),
    }
}

fn helper_probe(helper_exists: bool) -> HealthProbeResult {
    if helper_exists {
        HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        }
    } else {
        HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::HelperUnavailable),
            detail: Some("Bundled llm-notch-hook helper not found".into()),
        }
    }
}

fn traffic_probe(
    detected: &DetectedConnector,
    last_event_at_ms: Option<i64>,
    now_ms: i64,
) -> HealthProbeResult {
    if !detected.managed_entries_present {
        return HealthProbeResult {
            axis: HealthProbeAxis::Traffic,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::NoTraffic),
            detail: Some("Integration not installed".into()),
        };
    }
    match last_event_at_ms {
        Some(at) if now_ms.saturating_sub(at) <= 15 * 60 * 1_000 => HealthProbeResult {
            axis: HealthProbeAxis::Traffic,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        },
        Some(at) => HealthProbeResult {
            axis: HealthProbeAxis::Traffic,
            outcome: HealthProbeOutcome::Warn,
            failure_kind: Some(HealthProbeFailureKind::NoTraffic),
            detail: Some(format!(
                "No events in {}m",
                now_ms.saturating_sub(at) / 60_000
            )),
        },
        None => HealthProbeResult {
            axis: HealthProbeAxis::Traffic,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::NoTraffic),
            detail: Some("No recent events observed".into()),
        },
    }
}

fn detail_for(status: ConnectorUserStatus) -> Option<String> {
    match status {
        ConnectorUserStatus::Connected => None,
        ConnectorUserStatus::WaitingFirstEvent => {
            Some("Hooks installed; waiting for the first agent event.".into())
        }
        ConnectorUserStatus::ActionNeeded => {
            Some("Complete the external trust step, then start an agent session.".into())
        }
        ConnectorUserStatus::DriftDetected => {
            Some("Configuration drift detected — review and repair.".into())
        }
        ConnectorUserStatus::NotInstalled => Some("Integration is not installed.".into()),
        ConnectorUserStatus::NotFound => Some("Agent config path not found.".into()),
        ConnectorUserStatus::Error => Some("Connector health probe failed.".into()),
    }
}

pub fn helper_exists(helper_path: &Path) -> bool {
    helper_path.is_file()
}

pub fn file_has_managed_entries(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    std::fs::read_to_string(path)
        .map(|text| is_managed_command(&text))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterRegistry;
    use crate::detect::DetectedConnector;
    use notch_protocol::{AgentSource, ConnectorScope};

    #[test]
    fn maps_installed_no_traffic_to_waiting() {
        let detected = DetectedConnector {
            source: AgentSource::Cursor,
            scope: ConnectorScope::User,
            display_path: "~/.cursor/hooks.json".into(),
            config_present: true,
            managed_entries_present: true,
        };
        let entry = probe_connector(
            &AdapterRegistry::new(
                std::path::PathBuf::from("."),
                std::path::PathBuf::from("llm-notch-hook.exe"),
            ),
            &detected,
            AdapterCapabilities::template(AgentSource::Cursor),
            true,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::WaitingFirstEvent);
    }
}
