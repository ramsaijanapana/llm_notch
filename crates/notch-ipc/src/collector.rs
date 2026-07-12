//! Terminal metadata collectors for hook ingest payloads.
//!
//! # Sources that can supply verified terminal fields
//!
//! | Field | Source | Notes |
//! |-------|--------|-------|
//! | `terminal_session_id` | `WT_SESSION` (Windows Terminal) | GUID per tab/pane connection; inherited by child processes when WT sets it. Unreliable when WT is the default terminal handler via Start Menu shortcuts. |
//! | `terminal_session_id` | `LLM_NOTCH_TERMINAL_SESSION_ID` | Explicit collector override for non-WT hosts or when `WT_SESSION` is absent. |
//! | `tab_id` | `LLM_NOTCH_TAB_ID` | Numeric tab index for WT exact-pane (`wt.exe focus-tab -t`). Not provided by Windows Terminal env vars today; see `notch-platform::wt_collector` and `integrations/windows-terminal/README.md`. |
//! | `pane_id` | `LLM_NOTCH_PANE_ID` | Numeric pane index for WT exact-pane (`wt.exe focus-pane -t`). Requires a shell-integration or wrapper collector; WT does not publish pane indices. |
//! | `window_handle` | `LLM_NOTCH_WINDOW_HANDLE` | Native HWND for window-focus fallback; set by a platform collector or discovered from the process tree on Windows. |
//! | any field | Ingest wire JSON (`terminalSessionId`, `tabId`, `paneId`, `windowHandle`) | Normalized stdin / `emit` CLI paths. Payload values win over env when both are set. |
//!
//! Vendor hook stdin JSON does **not** carry terminal metadata today. Collectors must inject
//! env vars or use normalized/emit ingest. IDs are never invented or parsed from window titles.

use std::env;

use notch_protocol::VerifiedTerminalContext;

use crate::error::{IpcError, IpcResult};
use crate::wire::IngestPayload;

/// Windows Terminal session GUID (`WT_SESSION`).
pub const ENV_WT_SESSION: &str = "WT_SESSION";
/// Explicit terminal session identifier override.
pub const ENV_TERMINAL_SESSION_ID: &str = "LLM_NOTCH_TERMINAL_SESSION_ID";
/// Tab index string for exact-pane navigation (collector-supplied).
pub const ENV_TAB_ID: &str = "LLM_NOTCH_TAB_ID";
/// Pane index string for exact-pane navigation (collector-supplied).
pub const ENV_PANE_ID: &str = "LLM_NOTCH_PANE_ID";
/// Native window handle for verified window-focus activation.
pub const ENV_WINDOW_HANDLE: &str = "LLM_NOTCH_WINDOW_HANDLE";

const MAX_TERMINAL_ID_LEN: usize = 128;

/// Fills missing ingest terminal fields from the hook process environment.
///
/// Existing payload fields are preserved; env vars only supply values that are still `None`.
pub fn enrich_ingest_with_collector_env(payload: &mut IngestPayload) {
    if payload.terminal_session_id.is_none() {
        payload.terminal_session_id = read_terminal_session_id_from_env();
    }
    if payload.tab_id.is_none() {
        payload.tab_id = read_env_string(ENV_TAB_ID);
    }
    if payload.pane_id.is_none() {
        payload.pane_id = read_env_string(ENV_PANE_ID);
    }
    if payload.window_handle.is_none() {
        payload.window_handle = read_window_handle_from_env().or_else(discover_window_handle);
    }
}

fn discover_window_handle() -> Option<u64> {
    #[cfg(test)]
    {
        return None;
    }
    #[cfg(all(not(test), windows))]
    {
        return notch_platform::discover_terminal_window_handle();
    }
    #[cfg(all(not(test), not(windows)))]
    {
        None
    }
}

/// Builds `verified_terminal` for session normalization when any collector field is present.
pub fn verified_terminal_from_ingest(payload: &IngestPayload) -> Option<VerifiedTerminalContext> {
    let terminal = VerifiedTerminalContext {
        terminal_session_id: payload.terminal_session_id.clone(),
        tab_id: payload.tab_id.clone(),
        pane_id: payload.pane_id.clone(),
        window_handle: payload.window_handle,
    };
    if terminal.terminal_session_id.is_none()
        && terminal.tab_id.is_none()
        && terminal.pane_id.is_none()
        && terminal.window_handle.is_none()
    {
        return None;
    }
    Some(terminal)
}

