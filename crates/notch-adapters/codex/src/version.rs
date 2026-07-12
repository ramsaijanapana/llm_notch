//! Codex version and feature-flag probing from hook payloads and config hints.

/// Profile returned by [`detect_version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexVersionProfile {
    /// Lifecycle hooks enabled via canonical `features.hooks`.
    LifecycleHooks { hooks_feature: HooksFeatureFlag },
    /// Legacy `notify` completion callback only — strictly weaker.
    NotifyFallback,
    /// Unrecognized hook surface — observation-only capabilities.
    Unknown { hook_event: Option<String> },
}

/// Which Codex feature flag enables lifecycle hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HooksFeatureFlag {
    /// Current canonical key (`features.hooks`).
    Hooks,
    /// Deprecated alias still accepted by Codex (`features.codex_hooks`).
    CodexHooksDeprecated,
}

/// Probe config text for the hooks feature flag name Codex will honor.
pub fn probe_features_flag(config_toml: &str) -> HooksFeatureFlag {
    let normalized = config_toml.to_ascii_lowercase();
    let has_hooks = normalized
        .lines()
        .any(|line| line.trim_start().starts_with("hooks") && !line.contains("codex_hooks"));
    if has_hooks && !normalized.contains("hooks = false") {
        return HooksFeatureFlag::Hooks;
    }
    if normalized.contains("codex_hooks") && !normalized.contains("codex_hooks = false") {
        return HooksFeatureFlag::CodexHooksDeprecated;
    }
    HooksFeatureFlag::Hooks
}

/// Detect Codex integration mode from vendor event name and optional payload metadata.
///
/// Unknown vendor events downgrade to observation-only via [`crate::capabilities`].
pub fn detect_version(vendor_event: &str, hook_event_name: Option<&str>) -> CodexVersionProfile {
    let normalized = vendor_event
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    if normalized == "notify" {
        return CodexVersionProfile::NotifyFallback;
    }

    let hook_event = hook_event_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if is_lifecycle_hook_event(hook_event.or(Some(vendor_event))) {
        return CodexVersionProfile::LifecycleHooks {
            hooks_feature: HooksFeatureFlag::Hooks,
        };
    }

    CodexVersionProfile::Unknown {
        hook_event: hook_event.map(str::to_string),
    }
}

fn is_lifecycle_hook_event(event: Option<&str>) -> bool {
    let Some(raw) = event else {
        return false;
    };
    matches!(
        raw,
        "SessionStart"
            | "SubagentStart"
            | "PreToolUse"
            | "PermissionRequest"
            | "PostToolUse"
            | "PostToolUseFailure"
            | "UserPromptSubmit"
            | "PreCompact"
            | "PostCompact"
            | "SubagentStop"
            | "Stop"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_vendor_event_is_hooks_profile() {
        let profile = detect_version("SessionStart", Some("SessionStart"));
        assert_eq!(
            profile,
            CodexVersionProfile::LifecycleHooks {
                hooks_feature: HooksFeatureFlag::Hooks,
            }
        );
    }

    #[test]
    fn notify_is_fallback_profile() {
        let profile = detect_version("notify", None);
        assert_eq!(profile, CodexVersionProfile::NotifyFallback);
    }

    #[test]
    fn unknown_event_is_observation_only_profile() {
        let profile = detect_version("FutureHook", Some("FutureHook"));
        assert!(matches!(profile, CodexVersionProfile::Unknown { .. }));
    }

    #[test]
    fn prefers_canonical_hooks_feature_flag() {
        let flag = probe_features_flag("[features]\nhooks = true\n");
        assert_eq!(flag, HooksFeatureFlag::Hooks);
    }

    #[test]
    fn detects_deprecated_codex_hooks_alias() {
        let flag = probe_features_flag("[features]\ncodex_hooks = true\n");
        assert_eq!(flag, HooksFeatureFlag::CodexHooksDeprecated);
    }
}
