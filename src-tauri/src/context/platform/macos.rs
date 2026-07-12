//! macOS context activation via notch-platform host bridges with NSWorkspace fallback.

use notch_platform::NavigationDisposition;
use notch_protocol::ContextOpenTier;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplicationActivationOptions, NSWorkspace};
use objc2_foundation::NSString;

use crate::context::activate::{ActivationOutcome, bundle_id_for_host};
use crate::context::locator::{ContextLocator, HostKind};
use crate::context::platform::bridge::{activate_via_platform_bridge, map_navigation_outcome};

pub fn activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    let bridge_outcome = activate_via_platform_bridge(locator);
    if bridge_outcome.disposition == NavigationDisposition::Activated {
        return map_navigation_outcome(bridge_outcome);
    }

    let bridge_detail = (!bridge_outcome.message.is_empty()).then_some(bridge_outcome.message);
    legacy_activate(locator, target_tier, bridge_detail)
}

fn legacy_activate(
    locator: &ContextLocator,
    target_tier: ContextOpenTier,
    bridge_detail: Option<String>,
) -> ActivationOutcome {
    let host = locator.host();

    match target_tier {
        ContextOpenTier::ExactPane => {
            if !has_verified_pane_route(locator) {
                let outcome =
                    activate_application(host, ContextOpenTier::WindowFocus, bridge_detail.clone());
                return ActivationOutcome {
                    detail: Some(merge_detail(
                        bridge_detail,
                        "Exact pane requested without verified pane metadata; focused the host application instead.",
                    )),
                    ..outcome
                };
            }
            let outcome =
                activate_application(host, ContextOpenTier::WindowFocus, bridge_detail.clone());
            ActivationOutcome {
                achieved_tier: ContextOpenTier::WindowFocus,
                activated: outcome.activated,
                detail: Some(merge_detail(
                    bridge_detail,
                    "Exact pane selection could not be verified; focused the host application instead.",
                )),
            }
        }
        ContextOpenTier::WindowFocus | ContextOpenTier::AppActivate => {
            activate_application(host, target_tier, bridge_detail)
        }
        ContextOpenTier::None => ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(merge_detail(
                bridge_detail,
                "Context navigation tier is none.",
            )),
        },
    }
}

fn has_verified_pane_route(locator: &ContextLocator) -> bool {
    let terminal = locator.verified_terminal();
    terminal.pane_id.is_some()
        && (terminal.tab_id.is_some() || terminal.terminal_session_id.is_some())
}

fn activate_application(
    host: HostKind,
    target_tier: ContextOpenTier,
    bridge_detail: Option<String>,
) -> ActivationOutcome {
    let Some(bundle_id) = bundle_id_for_host(host) else {
        return ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(merge_detail(
                bridge_detail,
                format!("No bundle identifier for {host:?}."),
            )),
        };
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(merge_detail(
                bridge_detail,
                "macOS activation must run on the main thread; try again from the dashboard.",
            )),
        };
    };
    let workspace = NSWorkspace::sharedWorkspace(mtm);
    let bundle = NSString::from_str(bundle_id);
    let activated = workspace
        .openApplicationWithBundleIdentifier_options_configuration_error(
            &bundle,
            NSApplicationActivationOptions::ActivateIgnoringOtherApps,
            None,
        )
        .is_ok();
    ActivationOutcome {
        achieved_tier: if activated {
            target_tier
        } else {
            ContextOpenTier::None
        },
        activated,
        detail: if activated {
            bridge_detail.or_else(|| host_detail(host, target_tier))
        } else {
            Some(merge_detail(
                bridge_detail,
                format!("Could not activate {host:?}; open the agent application manually."),
            ))
        },
    }
}

fn merge_detail(bridge_detail: Option<String>, fallback: impl Into<String>) -> String {
    match bridge_detail {
        Some(detail) if !detail.is_empty() => detail,
        _ => fallback.into(),
    }
}

fn host_detail(host: HostKind, tier: ContextOpenTier) -> Option<String> {
    match (host, tier) {
        (HostKind::TerminalApp | HostKind::ITerm2, ContextOpenTier::WindowFocus) => {
            Some("Activated terminal application (tab/pane selection is best-effort).".into())
        }
        (HostKind::VsCode | HostKind::Cursor, ContextOpenTier::AppActivate) => {
            Some("Activated editor application.".into())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::platform::bridge::terminal_locator_for_activation;
    use notch_platform::{NavigationTier, try_macos_exact_pane_host_bridge};
    use notch_protocol::{ProcessIdentity, VerifiedTerminalContext};

    #[test]
    fn macos_bundle_ids_are_available_for_targets() {
        assert!(bundle_id_for_host(HostKind::ITerm2).is_some());
        assert!(bundle_id_for_host(HostKind::TerminalApp).is_some());
    }

    #[test]
    fn exact_pane_without_verified_metadata_downgrades_honestly() {
        let locator =
            ContextLocator::encode(HostKind::TerminalApp, None, None, None).expect("encode");
        let outcome = legacy_activate(&locator, ContextOpenTier::ExactPane, None);
        assert_ne!(outcome.achieved_tier, ContextOpenTier::ExactPane);
        assert!(outcome.detail.unwrap_or_default().contains("pane"));
    }

    #[test]
    fn macos_activation_never_claims_exact_pane_without_verified_metadata() {
        let locator = ContextLocator::encode(
            HostKind::TerminalApp,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 0,
            }),
            Some("workspace-label"),
            None,
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_ne!(terminal.tier(), NavigationTier::ExactPane);
        assert_eq!(
            try_macos_exact_pane_host_bridge(&terminal),
            notch_platform::HostBridgeOutcome::NotApplicable
        );

        let outcome = activate(&locator, ContextOpenTier::ExactPane);
        assert_ne!(outcome.achieved_tier, ContextOpenTier::ExactPane);
    }

    #[test]
    fn terminal_exact_pane_with_pane_id_is_honest_about_split_limitations() {
        let verified = VerifiedTerminalContext {
            tab_id: Some("1".into()),
            pane_id: Some("1".into()),
            ..Default::default()
        };
        let locator = ContextLocator::encode(HostKind::TerminalApp, None, None, Some(&verified))
            .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_eq!(terminal.tier(), NavigationTier::ExactPane);
        assert!(matches!(
            try_macos_exact_pane_host_bridge(&terminal),
            notch_platform::HostBridgeOutcome::Unavailable { .. }
        ));
    }
}
