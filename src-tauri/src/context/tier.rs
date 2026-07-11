//! Context-open tier selection with honest capability capping.

use notch_protocol::ContextOpenTier;

use crate::context::locator::HostKind;

/// Returns the highest tier we can honestly attempt for a resolved host.
pub fn achievable_tier(host: HostKind, pane_verified: bool) -> ContextOpenTier {
    match host {
        HostKind::TerminalApp | HostKind::ITerm2 if pane_verified => ContextOpenTier::ExactPane,
        HostKind::TerminalApp | HostKind::ITerm2 | HostKind::WindowsTerminal => {
            ContextOpenTier::WindowFocus
        }
        HostKind::VsCode | HostKind::Cursor | HostKind::UnknownHost => ContextOpenTier::AppActivate,
    }
}

/// Caps the achievable tier by adapter-advertised support.
pub fn cap_tier(
    adapter_cap: ContextOpenTier,
    host: HostKind,
    pane_verified: bool,
) -> ContextOpenTier {
    let achievable = achievable_tier(host, pane_verified);
    min_tier(adapter_cap, achievable)
}

pub fn fallback_message(
    requested: ContextOpenTier,
    achieved: ContextOpenTier,
    host: HostKind,
) -> Option<String> {
    if achieved == requested {
        return None;
    }
    Some(match (requested, achieved, host) {
        (ContextOpenTier::ExactPane, ContextOpenTier::WindowFocus, _) => {
            "Exact pane selection is not verified for this host; focused the nearest window instead."
                .to_string()
        }
        (ContextOpenTier::ExactPane | ContextOpenTier::WindowFocus, ContextOpenTier::AppActivate, _) => {
            "Window focus is unavailable; activated the host application instead.".to_string()
        }
        (_, ContextOpenTier::None, _) => {
            "Context navigation is unavailable for this session.".to_string()
        }
        _ => format!(
            "Requested {requested:?} navigation; achieved {achieved:?} for {host:?}."
        ),
    })
}

fn min_tier(left: ContextOpenTier, right: ContextOpenTier) -> ContextOpenTier {
    tier_rank(left)
        .min(tier_rank(right))
        .and_then(rank_tier)
        .unwrap_or(ContextOpenTier::None)
}

fn tier_rank(tier: ContextOpenTier) -> Option<u8> {
    match tier {
        ContextOpenTier::None => Some(0),
        ContextOpenTier::AppActivate => Some(1),
        ContextOpenTier::WindowFocus => Some(2),
        ContextOpenTier::ExactPane => Some(3),
    }
}

fn rank_tier(rank: u8) -> Option<ContextOpenTier> {
    match rank {
        0 => Some(ContextOpenTier::None),
        1 => Some(ContextOpenTier::AppActivate),
        2 => Some(ContextOpenTier::WindowFocus),
        3 => Some(ContextOpenTier::ExactPane),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_without_verification_caps_at_window_focus() {
        assert_eq!(
            achievable_tier(HostKind::TerminalApp, false),
            ContextOpenTier::WindowFocus
        );
        assert_eq!(
            achievable_tier(HostKind::ITerm2, true),
            ContextOpenTier::ExactPane
        );
    }

    #[test]
    fn editor_hosts_default_to_app_activate() {
        assert_eq!(
            achievable_tier(HostKind::Cursor, false),
            ContextOpenTier::AppActivate
        );
        assert_eq!(
            achievable_tier(HostKind::VsCode, false),
            ContextOpenTier::AppActivate
        );
    }

    #[test]
    fn adapter_cap_limits_achievable_tier() {
        assert_eq!(
            cap_tier(
                ContextOpenTier::AppActivate,
                HostKind::WindowsTerminal,
                false
            ),
            ContextOpenTier::AppActivate
        );
        assert_eq!(
            cap_tier(ContextOpenTier::None, HostKind::Cursor, false),
            ContextOpenTier::None
        );
        assert_eq!(
            cap_tier(ContextOpenTier::ExactPane, HostKind::TerminalApp, false),
            ContextOpenTier::WindowFocus
        );
    }

    #[test]
    fn fallback_message_when_exact_pane_not_verified() {
        let message = fallback_message(
            ContextOpenTier::ExactPane,
            ContextOpenTier::WindowFocus,
            HostKind::TerminalApp,
        );
        assert!(message.is_some());
        assert!(message.unwrap().contains("Exact pane"));
    }
}
