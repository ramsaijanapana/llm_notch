//! Opaque context locators: encode, decode, and validate wire tokens.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use notch_protocol::{ProcessIdentity, VerifiedTerminalContext};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const LOCATOR_PREFIX: &str = "ln1_";
pub const MAX_LOCATOR_LEN: usize = 512;
pub const MAX_PANE_HINT_LEN: usize = 64;
pub const MAX_TERMINAL_ID_LEN: usize = 128;
const PAYLOAD_VERSION: u8 = 1;

/// Supported host applications for first-release context navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    TerminalApp,
    ITerm2,
    WindowsTerminal,
    VsCode,
    Cursor,
    UnknownHost,
}

/// Internal locator payload — never sent to the renderer as structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct LocatorPayload {
    v: u8,
    host: HostKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tab_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pane_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_handle: Option<u64>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocatorError {
    #[error("locator is empty")]
    Empty,
    #[error("locator exceeds maximum length")]
    TooLong,
    #[error("locator has invalid prefix")]
    InvalidPrefix,
    #[error("locator contains unsafe characters")]
    UnsafeCharacters,
    #[error("locator encoding is invalid")]
    InvalidEncoding,
    #[error("locator payload is invalid")]
    InvalidPayload,
    #[error("locator pane hint is invalid")]
    InvalidPaneHint,
    #[error("locator verified terminal field is invalid")]
    InvalidVerifiedTerminalField,
    #[error("locator contains path escape")]
    PathEscape,
}

/// Opaque validated locator token exchanged on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextLocator {
    token: String,
    payload: LocatorPayload,
}

impl ContextLocator {
    pub fn encode(
        host: HostKind,
        process: Option<ProcessIdentity>,
        pane_hint: Option<&str>,
        verified_terminal: Option<&VerifiedTerminalContext>,
    ) -> Result<Self, LocatorError> {
        if let Some(hint) = pane_hint {
            validate_pane_hint(hint)?;
        }
        let verified = verified_terminal.cloned().unwrap_or_default();
        validate_verified_terminal_fields(&verified)?;
        let payload = LocatorPayload {
            v: PAYLOAD_VERSION,
            host,
            pid: process.as_ref().map(|identity| identity.pid),
            started_at_ms: process.as_ref().map(|identity| identity.started_at_ms),
            pane_hint: pane_hint.map(str::to_string),
            terminal_session_id: verified.terminal_session_id,
            tab_id: verified.tab_id,
            pane_id: verified.pane_id,
            window_handle: verified.window_handle,
        };
        let json = serde_json::to_vec(&payload).map_err(|_| LocatorError::InvalidPayload)?;
        let encoded = URL_SAFE_NO_PAD.encode(json);
        let token = format!("{LOCATOR_PREFIX}{encoded}");
        if token.len() > MAX_LOCATOR_LEN {
            return Err(LocatorError::TooLong);
        }
        Ok(Self { token, payload })
    }

    pub fn parse(token: &str) -> Result<Self, LocatorError> {
        validate_wire_token(token)?;
        let encoded = token
            .strip_prefix(LOCATOR_PREFIX)
            .ok_or(LocatorError::InvalidPrefix)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| LocatorError::InvalidEncoding)?;
        let payload: LocatorPayload =
            serde_json::from_slice(&bytes).map_err(|_| LocatorError::InvalidPayload)?;
        if payload.v != PAYLOAD_VERSION {
            return Err(LocatorError::InvalidPayload);
        }
        if let Some(hint) = &payload.pane_hint {
            validate_pane_hint(hint)?;
        }
        validate_verified_terminal_fields(&verified_terminal_from_payload(&payload))?;
        Ok(Self {
            token: token.to_string(),
            payload,
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn host(&self) -> HostKind {
        self.payload.host
    }

    pub fn process(&self) -> Option<ProcessIdentity> {
        match (self.payload.pid, self.payload.started_at_ms) {
            (Some(pid), Some(started_at_ms)) => Some(ProcessIdentity { pid, started_at_ms }),
            _ => None,
        }
    }

    pub fn pane_hint(&self) -> Option<&str> {
        self.payload.pane_hint.as_deref()
    }

    pub fn verified_terminal(&self) -> VerifiedTerminalContext {
        verified_terminal_from_payload(&self.payload)
    }

    /// True when verified tab/pane/session metadata is complete for this host.
    pub fn pane_verified(&self) -> bool {
        pane_verified_for_host(self.host(), &self.verified_terminal())
    }
}

pub fn verified_terminal_from_payload(payload: &LocatorPayload) -> VerifiedTerminalContext {
    VerifiedTerminalContext {
        terminal_session_id: payload.terminal_session_id.clone(),
        tab_id: payload.tab_id.clone(),
        pane_id: payload.pane_id.clone(),
        window_handle: payload.window_handle,
    }
}

/// Returns whether verified metadata supports exact-pane navigation for `host`.
pub fn pane_verified_for_host(host: HostKind, terminal: &VerifiedTerminalContext) -> bool {
    match host {
        HostKind::WindowsTerminal | HostKind::VsCode | HostKind::Cursor => {
            terminal.terminal_session_id.is_some()
                && terminal.tab_id.is_some()
                && terminal.pane_id.is_some()
        }
        HostKind::TerminalApp | HostKind::ITerm2 => {
            terminal.pane_id.is_some()
                && (terminal.tab_id.is_some() || terminal.terminal_session_id.is_some())
        }
        HostKind::UnknownHost => false,
    }
}

pub fn validate_wire_token(token: &str) -> Result<(), LocatorError> {
    if token.is_empty() {
        return Err(LocatorError::Empty);
    }
    if token.len() > MAX_LOCATOR_LEN {
        return Err(LocatorError::TooLong);
    }
    if !token.starts_with(LOCATOR_PREFIX) {
        return Err(LocatorError::InvalidPrefix);
    }
    if contains_unsafe_shell_chars(token) {
        return Err(LocatorError::UnsafeCharacters);
    }
    if token.contains("..") {
        return Err(LocatorError::PathEscape);
    }
    let suffix = &token[LOCATOR_PREFIX.len()..];
    if suffix.is_empty()
        || !suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err(LocatorError::InvalidEncoding);
    }
    Ok(())
}

fn validate_verified_terminal_fields(terminal: &VerifiedTerminalContext) -> Result<(), LocatorError> {
    for value in [
        terminal.terminal_session_id.as_deref(),
        terminal.tab_id.as_deref(),
        terminal.pane_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_terminal_id(value)?;
    }
    Ok(())
}

fn validate_terminal_id(value: &str) -> Result<(), LocatorError> {
    if value.is_empty() || value.len() > MAX_TERMINAL_ID_LEN {
        return Err(LocatorError::InvalidVerifiedTerminalField);
    }
    if value.contains("..") || value.contains('/') || value.contains('\\') {
        return Err(LocatorError::PathEscape);
    }
    if contains_unsafe_shell_chars(value) {
        return Err(LocatorError::UnsafeCharacters);
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '%')
    }) {
        return Err(LocatorError::InvalidVerifiedTerminalField);
    }
    Ok(())
}

