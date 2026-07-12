//! Maps `ContextLocator` values into notch-platform activation inputs.

use notch_platform::{
    HostActivationBridge, NavigationDisposition, NavigationOutcome, NavigationTier,
    ProcessDescriptor, TerminalHost, TerminalLocator, TerminalNavigator, VerifiedTerminalMetadata,
    current_activation_bridge, current_navigator, hwnd_for_pid,
};
use notch_protocol::ContextOpenTier;

use crate::context::activate::ActivationOutcome;
use crate::context::locator::{ContextLocator, HostKind};

/// Runs the platform activation bridge for a context locator.
pub fn activate_via_platform_bridge(locator: &ContextLocator) -> NavigationOutcome {
    let terminal_locator = terminal_locator_for_activation(locator);
    current_activation_bridge().activate(&terminal_locator)
}

/// Builds the terminal locator used by the host activation bridge.
pub fn terminal_locator_for_activation(locator: &ContextLocator) -> TerminalLocator {
    let mut descriptor = process_descriptor_from_locator(locator);
    enrich_descriptor(&mut descriptor, locator);
    current_navigator().discover(&descriptor)
}

pub fn map_navigation_outcome(outcome: NavigationOutcome) -> ActivationOutcome {
    ActivationOutcome {
        achieved_tier: map_navigation_tier(outcome.tier),
        activated: outcome.disposition == NavigationDisposition::Activated,
        detail: (!outcome.message.is_empty()).then_some(outcome.message),
    }
}

fn process_descriptor_from_locator(locator: &ContextLocator) -> ProcessDescriptor {
    let process = locator.process();
    ProcessDescriptor {
        process_id: process.as_ref().map(|identity| identity.pid).unwrap_or(0),
        process_started_at_ms: process
            .as_ref()
            .and_then(|identity| u64::try_from(identity.started_at_ms).ok()),
        executable: String::new(),
        parent_executable: None,
        terminal_executable: host_kind_to_terminal_executable(locator.host()),
        metadata: metadata_from_locator(locator),
    }
}

fn metadata_from_locator(locator: &ContextLocator) -> VerifiedTerminalMetadata {
    let terminal = locator.verified_terminal();
    VerifiedTerminalMetadata {
        application_id: host_kind_to_application_id(locator.host()),
        window_handle: terminal.window_handle,
        terminal_session_id: terminal.terminal_session_id,
        tab_id: terminal.tab_id,
        pane_id: terminal.pane_id,
        ..Default::default()
    }
}

fn enrich_descriptor(descriptor: &mut ProcessDescriptor, locator: &ContextLocator) {
    #[cfg(target_os = "windows")]
    {
        if descriptor.metadata.window_handle.is_none() {
            if let Some(pid) = locator.process().map(|identity| identity.pid) {
                if let Some(raw_handle) = hwnd_for_pid(pid) {
                    descriptor.metadata.window_handle = Some(raw_handle);
                }
            }
        }
    }
}

fn host_kind_to_terminal_executable(host: HostKind) -> Option<String> {
    Some(
        match host {
            HostKind::WindowsTerminal => "WindowsTerminal.exe",
            HostKind::VsCode => "Code.exe",
            HostKind::Cursor => "Cursor.exe",
            HostKind::TerminalApp => "Terminal.app",
            HostKind::ITerm2 => "iTerm2.app",
            HostKind::UnknownHost => "unknown",
        }
        .into(),
    )
}

fn host_kind_to_application_id(host: HostKind) -> Option<String> {
    host_kind_to_terminal_executable(host)
}

pub fn host_kind_to_terminal_host(host: HostKind) -> TerminalHost {
    match host {
        HostKind::WindowsTerminal => TerminalHost::WindowsTerminal,
        HostKind::VsCode => TerminalHost::VsCode,
        HostKind::Cursor => TerminalHost::Cursor,
        HostKind::TerminalApp => TerminalHost::MacTerminal,
        HostKind::ITerm2 => TerminalHost::ITerm2,
        HostKind::UnknownHost => TerminalHost::Unknown,
    }
}

pub fn map_navigation_tier(tier: NavigationTier) -> ContextOpenTier {
    match tier {
        NavigationTier::Unsupported => ContextOpenTier::None,
        NavigationTier::AppActivate => ContextOpenTier::AppActivate,
        NavigationTier::WindowFocus => ContextOpenTier::WindowFocus,
        NavigationTier::ExactPane => ContextOpenTier::ExactPane,
    }
}

