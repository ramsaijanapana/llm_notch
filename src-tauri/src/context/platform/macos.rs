//! macOS context activation via NSWorkspace / NSRunningApplication.

use notch_protocol::ContextOpenTier;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;

use crate::context::activate::{ActivationOutcome, bundle_id_for_host};
use crate::context::locator::{ContextLocator, HostKind};

pub fn activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    let host = locator.host();
    match target_tier {
        ContextOpenTier::ExactPane => activate_exact_pane(locator, host),
        ContextOpenTier::WindowFocus | ContextOpenTier::AppActivate => {
            activate_application(host, target_tier)
        }
        ContextOpenTier::None => ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some("Context navigation tier is none.".into()),
        },
    }
}

fn activate_application(host: HostKind, target_tier: ContextOpenTier) -> ActivationOutcome {
    let Some(bundle_id) = bundle_id_for_host(host) else {
        return ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(format!("No bundle identifier for {host:?}.")),
        };
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(
                "macOS activation must run on the main thread; try again from the dashboard."
                    .into(),
            ),
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
            host_detail(host, target_tier)
        } else {
            Some(format!(
                "Could not activate {host:?}; open the agent application manually."
            ))
        },
    }
}

fn activate_exact_pane(locator: &ContextLocator, host: HostKind) -> ActivationOutcome {
    if !locator.pane_hint().is_some() {
        return downgrade_to_window_focus(host);
    }
    match host {
        HostKind::TerminalApp | HostKind::ITerm2 => {
            let app_outcome = activate_application(host, ContextOpenTier::WindowFocus);
            if !app_outcome.activated {
                return app_outcome;
            }
            // Best-effort: activation succeeded; exact tab/pane selection is not verified here.
            ActivationOutcome {
                achieved_tier: ContextOpenTier::WindowFocus,
                activated: true,
                detail: Some(
                    "Terminal/iTerm2 window activated; exact pane selection is best-effort only."
                        .into(),
                ),
            }
        }
        _ => activate_application(host, ContextOpenTier::AppActivate),
    }
}

fn downgrade_to_window_focus(host: HostKind) -> ActivationOutcome {
    let outcome = activate_application(host, ContextOpenTier::WindowFocus);
    ActivationOutcome {
        detail: Some(
            "Exact pane requested without a verified pane hint; focused the host window instead."
                .into(),
        ),
        ..outcome
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
    use crate::context::locator::ContextLocator;

    #[test]
    fn macos_bundle_ids_are_available_for_targets() {
        assert!(bundle_id_for_host(HostKind::ITerm2).is_some());
        assert!(bundle_id_for_host(HostKind::TerminalApp).is_some());
    }

    #[test]
    fn exact_pane_without_hint_downgrades_honestly() {
        let locator = ContextLocator::encode(HostKind::TerminalApp, None, None).expect("encode");
        let outcome = activate_exact_pane(&locator, HostKind::TerminalApp);
        assert_ne!(outcome.achieved_tier, ContextOpenTier::ExactPane);
        assert!(outcome.detail.unwrap_or_default().contains("pane"));
    }
}
