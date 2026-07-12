//! Orthogonal connector health probes and user-facing status mapping.
//!
//! UI derives summary status from the first failing probe axis; the full probe
//! vector is expandable diagnostics detail.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Independent health dimensions evaluated by the connector manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum HealthProbeAxis {
    Installation,
    Trust,
    Traffic,
    Helper,
    /// Positive-only process discovery evidence; does not affect user-facing status.
    Process,
}

/// Outcome for a single probe axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum HealthProbeOutcome {
    Ok,
    Warn,
    Fail,
}

/// Optional machine-readable reason when a probe is not OK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum HealthProbeFailureKind {
    AgentNotFound,
    NotInstalled,
    TrustRequired,
    HelperUnavailable,
    HelperPathMissing,
    HooksMisconfigured,
    NoTraffic,
    ConfigDrift,
    InternalError,
}

/// Result of evaluating one orthogonal probe axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct HealthProbeResult {
    pub axis: HealthProbeAxis,
    pub outcome: HealthProbeOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failure_kind: Option<HealthProbeFailureKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

/// Deterministic user-facing connector status derived from probe results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ConnectorUserStatus {
    NotFound,
    NotInstalled,
    ActionNeeded,
    WaitingFirstEvent,
    HelperMissing,
    HooksMisconfigured,
    Connected,
    DriftDetected,
    Error,
}

