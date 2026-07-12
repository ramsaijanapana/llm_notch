use std::path::Path;

use notch_protocol::{
    AdapterCapabilities, ConnectorHealthEntry, ConnectorUserStatus, HealthProbeAxis,
    HealthProbeFailureKind, HealthProbeOutcome, HealthProbeResult, map_probes_to_user_status,
};

use crate::adapter::AdapterRegistry;
use crate::detect::DetectedConnector;
use crate::merge::{ConfiguredHelperValidation, validate_configured_helper_paths};

pub fn probe_connector(
    registry: &AdapterRegistry,
    detected: &DetectedConnector,
    capabilities: AdapterCapabilities,
    expected_helper_path: &Path,
    last_event_at_ms: Option<i64>,
    now_ms: i64,
) -> ConnectorHealthEntry {
    let probes = build_probes(
        registry,
        detected,
        expected_helper_path,
        last_event_at_ms,
        now_ms,
    );
    let status = map_probes_to_user_status(&probes);
    let detail = detail_for(status, &probes);
    ConnectorHealthEntry {
        source: detected.source,
        status,
        probes,
        capabilities,
        detail,
    }
}

fn build_probes(
    registry: &AdapterRegistry,
    detected: &DetectedConnector,
    expected_helper_path: &Path,
    last_event_at_ms: Option<i64>,
    now_ms: i64,
) -> Vec<HealthProbeResult> {
    let installation = installation_probe(detected);
    let trust = trust_probe(registry, detected, last_event_at_ms);
    let helper = helper_probe(detected, expected_helper_path);
    let traffic = traffic_probe(detected, last_event_at_ms, now_ms);
    let mut probes = vec![installation, trust, helper, traffic];
    if let Some(process) = process_probe(detected) {
        probes.push(process);
    }
    probes
}

fn installation_probe(detected: &DetectedConnector) -> HealthProbeResult {
    if detected.config_present {
        if !detected.managed_entries_present {
            return HealthProbeResult {
                axis: HealthProbeAxis::Installation,
                outcome: HealthProbeOutcome::Warn,
                failure_kind: Some(HealthProbeFailureKind::ConfigDrift),
                detail: Some("Hook config present — llm_notch hooks need repair".into()),
            };
        }
        return HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Ok,
            failure_kind: None,
            detail: None,
        };
    }

    if detected.executable_present {
        let detail = detected.executable_path.as_ref().map_or_else(
            || "CLI installed — hook config missing; use Connect to wire llm_notch hooks".into(),
            |path| {
                format!(
                    "CLI installed at {path} — hook config missing; use Connect to wire llm_notch hooks"
                )
            },
        );
        return HealthProbeResult {
            axis: HealthProbeAxis::Installation,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::NotInstalled),
            detail: Some(detail),
        };
    }

    HealthProbeResult {
        axis: HealthProbeAxis::Installation,
        outcome: HealthProbeOutcome::Fail,
        failure_kind: Some(HealthProbeFailureKind::AgentNotFound),
        detail: Some(format!(
            "No config at {} and agent CLI not found on PATH",
            detected.display_path
        )),
    }
}

