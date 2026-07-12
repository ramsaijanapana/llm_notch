//! Windows Terminal metadata collector for hook and platform bridges.
//!
//! # What Windows Terminal exposes today
//!
//! Shell integration injects these environment variables into each tab/pane session:
//!
//! | Variable | Discoverable | Used for ExactPane |
//! |----------|--------------|-------------------|
//! | `WT_SESSION` | Yes (GUID per connection) | Stored as `terminal_session_id`; **not** a `wt.exe` window/tab target |
//! | `WT_PROFILE_ID` | Yes (profile GUID) | Informational only |
//! | `WT_PROFILE_NAME` | Yes | Informational only |
//! | Tab index | **No** WT env var | Requires `LLM_NOTCH_TAB_ID` pass-through or explicit user profile config |
//! | Pane index | **No** WT env var | Requires `LLM_NOTCH_PANE_ID` pass-through or explicit user profile config |
//!
//! This module **never** parses window titles or invents numeric indices.
//! When `LLM_NOTCH_WINDOW_HANDLE` is absent, `collect_wt_metadata` may discover a
//! verified HWND via `hwnd_collector` (process-tree walk + `IsWindow` validation).
//! See `integrations/windows-terminal/README.md` for hook wiring.

/// Windows Terminal session GUID (`WT_SESSION`).
pub const ENV_WT_SESSION: &str = "WT_SESSION";
/// Windows Terminal profile GUID (informational).
pub const ENV_WT_PROFILE_ID: &str = "WT_PROFILE_ID";
/// Windows Terminal profile display name (informational).
pub const ENV_WT_PROFILE_NAME: &str = "WT_PROFILE_NAME";
/// Explicit terminal session override (wins over `WT_SESSION`).
pub const ENV_TERMINAL_SESSION_ID: &str = "LLM_NOTCH_TERMINAL_SESSION_ID";
/// Tab index string for `wt.exe focus-tab -t` (collector-supplied only).
pub const ENV_TAB_ID: &str = "LLM_NOTCH_TAB_ID";
/// Pane index string for `wt.exe focus-pane -t` (collector-supplied only).
pub const ENV_PANE_ID: &str = "LLM_NOTCH_PANE_ID";
/// Native HWND for window-focus fallback (collector-supplied only).
pub const ENV_WINDOW_HANDLE: &str = "LLM_NOTCH_WINDOW_HANDLE";

/// Snapshot of terminal metadata readable from the current process environment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WtCollectorSnapshot {
    pub terminal_session_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub window_handle: Option<u64>,
    pub wt_profile_id: Option<String>,
    pub wt_profile_name: Option<String>,
}

/// Optional explicit values from a profile command line or wrapper script.
///
/// These are **not** discovered from Windows Terminal APIs. Callers must only
/// supply values the user configured because WT does not publish tab/pane indices.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WtCollectorOverrides {
    pub terminal_session_id: Option<String>,
    pub tab_id: Option<String>,
    pub pane_id: Option<String>,
    pub window_handle: Option<String>,
}

impl WtCollectorSnapshot {
    pub fn has_navigation_metadata(&self) -> bool {
        self.terminal_session_id.is_some()
            || self.tab_id.is_some()
            || self.pane_id.is_some()
            || self.window_handle.is_some()
    }

    pub fn has_exact_pane_route(&self) -> bool {
        self.terminal_session_id.is_some() && self.tab_id.is_some() && self.pane_id.is_some()
    }
}

/// Reads collector fields from the current process environment.
pub fn collect_wt_metadata_from_env() -> WtCollectorSnapshot {
    collect_wt_metadata(|name| std::env::var(name), WtCollectorOverrides::default())
}