/// Maps orthogonal probe results to the single user-facing status.
///
/// Priority follows the first failing axis: installation → trust → helper → traffic,
/// with drift and error fallbacks documented in `docs/parity/CONTRACT_FREEZE.md`.
/// The `process` axis is diagnostic-only and is excluded from this mapping.
pub fn map_probes_to_user_status(probes: &[HealthProbeResult]) -> ConnectorUserStatus {
    let actionable: Vec<_> = probes
        .iter()
        .filter(|entry| entry.axis != HealthProbeAxis::Process)
        .collect();
    let probe = |axis: HealthProbeAxis| actionable.iter().find(|entry| entry.axis == axis);

    if let Some(installation) = probe(HealthProbeAxis::Installation) {
        match installation.outcome {
            HealthProbeOutcome::Fail => {
                return match installation.failure_kind {
                    Some(HealthProbeFailureKind::AgentNotFound) => ConnectorUserStatus::NotFound,
                    _ => ConnectorUserStatus::NotInstalled,
                };
            }
            HealthProbeOutcome::Warn => return ConnectorUserStatus::DriftDetected,
            HealthProbeOutcome::Ok => {}
        }
    }

    if let Some(trust) = probe(HealthProbeAxis::Trust) {
        match trust.outcome {
            HealthProbeOutcome::Ok => {}
            HealthProbeOutcome::Warn | HealthProbeOutcome::Fail => {
                return ConnectorUserStatus::ActionNeeded;
            }
        }
    }

    if matches!(
        probe(HealthProbeAxis::Helper).map(|entry| entry.outcome),
        Some(HealthProbeOutcome::Fail)
    ) {
        return match probe(HealthProbeAxis::Helper).and_then(|entry| entry.failure_kind) {
            Some(HealthProbeFailureKind::HelperPathMissing) => ConnectorUserStatus::HelperMissing,
            Some(HealthProbeFailureKind::HooksMisconfigured) => {
                ConnectorUserStatus::HooksMisconfigured
            }
            _ => ConnectorUserStatus::Error,
        };
    }

    if matches!(
        probe(HealthProbeAxis::Traffic).map(|entry| entry.outcome),
        Some(HealthProbeOutcome::Fail | HealthProbeOutcome::Warn)
    ) {
        return ConnectorUserStatus::WaitingFirstEvent;
    }

    if probes
        .iter()
        .filter(|entry| entry.axis != HealthProbeAxis::Process)
        .any(|entry| entry.outcome == HealthProbeOutcome::Warn)
    {
        return ConnectorUserStatus::DriftDetected;
    }

    if actionable
        .iter()
        .all(|entry| entry.outcome == HealthProbeOutcome::Ok)
    {
        return ConnectorUserStatus::Connected;
    }

    ConnectorUserStatus::Error
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(
        axis: HealthProbeAxis,
        outcome: HealthProbeOutcome,
        failure_kind: Option<HealthProbeFailureKind>,
    ) -> HealthProbeResult {
        HealthProbeResult {
            axis,
            outcome,
            failure_kind,
            detail: None,
        }
    }

    #[test]
    fn maps_all_ok_to_connected() {
        let probes = vec![
            probe(HealthProbeAxis::Installation, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Trust, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Traffic, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Helper, HealthProbeOutcome::Ok, None),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::Connected
        );
    }

    #[test]
    fn maps_agent_not_found_before_other_failures() {
        let probes = vec![
            probe(
                HealthProbeAxis::Installation,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::AgentNotFound),
            ),
            probe(HealthProbeAxis::Traffic, HealthProbeOutcome::Fail, None),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::NotFound
        );
    }

    #[test]
    fn maps_trust_failure_to_action_needed() {
        let probes = vec![
            probe(HealthProbeAxis::Installation, HealthProbeOutcome::Ok, None),
            probe(
                HealthProbeAxis::Trust,
                HealthProbeOutcome::Warn,
                Some(HealthProbeFailureKind::TrustRequired),
            ),
            probe(HealthProbeAxis::Traffic, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Helper, HealthProbeOutcome::Ok, None),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::ActionNeeded
        );
    }

    #[test]
    fn maps_no_traffic_to_waiting_first_event() {
        let probes = vec![
            probe(HealthProbeAxis::Installation, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Trust, HealthProbeOutcome::Ok, None),
            probe(
                HealthProbeAxis::Traffic,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::NoTraffic),
            ),
            probe(HealthProbeAxis::Helper, HealthProbeOutcome::Ok, None),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::WaitingFirstEvent
        );
    }

    #[test]
    fn process_probe_does_not_change_user_status() {
        let probes = vec![
            probe(
                HealthProbeAxis::Installation,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::AgentNotFound),
            ),
            probe(HealthProbeAxis::Process, HealthProbeOutcome::Ok, None),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::NotFound
        );
    }

    #[test]
    fn maps_helper_path_missing_before_traffic() {
        let probes = vec![
            probe(HealthProbeAxis::Installation, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Trust, HealthProbeOutcome::Ok, None),
            probe(
                HealthProbeAxis::Helper,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::HelperPathMissing),
            ),
            probe(
                HealthProbeAxis::Traffic,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::NoTraffic),
            ),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::HelperMissing
        );
    }

    #[test]
    fn maps_hooks_misconfigured_before_traffic() {
        let probes = vec![
            probe(HealthProbeAxis::Installation, HealthProbeOutcome::Ok, None),
            probe(HealthProbeAxis::Trust, HealthProbeOutcome::Ok, None),
            probe(
                HealthProbeAxis::Helper,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::HooksMisconfigured),
            ),
            probe(
                HealthProbeAxis::Traffic,
                HealthProbeOutcome::Fail,
                Some(HealthProbeFailureKind::NoTraffic),
            ),
        ];
        assert_eq!(
            map_probes_to_user_status(&probes),
            ConnectorUserStatus::HooksMisconfigured
        );
    }

    #[test]
    fn health_status_round_trips_camel_case() {
        let value = serde_json::to_value(ConnectorUserStatus::WaitingFirstEvent).unwrap();
        assert_eq!(value, "waitingFirstEvent");
        let decoded: ConnectorUserStatus = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, ConnectorUserStatus::WaitingFirstEvent);

        let helper_missing = serde_json::to_value(ConnectorUserStatus::HelperMissing).unwrap();
        assert_eq!(helper_missing, "helperMissing");
        let hooks_misconfigured =
            serde_json::to_value(ConnectorUserStatus::HooksMisconfigured).unwrap();
        assert_eq!(hooks_misconfigured, "hooksMisconfigured");
    }
}
