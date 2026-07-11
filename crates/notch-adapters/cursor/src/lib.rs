//! Cursor hooks adapter for llm_notch connector and hook pipelines.
//!
//! Observation-only V1: templates fail open, never broker permission decisions.

mod capabilities;
mod health;
mod merge;
mod normalize;
mod response;
mod template;
mod version;

pub use capabilities::capabilities;
pub use health::{
    CursorHealthHints, CursorInstallState, classify_hooks_commands, health_probe_hints,
    managed_entry_present,
};
pub use merge::{
    ManagedHookEntry, MergeScope, cursor_managed_entries, entry_fingerprint, is_managed_command,
    merge_hooks_json,
};
pub use normalize::{CursorNormalizeError, NormalizedCursorEvent, normalize_event};
pub use response::{
    CursorPermissionDecision, CursorRespondableHook, build_permission_response, hook_response,
};
pub use template::{
    HELPER_PATH_PLACEHOLDER, WRAPPER_PATH_PLACEHOLDER, render_hook_command,
    render_wrapper_command, template_hooks_json,
};
pub use version::{CursorVersionProfile, detect_version};