fn trust_probe(
    registry: &AdapterRegistry,
    detected: &DetectedConnector,
    last_event_at_ms: Option<i64>,
) -> HealthProbeResult {
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
    // Untrusted Codex hooks are skipped; observed traffic after managed hooks implies trust.
    if detected.managed_entries_present && last_event_at_ms.is_some() {
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

fn helper_probe(detected: &DetectedConnector, expected_helper_path: &Path) -> HealthProbeResult {
    if detected.managed_entries_present {
        return helper_probe_for_managed_hooks(&detected.managed_commands, expected_helper_path);
    }

    if helper_exists(expected_helper_path) {
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

fn helper_probe_for_managed_hooks(
    managed_commands: &[String],
    expected_helper_path: &Path,
) -> HealthProbeResult {
    match validate_configured_helper_paths(managed_commands, expected_helper_path) {
        ConfiguredHelperValidation::Ok => {
            if helper_exists(expected_helper_path) {
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
                    failure_kind: Some(HealthProbeFailureKind::HelperPathMissing),
                    detail: Some(format!(
                        "Hook helper path not found: {}",
                        expected_helper_path.display()
                    )),
                }
            }
        }
        ConfiguredHelperValidation::UnresolvedPlaceholder => HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::HooksMisconfigured),
            detail: Some(
                "Managed hook command still contains an unresolved helper placeholder".into(),
            ),
        },
        ConfiguredHelperValidation::PathMissing { configured } => HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::HelperPathMissing),
            detail: Some(format!(
                "Hook helper path not found: {}",
                configured.display()
            )),
        },
        ConfiguredHelperValidation::PathMismatch {
            configured,
            expected,
        } => HealthProbeResult {
            axis: HealthProbeAxis::Helper,
            outcome: HealthProbeOutcome::Fail,
            failure_kind: Some(HealthProbeFailureKind::HooksMisconfigured),
            detail: Some(format!(
                "Hook helper path {} does not match bundled helper {}",
                configured.display(),
                expected.display()
            )),
        },
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

/// Positive-only process evidence; omitted when no matching process is observed.
fn process_probe(detected: &DetectedConnector) -> Option<HealthProbeResult> {
    if !detected.process_running {
        return None;
    }
    let detail = detected
        .running_process_name
        .as_ref()
        .map(|name| format!("Agent process running ({name}); session not verified by hooks"));
    Some(HealthProbeResult {
        axis: HealthProbeAxis::Process,
        outcome: HealthProbeOutcome::Ok,
        failure_kind: None,
        detail,
    })
}

fn detail_for(status: ConnectorUserStatus, probes: &[HealthProbeResult]) -> Option<String> {
    if let Some(detail) = probes
        .iter()
        .find(|probe| probe.axis == HealthProbeAxis::Helper && probe.detail.is_some())
        .and_then(|probe| probe.detail.clone())
    {
        if matches!(
            status,
            ConnectorUserStatus::HelperMissing | ConnectorUserStatus::HooksMisconfigured
        ) {
            return Some(detail);
        }
    }

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
        ConnectorUserStatus::NotInstalled => {
            Some("Agent CLI or llm_notch hooks are not fully connected.".into())
        }
        ConnectorUserStatus::NotFound => Some("Agent config path not found.".into()),
        ConnectorUserStatus::HelperMissing => Some(
            "Hook helper binary is missing at the configured path. Use Repair to rewrite hooks."
                .into(),
        ),
        ConnectorUserStatus::HooksMisconfigured => Some(
            "Hook helper path does not match the bundled helper. Use Repair to reconcile paths."
                .into(),
        ),
        ConnectorUserStatus::Error => Some("Connector health probe failed.".into()),
    }
}