pub fn map_context_tier(tier: ContextOpenTier) -> NavigationTier {
    match tier {
        ContextOpenTier::None => NavigationTier::Unsupported,
        ContextOpenTier::AppActivate => NavigationTier::AppActivate,
        ContextOpenTier::WindowFocus => NavigationTier::WindowFocus,
        ContextOpenTier::ExactPane => NavigationTier::ExactPane,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_platform::NavigationDisposition;
    use notch_protocol::{ProcessIdentity, VerifiedTerminalContext};

    #[test]
    fn maps_navigation_tiers_without_inflation() {
        assert_eq!(
            map_navigation_tier(NavigationTier::ExactPane),
            ContextOpenTier::ExactPane
        );
        assert_eq!(
            map_navigation_tier(NavigationTier::Unsupported),
            ContextOpenTier::None
        );
    }

    #[test]
    fn maps_host_kinds_to_terminal_hosts() {
        assert_eq!(
            host_kind_to_terminal_host(HostKind::WindowsTerminal),
            TerminalHost::WindowsTerminal
        );
        assert_eq!(
            host_kind_to_terminal_host(HostKind::Cursor),
            TerminalHost::Cursor
        );
    }

    #[test]
    #[cfg(windows)]
    fn terminal_locator_uses_verified_process_identity() {
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            }),
            None,
            None,
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_eq!(terminal.process_id(), 4242);
        assert_eq!(terminal.host(), &TerminalHost::WindowsTerminal);
        assert_ne!(terminal.tier(), NavigationTier::ExactPane);
    }

    #[test]
    #[cfg(windows)]
    fn locator_verified_metadata_enables_exact_pane_activation_mapping() {
        let verified = VerifiedTerminalContext {
            terminal_session_id: Some("0".into()),
            tab_id: Some("1".into()),
            pane_id: Some("0".into()),
            window_handle: None,
        };
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            }),
            None,
            Some(&verified),
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_eq!(terminal.tier(), NavigationTier::ExactPane);
        assert_eq!(
            terminal.verified_metadata().terminal_session_id.as_deref(),
            Some("0")
        );
        assert_eq!(terminal.verified_metadata().tab_id.as_deref(), Some("1"));
        assert_eq!(terminal.verified_metadata().pane_id.as_deref(), Some("0"));
    }

    #[test]
    fn activated_navigation_outcome_maps_to_activation_outcome() {
        let mapped = map_navigation_outcome(NavigationOutcome {
            tier: NavigationTier::WindowFocus,
            disposition: NavigationDisposition::Activated,
            message: "Focused the verified host window".into(),
        });
        assert!(mapped.activated);
        assert_eq!(mapped.achieved_tier, ContextOpenTier::WindowFocus);
        assert_eq!(
            mapped.detail.as_deref(),
            Some("Focused the verified host window")
        );
    }

    #[test]
    fn failed_navigation_outcome_maps_honestly() {
        let mapped = map_navigation_outcome(NavigationOutcome {
            tier: NavigationTier::AppActivate,
            disposition: NavigationDisposition::ActivationFailed,
            message: "missing handle".into(),
        });
        assert!(!mapped.activated);
        assert_eq!(mapped.achieved_tier, ContextOpenTier::AppActivate);
    }

    #[test]
    #[cfg(windows)]
    fn verified_wt_metadata_enables_exact_pane_discovery() {
        use notch_platform::{ProcessDescriptor, VerifiedTerminalMetadata, current_navigator};

        let descriptor = ProcessDescriptor {
            process_id: 42,
            process_started_at_ms: Some(100),
            executable: "agent.exe".into(),
            parent_executable: None,
            terminal_executable: Some("WindowsTerminal.exe".into()),
            metadata: VerifiedTerminalMetadata {
                terminal_session_id: Some("0".into()),
                tab_id: Some("1".into()),
                pane_id: Some("0".into()),
                ..Default::default()
            },
        };
        let terminal = current_navigator().discover(&descriptor);
        assert_eq!(terminal.host(), &TerminalHost::WindowsTerminal);
        assert_eq!(terminal.tier(), NavigationTier::ExactPane);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn mac_terminal_locator_without_verified_metadata_stays_below_exact_pane() {
        let locator = ContextLocator::encode(
            HostKind::TerminalApp,
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            }),
            None,
            None,
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_eq!(terminal.host(), &TerminalHost::MacTerminal);
        assert_ne!(terminal.tier(), NavigationTier::ExactPane);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn mac_iterm_locator_verified_metadata_enables_exact_pane_mapping() {
        let verified = VerifiedTerminalContext {
            tab_id: Some("2".into()),
            pane_id: Some("1".into()),
            ..Default::default()
        };
        let locator = ContextLocator::encode(
            HostKind::ITerm2,
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            }),
            None,
            Some(&verified),
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_eq!(terminal.host(), &TerminalHost::ITerm2);
        assert_eq!(terminal.tier(), NavigationTier::ExactPane);
        assert_eq!(terminal.verified_metadata().tab_id.as_deref(), Some("2"));
        assert_eq!(terminal.verified_metadata().pane_id.as_deref(), Some("1"));
    }
}
