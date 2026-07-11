//! Capability declarations for generic protocol clients.

use notch_protocol::{
    AdapterCapabilities, AgentSource, AttentionCapability, AttributionQuality, ContextOpenTier,
};

/// Optional capability flags declared by third-party generic clients.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericClientCapabilities {
    /// Client understands ingest ACK semantics after host persistence.
    pub supports_response_ack: bool,
    /// Client emits explicit attention events.
    pub emits_attention: bool,
    /// Client provides validated `(pid, processStartedAtMs)` pairs.
    pub declares_process_root: bool,
}

/// Default shipped generic template capabilities (no ACK).
pub fn capabilities() -> AdapterCapabilities {
    AdapterCapabilities::template(AgentSource::Generic)
}

/// Capabilities when a client opts into documented ingest ACK handling.
pub fn capabilities_with_ack(client: &GenericClientCapabilities) -> AdapterCapabilities {
    let mut caps = capabilities();
    if client.emits_attention {
        caps.attention = AttentionCapability::Full;
    }
    if client.declares_process_root {
        caps.process_attribution = AttributionQuality::Exact;
    }
    if client.supports_response_ack {
        // ACK support is transport-level only; decision response remains false in V1.
        caps.observe_lifecycle = true;
        caps.observe_tools = true;
    }
    caps.context_open_tier = ContextOpenTier::None;
    caps.decision_response = false;
    caps.respond_decisions = false;
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_generic_capabilities_match_matrix() {
        let caps = capabilities();
        assert_eq!(caps.source, AgentSource::Generic);
        assert!(caps.events);
        assert_eq!(caps.attention, AttentionCapability::Full);
        assert!(!caps.decision_response);
    }

    #[test]
    fn ack_declaration_does_not_enable_decision_response() {
        let caps = capabilities_with_ack(&GenericClientCapabilities {
            supports_response_ack: true,
            ..Default::default()
        });
        assert!(!caps.decision_response);
        assert!(!caps.respond_decisions);
    }

    #[test]
    fn process_root_declaration_sets_exact_attribution() {
        let caps = capabilities_with_ack(&GenericClientCapabilities {
            declares_process_root: true,
            ..Default::default()
        });
        assert_eq!(caps.process_attribution, AttributionQuality::Exact);
    }
}