/// Reads collector fields using the supplied env accessor and optional overrides.
pub fn collect_wt_metadata(
    read_env: impl Fn(&str) -> Result<String, std::env::VarError>,
    overrides: WtCollectorOverrides,
) -> WtCollectorSnapshot {
    WtCollectorSnapshot {
        terminal_session_id: pick_terminal_session_id(&read_env, &overrides),
        tab_id: pick_string_field(&read_env, ENV_TAB_ID, overrides.tab_id.as_deref()),
        pane_id: pick_string_field(&read_env, ENV_PANE_ID, overrides.pane_id.as_deref()),
        window_handle: pick_window_handle(
            &read_env,
            overrides.window_handle.as_deref(),
            discover_window_handle,
        ),
        wt_profile_id: read_trimmed_env(&read_env, ENV_WT_PROFILE_ID),
        wt_profile_name: read_trimmed_env(&read_env, ENV_WT_PROFILE_NAME),
    }
}

/// Builds `(name, value)` pairs suitable for exporting to child hook processes.
///
/// Only populated fields are returned. Existing env values are not overwritten by
/// callers; this function simply lists what the snapshot contains.
pub fn collector_env_exports(snapshot: &WtCollectorSnapshot) -> Vec<(&'static str, String)> {
    let mut exports = Vec::new();
    if let Some(value) = snapshot.terminal_session_id.as_ref() {
        exports.push((ENV_TERMINAL_SESSION_ID, value.clone()));
    }
    if let Some(value) = snapshot.tab_id.as_ref() {
        exports.push((ENV_TAB_ID, value.clone()));
    }
    if let Some(value) = snapshot.pane_id.as_ref() {
        exports.push((ENV_PANE_ID, value.clone()));
    }
    if let Some(value) = snapshot.window_handle {
        exports.push((ENV_WINDOW_HANDLE, value.to_string()));
    }
    exports
}

fn pick_terminal_session_id(
    read_env: &impl Fn(&str) -> Result<String, std::env::VarError>,
    overrides: &WtCollectorOverrides,
) -> Option<String> {
    read_trimmed_env(read_env, ENV_TERMINAL_SESSION_ID)
        .or_else(|| read_trimmed_env(read_env, ENV_WT_SESSION))
        .or_else(|| trimmed_non_empty(overrides.terminal_session_id.as_deref()))
}

fn pick_string_field(
    read_env: &impl Fn(&str) -> Result<String, std::env::VarError>,
    env_name: &str,
    override_value: Option<&str>,
) -> Option<String> {
    read_trimmed_env(read_env, env_name).or_else(|| trimmed_non_empty(override_value))
}

fn pick_window_handle(
    read_env: &impl Fn(&str) -> Result<String, std::env::VarError>,
    override_value: Option<&str>,
    discover: impl FnOnce() -> Option<u64>,
) -> Option<u64> {
    parse_window_handle(read_trimmed_env(read_env, ENV_WINDOW_HANDLE).as_deref())
        .or_else(|| parse_window_handle(trimmed_non_empty(override_value).as_deref()))
        .or_else(discover)
}

fn discover_window_handle() -> Option<u64> {
    #[cfg(test)]
    {
        return None;
    }
    #[cfg(not(test))]
    {
        crate::hwnd_collector::discover_terminal_window_handle()
    }
}

