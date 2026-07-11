//! User-initiated context navigation for agent sessions.

mod activate;
pub mod locator;
mod platform;
mod resolve;
mod tier;

pub use locator::{ContextLocator, HostKind, LocatorError};
pub use resolve::ResolvedContext;
pub use tier::{achievable_tier, cap_tier, fallback_message};

use notch_protocol::{AdapterCapabilities, AgentSession, ContextOpenTier};

use crate::context::activate::activate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenContextResult {
    pub context_open_tier: ContextOpenTier,
    pub activated: bool,
    pub message: Option<String>,
}

pub fn open_session_context(
    session: &AgentSession,
    adapter: Option<&AdapterCapabilities>,
) -> OpenContextResult {
    let adapter_cap = adapter
        .map(|caps| caps.response_paths().context_open_tier)
        .unwrap_or(ContextOpenTier::None);

    if adapter_cap == ContextOpenTier::None && session.process_root.is_none() {
        return OpenContextResult {
            context_open_tier: ContextOpenTier::None,
            activated: false,
            message: Some(
                "This adapter does not advertise context open and no process is attributed."
                    .into(),
            ),
        };
    }

    let Some(resolved) = (match resolve::resolve_session(session) {
        Ok(value) => value,
        Err(error) => {
            return OpenContextResult {
                context_open_tier: ContextOpenTier::None,
                activated: false,
                message: Some(format!("Invalid context locator: {error}")),
            };
        }
    }) else {
        let fallback_host = resolve::infer_host_from_source(session.source);
        let tier = cap_tier(adapter_cap, fallback_host, false);
        if tier == ContextOpenTier::None {
            return OpenContextResult {
                context_open_tier: ContextOpenTier::None,
                activated: false,
                message: Some(
                    "No live process is attributed to this session; open the agent manually."
                        .into(),
                ),
            };
        }
        let locator = match ContextLocator::encode(fallback_host, None, None) {
            Ok(value) => value,
            Err(error) => {
                return OpenContextResult {
                    context_open_tier: ContextOpenTier::None,
                    activated: false,
                    message: Some(format!("Could not build context locator: {error}")),
                };
            }
        };
        let outcome = activate(&locator, tier);
        return OpenContextResult {
            context_open_tier: outcome.achieved_tier,
            activated: outcome.activated,
            message: outcome.detail.or_else(|| {
                fallback_message(tier, outcome.achieved_tier, fallback_host)
            }),
        };
    };

    let target_tier = cap_tier(adapter_cap, resolved.host, resolved.pane_verified);
    if target_tier == ContextOpenTier::None {
        return OpenContextResult {
            context_open_tier: ContextOpenTier::None,
            activated: false,
            message: Some("Context open is not supported for this adapter.".into()),
        };
    }

    let outcome = activate(&resolved.locator, target_tier);
    OpenContextResult {
        context_open_tier: outcome.achieved_tier,
        activated: outcome.activated,
        message: outcome
            .detail
            .or_else(|| fallback_message(target_tier, outcome.achieved_tier, resolved.host)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{AttentionKind, SessionStatus};

    fn session_with_root(source: notch_protocol::AgentSource) -> AgentSession {
        AgentSession {
            id: "sess-ctx".into(),
            source,
            external_session_id: "ext".into(),
            label: "ctx".into(),
            workspace_label: Some("proj".into()),
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 1,
            last_event_at_ms: 2,
            ended_at_ms: None,
            process_root: Some(notch_protocol::ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 1_700_000_000_000,
            }),
            latest_metric: None,
        }
    }

    #[test]
    fn open_result_includes_tier_for_attributed_session() {
        let session = session_with_root(notch_protocol::AgentSource::Cursor);
        let adapter = AdapterCapabilities::template(notch_protocol::AgentSource::Cursor);
        let result = open_session_context(&session, Some(&adapter));
        assert_eq!(result.context_open_tier, ContextOpenTier::None);
        assert!(!result.activated);
    }

    #[test]
    fn adapter_with_context_open_uses_cap() {
        let mut adapter = AdapterCapabilities::template(notch_protocol::AgentSource::Cursor);
        adapter.context_open = true;
        adapter.context_open_tier = ContextOpenTier::AppActivate;
        let session = session_with_root(notch_protocol::AgentSource::Cursor);
        let result = open_session_context(&session, Some(&adapter));
        assert!(result.context_open_tier != ContextOpenTier::None || result.message.is_some());
    }
}
