//! Capability matrix for Codex adapter modes and version profiles.

use notch_protocol::{
    AdapterCapabilities, AgentSource, AttentionCapability, AttributionQuality, ContextOpenTier,
};

use crate::version::CodexVersionProfile;

/// Returns frozen [`AdapterCapabilities`] for a detected Codex version profile.
///
/// Unknown profiles and legacy notify downgrade to honest, observation-only caps.
pub fn capabilities(profile: &CodexVersionProfile) -> AdapterCapabilities {
    match profile {
        CodexVersionProfile::LifecycleHooks { .. } => lifecycle_hooks_template(),
        CodexVersionProfile::NotifyFallback => notify_fallback_template(),
        CodexVersionProfile::Unknown { .. } => observation_only(),
    }
}

fn lifecycle_hooks_template() -> AdapterCapabilities {
    let mut caps = AdapterCapabilities::template(AgentSource::Codex);
    caps.events = true;
    caps.attention = AttentionCapability::Partial;
    caps.observe_lifecycle = true;
    caps.observe_tools = true;
    caps.requires_external_trust = true;
    caps.fail_open_hooks = true;
    caps
}

fn notify_fallback_template() -> AdapterCapabilities {
    AdapterCapabilities {
        source: AgentSource::Codex,
        events: false,
        attention: AttentionCapability::None,
        decision_response: false,
        context_open: false,
        process_attribution: AttributionQuality::Unknown,
        context_open_tier: ContextOpenTier::None,
        observe_lifecycle: false,
        observe_tools: false,
        respond_decisions: false,
        respond_questions: false,
        fail_open_hooks: true,
        requires_external_trust: false,
    }
}

fn observation_only() -> AdapterCapabilities {
    AdapterCapabilities {
        source: AgentSource::Codex,
        events: true,
        attention: AttentionCapability::None,
        decision_response: false,
        context_open: false,
        process_attribution: AttributionQuality::Unknown,
        context_open_tier: ContextOpenTier::None,
        observe_lifecycle: true,
        observe_tools: false,
        respond_decisions: false,
        respond_questions: false,
        fail_open_hooks: true,
        requires_external_trust: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::{CodexVersionProfile, detect_version};

    #[test]
    fn lifecycle_hooks_match_capability_matrix() {
        let profile = detect_version("PreToolUse", Some("PreToolUse"));
        let caps = capabilities(&profile);
        assert_eq!(caps.source, AgentSource::Codex);
        assert!(caps.events);
        assert_eq!(caps.attention, AttentionCapability::Partial);
        assert!(caps.requires_external_trust);
        assert!(!caps.decision_response);
    }

    #[test]
    fn notify_fallback_is_minimal() {
        let caps = capabilities(&CodexVersionProfile::NotifyFallback);
        assert!(!caps.events);
        assert!(!caps.observe_tools);
        assert!(!caps.requires_external_trust);
    }

    #[test]
    fn unknown_profile_is_observation_only() {
        let profile = CodexVersionProfile::Unknown {
            hook_event: Some("Experimental".into()),
        };
        let caps = capabilities(&profile);
        assert!(caps.events);
        assert!(caps.observe_lifecycle);
        assert!(!caps.observe_tools);
        assert!(!caps.respond_decisions);
    }
}