fn validate_pane_hint(hint: &str) -> Result<(), LocatorError> {
    if hint.is_empty() || hint.len() > MAX_PANE_HINT_LEN {
        return Err(LocatorError::InvalidPaneHint);
    }
    if hint.contains("..") || hint.contains('/') || hint.contains('\\') {
        return Err(LocatorError::PathEscape);
    }
    if contains_unsafe_shell_chars(hint) {
        return Err(LocatorError::UnsafeCharacters);
    }
    if !hint
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
    {
        return Err(LocatorError::InvalidPaneHint);
    }
    Ok(())
}

fn contains_unsafe_shell_chars(value: &str) -> bool {
    value.chars().any(|ch| {
        matches!(
            ch,
            ';' | '|' | '&' | '$' | '`' | '\n' | '\r' | '\0' | '<' | '>' | '(' | ')' | '{' | '}'
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_locator_payload() {
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            }),
            Some("tab-main"),
            None,
        )
        .expect("encode");
        let parsed = ContextLocator::parse(locator.token()).expect("parse");
        assert_eq!(parsed.host(), HostKind::WindowsTerminal);
        assert_eq!(
            parsed.process(),
            Some(ProcessIdentity {
                pid: 4242,
                started_at_ms: 1_700_000_000_000,
            })
        );
        assert_eq!(parsed.pane_hint(), Some("tab-main"));
    }

    #[test]
    fn rejects_path_like_tokens() {
        assert!(validate_wire_token("ln1_../etc/passwd").is_err());
        assert!(validate_wire_token("").is_err());
        assert!(validate_wire_token("bad_prefix_abc").is_err());
    }

    #[test]
    fn rejects_shell_metacharacters() {
        assert!(validate_wire_token("ln1_abc;rm").is_err());
        assert!(ContextLocator::encode(HostKind::Cursor, None, Some("pane|1"), None).is_err());
    }

    #[test]
    fn rejects_overlong_locator() {
        let huge = format!("{LOCATOR_PREFIX}{}", "a".repeat(MAX_LOCATOR_LEN));
        assert!(validate_wire_token(&huge).is_err());
    }

    #[test]
    fn rejects_invalid_pane_hint_paths() {
        assert!(ContextLocator::encode(HostKind::VsCode, None, Some("../secret"), None).is_err());
        assert!(ContextLocator::encode(HostKind::VsCode, None, Some("ok-pane_1"), None).is_ok());
    }

    #[test]
    fn round_trips_verified_terminal_metadata() {
        let verified = VerifiedTerminalContext {
            terminal_session_id: Some("0".into()),
            tab_id: Some("2".into()),
            pane_id: Some("1".into()),
            window_handle: Some(99),
        };
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            None,
            None,
            Some(&verified),
        )
        .expect("encode");
        let parsed = ContextLocator::parse(locator.token()).expect("parse");
        assert!(parsed.pane_verified());
        assert_eq!(parsed.verified_terminal(), verified);
    }

    #[test]
    fn pane_verified_requires_complete_wt_metadata() {
        let partial = VerifiedTerminalContext {
            tab_id: Some("1".into()),
            ..Default::default()
        };
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            None,
            None,
            Some(&partial),
        )
        .expect("encode");
        assert!(!locator.pane_verified());
    }
}
