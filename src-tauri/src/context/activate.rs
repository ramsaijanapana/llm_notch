//! Platform-specific context activation entry points.

use notch_protocol::ContextOpenTier;

use crate::context::locator::{ContextLocator, HostKind};
use crate::context::platform;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationOutcome {
    pub achieved_tier: ContextOpenTier,
    pub activated: bool,
    pub detail: Option<String>,
}

pub fn activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    match target_tier {
        ContextOpenTier::None => ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some("No context-open tier requested.".into()),
        },
        tier => platform_activate(locator, tier),
    }
}

fn platform_activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    #[cfg(target_os = "macos")]
    {
        return platform::macos::activate(locator, target_tier);
    }
    #[cfg(target_os = "windows")]
    {
        return platform::windows::activate(locator, target_tier);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (locator, target_tier);
        platform::stub::unsupported_platform()
    }
}

pub fn bundle_id_for_host(host: HostKind) -> Option<&'static str> {
    match host {
        HostKind::TerminalApp => Some("com.apple.Terminal"),
        HostKind::ITerm2 => Some("com.googlecode.iterm2"),
        HostKind::VsCode => Some("com.microsoft.VSCode"),
        HostKind::Cursor => Some("com.todesktop.230313mzl4w4u92"),
        HostKind::WindowsTerminal | HostKind::UnknownHost => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::locator::ContextLocator;
    use notch_protocol::ProcessIdentity;

    #[test]
    fn activation_stub_reports_honest_none_on_unknown_platform() {
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let locator = ContextLocator::encode(HostKind::Cursor, None, None, None).expect("encode");
            let outcome = activate(&locator, ContextOpenTier::AppActivate);
            assert!(!outcome.activated);
            assert_eq!(outcome.achieved_tier, ContextOpenTier::None);
        }
    }

    #[test]
    fn bundle_ids_are_defined_for_editor_hosts() {
        assert!(bundle_id_for_host(HostKind::Cursor).is_some());
        assert!(bundle_id_for_host(HostKind::VsCode).is_some());
        assert!(bundle_id_for_host(HostKind::TerminalApp).is_some());
    }

    #[test]
    fn windows_terminal_has_no_bundle_id() {
        assert!(bundle_id_for_host(HostKind::WindowsTerminal).is_none());
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn platform_activation_smoke_current_process() {
        let locator = ContextLocator::encode(
            HostKind::Cursor,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 0,
            }),
            None,
            None,
        )
        .expect("encode");
        let outcome = activate(&locator, ContextOpenTier::AppActivate);
        assert!(outcome.activated || outcome.detail.is_some());
    }
}
