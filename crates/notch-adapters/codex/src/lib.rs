//! Codex lifecycle hooks adapter for llm_notch connector and hook pipelines.
//!
//! Observation-only V1: templates fail open, never broker permission decisions.
//! Codex `/hooks` trust is a guided external step — llm_notch never automates it.

mod capabilities;
mod health;
mod merge;
mod normalize;
mod response;
mod template;
mod version;

pub use capabilities::capabilities;
pub use health::{CodexHealthHints, CodexInstallMode, external_trust_actions, health_probe_hints};
pub use merge::{
    ManagedHookEntry, MergeScope, codex_managed_entries, entry_fingerprint, is_managed_command,
    merge_hooks_json,
};
pub use normalize::{
    CodexNormalizeError, NormalizedCodexEvent, normalize_event, redact_vendor_json,
};
pub use response::{
    CodexPermissionBehavior, CodexRespondableHook, build_permission_response, hook_response,
};
pub use template::{
    HELPER_PATH_PLACEHOLDER, WRAPPER_PATH_PLACEHOLDER, inline_hooks_toml_snippet,
    render_hook_command, template_hooks_json,
};
pub use version::{CodexVersionProfile, detect_version, probe_features_flag};
