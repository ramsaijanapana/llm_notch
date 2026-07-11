//! Adapter capability matrix keyed by detected Claude Code version.

use notch_protocol::{
    AdapterCapabilities, AgentSource, AttentionCapability, AttributionQuality, ContextOpenTier,
};

use crate::version::ClaudeVersionProfile;

/// Returns frozen wire capabilities for the detected Claude Code version profile.
///
/// Unknown versions are observation-only: no verified decision response paths.
pub fn capabilities(profile: &ClaudeVersionProfile) -> AdapterCapabilities {
    let mut caps = AdapterCapabilities::template(AgentSource::ClaudeCode);
    caps.observe_lifecycle = true;
    caps.observe_tools = true;
    caps.events = true;
    caps.attention = AttentionCapability::Partial;
    caps.process_attribution = AttributionQuality::Unknown;
    caps.context_open = false;
    caps.context_open_tier = ContextOpenTier::None;
    caps.fail_open_hooks = true;
    caps.requires_external_trust = false;
    caps.respond_questions = false;

    match profile {
        ClaudeVersionProfile::Known { .. } => {
            caps.respond_decisions = true;
            caps.decision_response = true;
        }
        ClaudeVersionProfile::Unknown { .. } => {
            caps.respond_decisions = false;
            caps.decision_response = false;
        }
    }

    caps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::detect_version;

    #[test]
    fn known_version_enables_verified_decision_paths() {
        let profile = detect_version(Some("2.1.205"));
        let caps = capabilities(&profile);
        assert!(caps.respond_decisions);
        assert!(caps.decision_response);
        assert!(!caps.respond_questions);
        assert_eq!(caps.response_paths().decisions, true);
        assert_eq!(caps.response_paths().questions, false);
    }

    #[test]
    fn unknown_version_is_observation_only() {
        let profile = detect_version(Some("0.9.0"));
        let caps = capabilities(&profile);
        assert!(!caps.respond_decisions);
        assert!(!caps.decision_response);
        assert!(caps.events);
        assert_eq!(caps.attention, AttentionCapability::Partial);
    }

    #[test]
    fn observation_paths_cover_lifecycle_tools_and_attention() {
        let caps = capabilities(&detect_version(Some("2.1.0")));
        let observation = caps.observation_paths();
        assert!(observation.lifecycle_events);
        assert!(observation.tool_events);
        assert!(observation.attention_events);
    }
}