pub fn validate_terminal_id_field(value: &str, field: &str) -> IpcResult<()> {
    if value.is_empty() || value.len() > MAX_TERMINAL_ID_LEN {
        return Err(IpcError::FrameRejected(format!(
            "{field} length must be 1..={MAX_TERMINAL_ID_LEN}"
        )));
    }
    if value.contains("..") || value.contains('/') || value.contains('\\') {
        return Err(IpcError::FrameRejected(format!("{field} contains path escape")));
    }
    if contains_unsafe_shell_chars(value) {
        return Err(IpcError::FrameRejected(format!(
            "{field} contains unsafe shell characters"
        )));
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '%')
    }) {
        return Err(IpcError::FrameRejected(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(())
}

fn read_terminal_session_id_from_env() -> Option<String> {
    read_env_string(ENV_TERMINAL_SESSION_ID).or_else(|| read_env_string(ENV_WT_SESSION))
}

fn read_env_string(name: &str) -> Option<String> {
    let value = env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn read_window_handle_from_env() -> Option<u64> {
    notch_platform::parse_window_handle(std::env::var(ENV_WINDOW_HANDLE).ok().as_deref())
}

fn contains_unsafe_shell_chars(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '"' | '\'' | '`' | '$' | ';' | '|' | '&' | '<' | '>' | '\n' | '\r'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::IngestPayload;

    fn sample_payload() -> IngestPayload {
        IngestPayload {
            source: "cursor".into(),
            event: "sessionStart".into(),
            session_id: None,
            external_session_id: Some("ext-1".into()),
            label: None,
            workspace_label: None,
            status: None,
            attention: None,
            summary: None,
            tool_name: None,
            pid: None,
            process_started_at_ms: None,
            occurred_at_ms: Some(1),
            terminal_session_id: None,
            tab_id: None,
            pane_id: None,
            window_handle: None,
        }
    }

    #[test]
    fn absent_terminal_fields_yield_none() {
        assert!(verified_terminal_from_ingest(&sample_payload()).is_none());
    }

    #[test]
    fn partial_terminal_fields_are_passed_through_without_invention() {
        let mut payload = sample_payload();
        payload.terminal_session_id = Some("5720ee6d-6474-47b0-88db-fa7e10e60d37".into());
        let terminal = verified_terminal_from_ingest(&payload).expect("terminal");
        assert_eq!(
            terminal.terminal_session_id.as_deref(),
            Some("5720ee6d-6474-47b0-88db-fa7e10e60d37")
        );
        assert!(terminal.tab_id.is_none());
        assert!(terminal.pane_id.is_none());
        assert!(terminal.window_handle.is_none());
    }

    #[test]
    fn complete_wt_indices_map_to_verified_terminal() {
        let mut payload = sample_payload();
        payload.terminal_session_id = Some("0".into());
        payload.tab_id = Some("1".into());
        payload.pane_id = Some("0".into());
        let terminal = verified_terminal_from_ingest(&payload).expect("terminal");
        assert_eq!(terminal.tab_id.as_deref(), Some("1"));
        assert_eq!(terminal.pane_id.as_deref(), Some("0"));
    }

    #[test]
    fn payload_fields_are_not_overwritten_by_env_enrichment() {
        let mut payload = sample_payload();
        payload.tab_id = Some("2".into());
        unsafe {
            env::set_var(ENV_TAB_ID, "9");
            env::set_var(ENV_WT_SESSION, "guid-from-env");
        }
        enrich_ingest_with_collector_env(&mut payload);
        assert_eq!(payload.tab_id.as_deref(), Some("2"));
        assert_eq!(
            payload.terminal_session_id.as_deref(),
            Some("guid-from-env")
        );
        unsafe {
            env::remove_var(ENV_TAB_ID);
            env::remove_var(ENV_WT_SESSION);
        }
    }

    #[test]
    fn env_enrichment_fills_missing_fields() {
        let mut payload = sample_payload();
        unsafe {
            env::set_var(ENV_WT_SESSION, "wt-session-guid");
            env::set_var(ENV_TAB_ID, "1");
            env::set_var(ENV_PANE_ID, "0");
            env::set_var(ENV_WINDOW_HANDLE, "12345");
        }
        enrich_ingest_with_collector_env(&mut payload);
        assert_eq!(
            payload.terminal_session_id.as_deref(),
            Some("wt-session-guid")
        );
        assert_eq!(payload.tab_id.as_deref(), Some("1"));
        assert_eq!(payload.pane_id.as_deref(), Some("0"));
        // Arbitrary env numbers are not trusted without Win32 HWND verification.
        assert!(payload.window_handle.is_none());
        unsafe {
            env::remove_var(ENV_WT_SESSION);
            env::remove_var(ENV_TAB_ID);
            env::remove_var(ENV_PANE_ID);
            env::remove_var(ENV_WINDOW_HANDLE);
        }
    }

    #[test]
    fn rejects_unsafe_terminal_id_chars() {
        assert!(validate_terminal_id_field("bad;id", "tabId").is_err());
    }
}
