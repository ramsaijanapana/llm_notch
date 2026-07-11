//! Claude Code hooks adapter for llm_notch connector and hook pipelines.
//!
//! Shipped templates fail open (`{}` stdout, exit 0). Decision responses are built
//! only for vendor-verified paths: `PermissionRequest` allow/deny and `ExitPlanMode`
//! approval via `PreToolUse`. There is no generic question-answer response path.

mod capabilities;
mod health;
mod merge;
mod normalize;
mod response;
mod template;
mod version;

pub use capabilities::capabilities;
pub use health::{
    ClaudeHealthHints, ClaudeInstallState, health_probe_hints, managed_entry_present,
};
pub use merge::{
    ManagedHookEntry, MergeScope, claude_managed_entries, entry_fingerprint, is_managed_command,
    merge_settings_hooks,
};
pub use normalize::{ClaudeNormalizeError, NormalizedClaudeEvent, normalize_event};
pub use response::{
    ClaudePermissionDecision, ClaudeRespondableHook, build_decision_response,
    build_exit_plan_approve_response, build_permission_response, hook_response,
};
pub use template::{
    HELPER_PATH_PLACEHOLDER, WRAPPER_PATH_PLACEHOLDER, render_hook_command, template_settings_hooks,
};
pub use version::{ClaudeVersionProfile, detect_version};