fn read_trimmed_env(
    read_env: &impl Fn(&str) -> Result<String, std::env::VarError>,
    name: &str,
) -> Option<String> {
    trimmed_non_empty(read_env(name).ok().as_deref())
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_window_handle(value: Option<&str>) -> Option<u64> {
    crate::hwnd_collector::parse_window_handle(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_map(values: &[(&str, &str)]) -> impl Fn(&str) -> Result<String, std::env::VarError> {
        let map: HashMap<String, String> = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |name: &str| {
            map.get(name)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        }
    }

    #[test]
    fn empty_env_yields_empty_snapshot() {
        let snapshot = collect_wt_metadata(|_| Err(std::env::VarError::NotPresent), Default::default());
        assert_eq!(snapshot, WtCollectorSnapshot::default());
        assert!(!snapshot.has_navigation_metadata());
    }

    #[test]
    fn wt_session_maps_to_terminal_session_id() {
        let snapshot = collect_wt_metadata(
            env_map(&[("WT_SESSION", "5720ee6d-6474-47b0-88db-fa7e10e60d37")]),
            Default::default(),
        );
        assert_eq!(
            snapshot.terminal_session_id.as_deref(),
            Some("5720ee6d-6474-47b0-88db-fa7e10e60d37")
        );
        assert!(snapshot.tab_id.is_none());
        assert!(snapshot.pane_id.is_none());
    }

    #[test]
    fn explicit_terminal_session_id_wins_over_wt_session() {
        let snapshot = collect_wt_metadata(
            env_map(&[
                ("WT_SESSION", "guid-from-wt"),
                ("LLM_NOTCH_TERMINAL_SESSION_ID", "user-override"),
            ]),
            Default::default(),
        );
        assert_eq!(
            snapshot.terminal_session_id.as_deref(),
            Some("user-override")
        );
    }

    #[test]
    fn tab_and_pane_pass_through_without_invention() {
        let snapshot = collect_wt_metadata(
            env_map(&[
                ("LLM_NOTCH_TAB_ID", "2"),
                ("LLM_NOTCH_PANE_ID", "1"),
            ]),
            Default::default(),
        );
        assert_eq!(snapshot.tab_id.as_deref(), Some("2"));
        assert_eq!(snapshot.pane_id.as_deref(), Some("1"));
        assert!(!snapshot.has_exact_pane_route());
    }

    #[test]
    fn exact_pane_route_requires_all_three_indices() {
        let snapshot = collect_wt_metadata(
            env_map(&[
                ("WT_SESSION", "0"),
                ("LLM_NOTCH_TAB_ID", "1"),
                ("LLM_NOTCH_PANE_ID", "0"),
            ]),
            Default::default(),
        );
        assert!(snapshot.has_exact_pane_route());
    }

    #[test]
    fn overrides_apply_only_when_env_absent() {
        let snapshot = collect_wt_metadata(
            env_map(&[("LLM_NOTCH_TAB_ID", "3")]),
            WtCollectorOverrides {
                tab_id: Some("9".into()),
                pane_id: Some("1".into()),
                ..Default::default()
            },
        );
        assert_eq!(snapshot.tab_id.as_deref(), Some("3"));
        assert_eq!(snapshot.pane_id.as_deref(), Some("1"));
    }

    #[test]
    fn profile_metadata_is_informational() {
        let snapshot = collect_wt_metadata(
            env_map(&[
                ("WT_PROFILE_ID", "profile-guid"),
                ("WT_PROFILE_NAME", "PowerShell"),
            ]),
            Default::default(),
        );
        assert_eq!(snapshot.wt_profile_id.as_deref(), Some("profile-guid"));
        assert_eq!(snapshot.wt_profile_name.as_deref(), Some("PowerShell"));
        assert!(!snapshot.has_navigation_metadata());
    }

    #[test]
    fn collector_env_exports_include_only_populated_fields() {
        let snapshot = WtCollectorSnapshot {
            terminal_session_id: Some("wt-guid".into()),
            tab_id: Some("1".into()),
            pane_id: Some("0".into()),
            window_handle: Some(42),
            ..Default::default()
        };
        let exports = collector_env_exports(&snapshot);
        assert_eq!(
            exports,
            vec![
                (ENV_TERMINAL_SESSION_ID, "wt-guid".into()),
                (ENV_TAB_ID, "1".into()),
                (ENV_PANE_ID, "0".into()),
                (ENV_WINDOW_HANDLE, "42".into()),
            ]
        );
    }

    #[test]
    fn window_handle_rejects_zero_and_invalid() {
        let snapshot = collect_wt_metadata(
            env_map(&[("LLM_NOTCH_WINDOW_HANDLE", "0")]),
            Default::default(),
        );
        assert!(snapshot.window_handle.is_none());

        let invalid = collect_wt_metadata(
            env_map(&[("LLM_NOTCH_WINDOW_HANDLE", "not-a-number")]),
            Default::default(),
        );
        assert!(invalid.window_handle.is_none());
    }

    #[test]
    fn window_handle_does_not_trust_unverified_env_numbers() {
        let snapshot = collect_wt_metadata(
            env_map(&[("LLM_NOTCH_WINDOW_HANDLE", "4242")]),
            Default::default(),
        );
        assert!(snapshot.window_handle.is_none());
    }
}
