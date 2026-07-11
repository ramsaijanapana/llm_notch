//! Codex connector health hints and external trust guidance.

use notch_protocol::connector::{ExternalTrustAction, ExternalTrustActionKind};
use notch_protocol::health::{
    ConnectorUserStatus, HealthProbeAxis, HealthProbeFailureKind, HealthProbeOutcome,
    HealthProbeResult,
};

use crate::version::CodexVersionProfile;

/// How Codex integration was installed or detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexInstallMode {
    LifecycleHooks,
    NotifyFallback,
    Unknown,
}

/// Non-secret hints for connector health evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHealthHints {
    pub mode: CodexInstallMode,
    pub hooks_file_present: bool,
    pub hooks_trusted: bool,
    pub helper_available: bool,
    pub saw_traffic: bool,
}

/// External trust steps the user must complete in Codex `/hooks`.
///
/// llm_notch never automates hook trust — this returns instructions only.
pub fn external_trust_actions() -> Vec<ExternalTrustAction> {
    vec![ExternalTrustAction {
        kind: ExternalTrustActionKind::CodexHooksReview,
        instructions: "Open the Codex CLI, run `/hooks`, review each llm_notch hook definition, and trust it. Untrusted hooks are skipped until you complete this step.".into(),
    }]
}

/// Derive orthogonal probe hints from install/traffic state.
pub fn health_probe_hints(hints: &CodexHealthHints) -> Vec<HealthProbeResult> {
    let mut probes = Vec::new();

    probes.push(HealthProbeResult {
        axis: HealthProbeAxis::Installation,
        outcome: if hints.hooks_file_present || hints.mode == CodexInstallMode::NotifyFallback {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Fail
        },
        failure_kind: if hints.hooks_file_present {
            None
        } else {
            Some(HealthProbeFailureKind::NotInstalled)
        },
        detail: None,
    });

    probes.push(HealthProbeResult {
        axis: HealthProbeAxis::Trust,
        outcome: if hints.mode != CodexInstallMode::LifecycleHooks || hints.hooks_trusted {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Warn
        },
        failure_kind: if hints.hooks_trusted || hints.mode != CodexInstallMode::LifecycleHooks {
            None
        } else {
            Some(HealthProbeFailureKind::TrustRequired)
        },
        detail: Some("Run `/hooks` in Codex and trust llm_notch hook definitions.".into()),
    });

    probes.push(HealthProbeResult {
        axis: HealthProbeAxis::Helper,
        outcome: if hints.helper_available {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Fail
        },
        failure_kind: if hints.helper_available {
            None
        } else {
            Some(HealthProbeFailureKind::HelperUnavailable)
        },
        detail: None,
    });

    probes.push(HealthProbeResult {
        axis: HealthProbeAxis::Traffic,
        outcome: if hints.saw_traffic {
            HealthProbeOutcome::Ok
        } else {
            HealthProbeOutcome::Warn
        },
        failure_kind: if hints.saw_traffic {
            None
        } else {
            Some(HealthProbeFailureKind::NoTraffic)
        },
        detail: None,
    });

    probes
}

impl From<&CodexVersionProfile> for CodexInstallMode {
    fn from(profile: &CodexVersionProfile) -> Self {
        match profile {
            CodexVersionProfile::LifecycleHooks { .. } => CodexInstallMode::LifecycleHooks,
            CodexVersionProfile::NotifyFallback => CodexInstallMode::NotifyFallback,
            CodexVersionProfile::Unknown { .. } => CodexInstallMode::Unknown,
        }
    }
}

/// Map hints to the user-facing connector status used by detection.
#[allow(dead_code)] // consumed by connector/detection lane
pub fn user_status_from_hints(hints: &CodexHealthHints) -> ConnectorUserStatus {
    if !hints.hooks_file_present && hints.mode == CodexInstallMode::LifecycleHooks {
        return ConnectorUserStatus::NotInstalled;
    }
    if hints.mode == CodexInstallMode::LifecycleHooks && !hints.hooks_trusted {
        return ConnectorUserStatus::ActionNeeded;
    }
    if !hints.helper_available {
        return ConnectorUserStatus::Error;
    }
    if hints.saw_traffic {
        ConnectorUserStatus::Connected
    } else {
        ConnectorUserStatus::WaitingFirstEvent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_trust_never_automates_review() {
        let actions = external_trust_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, ExternalTrustActionKind::CodexHooksReview);
        assert!(actions[0].instructions.contains("/hooks"));
    }

    #[test]
    fn untrusted_hooks_surface_action_needed() {
        let hints = CodexHealthHints {
            mode: CodexInstallMode::LifecycleHooks,
            hooks_file_present: true,
            hooks_trusted: false,
            helper_available: true,
            saw_traffic: false,
        };
        assert_eq!(
            user_status_from_hints(&hints),
            ConnectorUserStatus::ActionNeeded
        );
    }
}
