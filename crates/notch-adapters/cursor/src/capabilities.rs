//! Capability matrix for Cursor adapter versions.

use notch_protocol::{
    AdapterCapabilities, AgentSource, AttentionCapability, AttributionQuality, ContextOpenTier,
};

use crate::version::CursorVersionProfile;

/// Returns frozen [`AdapterCapabilities`] for a detected Cursor version profile.
///
/// Unknown versions are observation-only: events and lifecycle/tool observation remain
/// enabled, but no decision response, attention inference, or context-open paths.
pub fn capabilities(profile: &CursorVersionProfile) -> AdapterCapabilities {
    match profile {
        CursorVersionProfile::Known { .. } => known_template(),
        CursorVersionProfile::Unknown { .. } => observation_only(),
    }
}

fn known_template() -> AdapterCapabilities {
    let mut caps = AdapterCapabilities::template(AgentSource::Cursor);
    caps.observe_lifecycle = true;
    caps.observe_tools = true;
    caps.fail_open_hooks = true;
    caps.context_open = true;
    caps.context_open_tier = ContextOpenTier::AppActivate;
    caps
}

fn observation_only() -> AdapterCapabilities {
    AdapterCapabilities {
        source: AgentSource::Cursor,
        events: true,
        attention: AttentionCapability::None,
        decision_response: false,
        context_open: false,
        process_attribution: AttributionQuality::Unknown,
        context_open_tier: ContextOpenTier::None,
        observe_lifecycle: true,
        observe_tools: true,
        respond_decisions: false,
        respond_questions: false,
        fail_open_hooks: true,
        requires_external_trust: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::detect_version;

    #[test]
    fn known_version_matches_capability_matrix() {
        let profile = detect_version(Some("1.7.2"), Some(1));
        let caps = capabilities(&profile);
        assert_eq!(caps.source, AgentSource::Cursor);
        assert!(caps.events);
        assert_eq!(caps.attention, AttentionCapability::None);
        assert!(!caps.decision_response);
        assert!(!caps.respond_decisions);
        assert_eq!(caps.process_attribution, AttributionQuality::Unknown);
        assert!(caps.fail_open_hooks);
    }

    #[test]
    fn unknown_version_is_observation_only() {
        let profile = detect_version(Some("future-99"), Some(1));
        let caps = capabilities(&profile);
        assert!(caps.events);
        assert!(caps.observe_lifecycle);
        assert!(caps.observe_tools);
        assert!(!caps.respond_decisions);
        assert!(!caps.decision_response);
        let paths = caps.response_paths();
        assert!(!paths.decisions);
        assert!(!paths.questions);
    }
}
