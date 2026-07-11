//! Resolve opaque locators from live sessions and process ancestry.

use notch_protocol::{AgentSession, AgentSource, ProcessIdentity};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::context::locator::{ContextLocator, HostKind, LocatorError};

const MAX_PARENT_HOPS: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub locator: ContextLocator,
    pub host: HostKind,
    pub pane_verified: bool,
}

pub fn resolve_session(session: &AgentSession) -> Result<Option<ResolvedContext>, LocatorError> {
    let Some(root) = session.process_root.as_ref() else {
        return Ok(None);
    };
    let host = detect_host_for_root(root)?;
    let pane_verified = pane_verified_for_host(host, session);
    let pane_hint = pane_hint_for_session(session);
    let locator = ContextLocator::encode(host, Some(root.clone()), pane_hint.as_deref())?;
    Ok(Some(ResolvedContext {
        locator,
        host,
        pane_verified,
    }))
}

pub fn detect_host_for_root(root: &ProcessIdentity) -> Result<HostKind, LocatorError> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut current = root.pid;
    for _ in 0..MAX_PARENT_HOPS {
        let Some(process) = system.process(Pid::from_u32(current)) else {
            break;
        };
        if let Some(host) = host_from_process_name(&process.name().to_string_lossy()) {
            return Ok(host);
        }
        let Some(parent) = process.parent() else {
            break;
        };
        current = parent.as_u32();
    }
    Ok(host_from_source_fallback(root))
}

fn host_from_process_name(name: &str) -> Option<HostKind> {
    let normalized = name.to_ascii_lowercase();
    if normalized.contains("windowsterminal") {
        return Some(HostKind::WindowsTerminal);
    }
    if normalized == "cursor" || normalized.contains("cursor.exe") {
        return Some(HostKind::Cursor);
    }
    if normalized == "code"
        || normalized.contains("code.exe")
        || normalized.contains("code - insiders")
    {
        return Some(HostKind::VsCode);
    }
    if normalized == "terminal" || normalized.contains("terminal.app") {
        return Some(HostKind::TerminalApp);
    }
    if normalized.contains("iterm2") || normalized == "iterm" {
        return Some(HostKind::ITerm2);
    }
    None
}

fn host_from_source_fallback(_root: &ProcessIdentity) -> HostKind {
    HostKind::UnknownHost
}

fn host_from_source(source: AgentSource) -> HostKind {
    match source {
        AgentSource::Cursor => HostKind::Cursor,
        AgentSource::ClaudeCode | AgentSource::Codex | AgentSource::Generic => {
            HostKind::UnknownHost
        }
        AgentSource::Unknown => HostKind::UnknownHost,
    }
}

fn pane_verified_for_host(host: HostKind, session: &AgentSession) -> bool {
    match host {
        HostKind::TerminalApp | HostKind::ITerm2 => {
            session.workspace_label.is_some() && session.process_root.is_some()
        }
        _ => false,
    }
}

fn pane_hint_for_session(session: &AgentSession) -> Option<String> {
    session.workspace_label.as_ref().and_then(|label| {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.chars().take(64).collect())
        }
    })
}

pub fn infer_host_from_source(source: AgentSource) -> HostKind {
    host_from_source(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{AttentionKind, SessionStatus};

    fn sample_session(source: AgentSource, root: Option<ProcessIdentity>) -> AgentSession {
        AgentSession {
            id: "sess-1".into(),
            source,
            external_session_id: "ext-1".into(),
            label: "test".into(),
            workspace_label: Some("workspace-a".into()),
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 1,
            last_event_at_ms: 2,
            ended_at_ms: None,
            process_root: root,
            latest_metric: None,
        }
    }

    #[test]
    fn session_without_process_root_returns_none() {
        let session = sample_session(AgentSource::Cursor, None);
        assert!(resolve_session(&session).unwrap().is_none());
    }

    #[test]
    fn maps_process_names_to_hosts() {
        assert_eq!(
            host_from_process_name("WindowsTerminal.exe"),
            Some(HostKind::WindowsTerminal)
        );
        assert_eq!(host_from_process_name("Cursor.exe"), Some(HostKind::Cursor));
        assert_eq!(host_from_process_name("Code.exe"), Some(HostKind::VsCode));
        assert_eq!(
            host_from_process_name("Terminal"),
            Some(HostKind::TerminalApp)
        );
        assert_eq!(host_from_process_name("iTerm2"), Some(HostKind::ITerm2));
    }

    #[test]
    fn cursor_source_session_encodes_locator() {
        let session = sample_session(
            AgentSource::Cursor,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 1_700_000_000_000,
            }),
        );
        let resolved = resolve_session(&session).expect("resolve").expect("some");
        assert!(resolved.locator.token().starts_with("ln1_"));
        assert_eq!(
            infer_host_from_source(AgentSource::Cursor),
            HostKind::Cursor
        );
    }

    #[test]
    fn live_process_detection_smoke() {
        let pid = std::process::id();
        let host = detect_host_for_root(&ProcessIdentity {
            pid,
            started_at_ms: 0,
        })
        .expect("detect");
        assert!(matches!(
            host,
            HostKind::Cursor
                | HostKind::VsCode
                | HostKind::WindowsTerminal
                | HostKind::TerminalApp
                | HostKind::ITerm2
                | HostKind::UnknownHost
        ));
    }
}