pub fn helper_exists(helper_path: &Path) -> bool {
    helper_path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterRegistry;
    use crate::detect::DetectedConnector;
    use notch_protocol::{AgentSource, ConnectorScope};

    fn detected_with_command(command: &str) -> DetectedConnector {
        DetectedConnector {
            source: AgentSource::Cursor,
            scope: ConnectorScope::User,
            display_path: "~/.cursor/hooks.json".into(),
            config_present: true,
            managed_entries_present: true,
            executable_present: true,
            executable_path: None,
            process_running: false,
            running_process_name: None,
            managed_commands: vec![command.into()],
        }
    }

    #[test]
    fn maps_installed_no_traffic_to_waiting() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let helper = dir.path().join("llm-notch-hook.exe");
        std::fs::write(&helper, b"helper").expect("write helper");
        let command = format!(
            r#""{}" hook --source cursor --vendor-event sessionStart --hook-mode"#,
            helper.display()
        );
        let detected = detected_with_command(&command);
        let entry = probe_connector(
            &AdapterRegistry::new(std::path::PathBuf::from("."), helper.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Cursor),
            &helper,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::WaitingFirstEvent);
    }

    #[test]
    fn maps_stale_debug_helper_path_to_helper_missing() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let bundled = dir.path().join("llm-notch-hook.exe");
        std::fs::write(&bundled, b"bundled").expect("write bundled");
        let detected = detected_with_command(
            r#""C:\dev\llm_notch\target\debug\llm-notch-hook.exe" hook --source cursor --vendor-event sessionStart --hook-mode"#,
        );
        let entry = probe_connector(
            &AdapterRegistry::new(std::path::PathBuf::from("."), bundled.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Cursor),
            &bundled,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::HelperMissing);
    }

    #[test]
    fn maps_wrong_existing_helper_path_to_hooks_misconfigured() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let bundled = dir.path().join("llm-notch-hook.exe");
        let stale = dir.path().join("stale-llm-notch-hook.exe");
        std::fs::write(&bundled, b"bundled").expect("write bundled");
        std::fs::write(&stale, b"stale").expect("write stale");
        let command = format!(
            r#""{}" hook --source cursor --vendor-event sessionStart --hook-mode"#,
            stale.display()
        );
        let detected = detected_with_command(&command);
        let entry = probe_connector(
            &AdapterRegistry::new(std::path::PathBuf::from("."), bundled.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Cursor),
            &bundled,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::HooksMisconfigured);
    }

    #[test]
    fn adds_process_probe_when_running_without_affecting_status() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let helper = dir.path().join("llm-notch-hook.exe");
        std::fs::write(&helper, b"helper").expect("write helper");
        let detected = DetectedConnector {
            source: AgentSource::Cursor,
            scope: ConnectorScope::User,
            display_path: "~/.cursor/hooks.json".into(),
            config_present: false,
            managed_entries_present: false,
            executable_present: false,
            executable_path: None,
            process_running: true,
            running_process_name: Some("cursor".into()),
            managed_commands: Vec::new(),
        };
        let entry = probe_connector(
            &AdapterRegistry::new(std::path::PathBuf::from("."), helper.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Cursor),
            &helper,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::NotFound);
        let process = entry
            .probes
            .iter()
            .find(|probe| probe.axis == HealthProbeAxis::Process)
            .expect("process probe");
        assert_eq!(process.outcome, HealthProbeOutcome::Ok);
        assert!(
            process
                .detail
                .as_ref()
                .is_some_and(|detail| detail.contains("cursor"))
        );
    }

    #[test]
    fn maps_codex_traffic_to_connected_when_hooks_managed() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let helper = dir.path().join("llm-notch-hook.exe");
        std::fs::write(&helper, b"helper").expect("write helper");
        let command = format!(
            r#""{}" hook --source codex --vendor-event SessionStart"#,
            helper.display()
        );
        let detected = DetectedConnector {
            source: AgentSource::Codex,
            scope: ConnectorScope::User,
            display_path: "~/.codex/hooks.json".into(),
            config_present: true,
            managed_entries_present: true,
            executable_present: true,
            executable_path: None,
            process_running: false,
            running_process_name: None,
            managed_commands: vec![command],
        };
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let entry = probe_connector(
            &AdapterRegistry::new(root, helper.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Codex),
            &helper,
            Some(1_000),
            2_000,
        );
        assert_eq!(entry.status, ConnectorUserStatus::Connected);
        let trust = entry
            .probes
            .iter()
            .find(|probe| probe.axis == HealthProbeAxis::Trust)
            .expect("trust probe");
        assert_eq!(trust.outcome, HealthProbeOutcome::Ok);
    }

    #[test]
    fn maps_codex_without_traffic_to_action_needed() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let helper = dir.path().join("llm-notch-hook.exe");
        std::fs::write(&helper, b"helper").expect("write helper");
        let command = format!(
            r#""{}" hook --source codex --vendor-event SessionStart"#,
            helper.display()
        );
        let detected = DetectedConnector {
            source: AgentSource::Codex,
            scope: ConnectorScope::User,
            display_path: "~/.codex/hooks.json".into(),
            config_present: true,
            managed_entries_present: true,
            executable_present: true,
            executable_path: None,
            process_running: false,
            running_process_name: None,
            managed_commands: vec![command],
        };
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let entry = probe_connector(
            &AdapterRegistry::new(root, helper.clone()),
            &detected,
            AdapterCapabilities::template(AgentSource::Codex),
            &helper,
            None,
            0,
        );
        assert_eq!(entry.status, ConnectorUserStatus::ActionNeeded);
    }
}
