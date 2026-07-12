//! Resolve opaque locators from live sessions and process ancestry.

use notch_platform::{
    NavigationTier, ProcessDescriptor, TerminalHost, VerifiedTerminalMetadata, current_navigator,
};
use notch_protocol::{
    AgentSession, AgentSource, ContextOpenTier, ProcessIdentity, VerifiedTerminalContext,
};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::context::locator::{ContextLocator, HostKind, LocatorError, pane_verified_for_host};

const MAX_PARENT_HOPS: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    pub locator: ContextLocator,
    pub host: HostKind,
    pub discovered_tier: ContextOpenTier,
    pub pane_verified: bool,
}

pub fn resolve_session(session: &AgentSession) -> Result<Option<ResolvedContext>, LocatorError> {
    let Some(root) = session.process_root.as_ref() else {
        return Ok(None);
    };
    let verified_terminal = session.verified_terminal.as_ref();
    let discovery = detect_navigation_for_root(root, verified_terminal)?;
    let host = discovery.host;
    let pane_hint = pane_hint_for_session(session);
    let locator = ContextLocator::encode(
        host,
        Some(root.clone()),
        pane_hint.as_deref(),
        verified_terminal,
    )?;
    let pane_verified = pane_verified_for_host(host, &locator.verified_terminal());
    Ok(Some(ResolvedContext {
        locator,
        host,
        discovered_tier: discovery.tier,
        pane_verified,
    }))
}

pub fn detect_host_for_root(root: &ProcessIdentity) -> Result<HostKind, LocatorError> {
    Ok(detect_navigation_for_root(root, None)?.host)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NavigationDiscovery {
    host: HostKind,
    tier: ContextOpenTier,
}

fn detect_navigation_for_root(
    root: &ProcessIdentity,
    verified_terminal: Option<&VerifiedTerminalContext>,
) -> Result<NavigationDiscovery, LocatorError> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut current = root.pid;
    let mut root_executable = String::new();
    for _ in 0..MAX_PARENT_HOPS {
        let Some(process) = system.process(Pid::from_u32(current)) else {
            break;
        };
        let executable = process.name().to_string_lossy().into_owned();
        if root_executable.is_empty() {
            root_executable.clone_from(&executable);
        }
        if let Some(host) = host_from_process_name(&executable) {
            return Ok(discover_verified_process(
                root,
                host,
                &root_executable,
                &executable,
                verified_terminal,
            ));
        }
        let Some(parent) = process.parent() else {
            break;
        };
        current = parent.as_u32();
    }
    Ok(NavigationDiscovery {
        host: host_from_source_fallback(root),
        tier: ContextOpenTier::None,
    })
}

fn discover_verified_process(
    root: &ProcessIdentity,
    fallback_host: HostKind,
    root_executable: &str,
    terminal_executable: &str,
    verified_terminal: Option<&VerifiedTerminalContext>,
) -> NavigationDiscovery {
    let descriptor = ProcessDescriptor {
        process_id: root.pid,
        process_started_at_ms: u64::try_from(root.started_at_ms).ok(),
        executable: root_executable.to_string(),
        parent_executable: None,
        terminal_executable: Some(terminal_executable.to_string()),
        metadata: build_verified_metadata(terminal_executable, verified_terminal),
    };
    let locator = current_navigator().discover(&descriptor);
    NavigationDiscovery {
        host: map_platform_host(locator.host()).unwrap_or(fallback_host),
        tier: map_platform_tier(locator.tier()),
    }
}

fn build_verified_metadata(
    terminal_executable: &str,
    verified_terminal: Option<&VerifiedTerminalContext>,
) -> VerifiedTerminalMetadata {
    let mut metadata = VerifiedTerminalMetadata {
        application_id: Some(terminal_executable.to_string()),
        ..Default::default()
    };
    if let Some(terminal) = verified_terminal {
        metadata.terminal_session_id = terminal.terminal_session_id.clone();
        metadata.tab_id = terminal.tab_id.clone();
        metadata.pane_id = terminal.pane_id.clone();
        metadata.window_handle = terminal.window_handle;
    }
    metadata
}

fn map_platform_host(host: &TerminalHost) -> Option<HostKind> {
    match host {
        TerminalHost::WindowsTerminal => Some(HostKind::WindowsTerminal),
        TerminalHost::VsCode => Some(HostKind::VsCode),
        TerminalHost::Cursor => Some(HostKind::Cursor),
        TerminalHost::MacTerminal => Some(HostKind::TerminalApp),
        TerminalHost::ITerm2 => Some(HostKind::ITerm2),
        TerminalHost::ConsoleHost
        | TerminalHost::PowerShell
        | TerminalHost::WezTerm
        | TerminalHost::Wsl
        | TerminalHost::Tmux
        | TerminalHost::Other(_)
        | TerminalHost::Unknown => None,
    }
}

fn map_platform_tier(tier: NavigationTier) -> ContextOpenTier {
    match tier {
        NavigationTier::Unsupported => ContextOpenTier::None,
        NavigationTier::AppActivate => ContextOpenTier::AppActivate,
        NavigationTier::WindowFocus => ContextOpenTier::WindowFocus,
        NavigationTier::ExactPane => ContextOpenTier::ExactPane,
    }
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
        AgentSource::ClaudeCode
        | AgentSource::Codex
        | AgentSource::Gemini
        | AgentSource::Qwen
        | AgentSource::AntigravityCli
        | AgentSource::CopilotCli
        | AgentSource::Generic => HostKind::UnknownHost,
        AgentSource::Unknown => HostKind::UnknownHost,
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
            verified_terminal: None,
            latest_metric: None,
        }
    }

    fn wt_verified_terminal() -> VerifiedTerminalContext {
        VerifiedTerminalContext {
            terminal_session_id: Some("0".into()),
            tab_id: Some("1".into()),
            pane_id: Some("0".into()),
            window_handle: None,
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
    #[cfg(windows)]
    fn verified_ancestry_without_window_or_pane_ids_caps_at_app_activation() {
        let discovery = discover_verified_process(
            &ProcessIdentity {
                pid: 42,
                started_at_ms: 1_700_000_000_000,
            },
            HostKind::WindowsTerminal,
            "agent.exe",
            "WindowsTerminal.exe",
            None,
        );

        assert_eq!(discovery.host, HostKind::WindowsTerminal);
        assert_eq!(discovery.tier, ContextOpenTier::AppActivate);
    }

    #[test]
    #[cfg(windows)]
    fn verified_wt_metadata_enables_exact_pane_discovery() {
        let discovery = discover_verified_process(
            &ProcessIdentity {
                pid: 42,
                started_at_ms: 1_700_000_000_000,
            },
            HostKind::WindowsTerminal,
            "agent.exe",
            "WindowsTerminal.exe",
            Some(&wt_verified_terminal()),
        );

        assert_eq!(discovery.host, HostKind::WindowsTerminal);
        assert_eq!(discovery.tier, ContextOpenTier::ExactPane);
    }

    #[test]
    #[cfg(windows)]
    fn resolve_session_carries_verified_terminal_into_locator() {
        let mut session = sample_session(
            AgentSource::Generic,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 1_700_000_000_000,
            }),
        );
        session.verified_terminal = Some(wt_verified_terminal());
        let resolved = resolve_session(&session).expect("resolve").expect("some");
        assert_eq!(resolved.locator.verified_terminal(), wt_verified_terminal());
        assert!(
            resolved.pane_verified,
            "expected pane verification for host {:?}",
            resolved.host
        );
        assert_eq!(resolved.locator.verified_terminal(), wt_verified_terminal());
    }

    #[test]
    fn resolve_session_without_verified_terminal_keeps_pane_unverified() {
        let session = sample_session(
            AgentSource::Generic,
            Some(ProcessIdentity {
                pid: 42,
                started_at_ms: 1_700_000_000_000,
            }),
        );
        let resolved = resolve_session(&session).expect("resolve").expect("some");
        assert!(!resolved.pane_verified);
        assert_eq!(
            resolved.locator.verified_terminal(),
            VerifiedTerminalContext::default()
        );
    }

    #[test]
    fn platform_tier_mapping_preserves_order_without_inflation() {
        assert_eq!(
            map_platform_tier(NavigationTier::Unsupported),
            ContextOpenTier::None
        );
        assert_eq!(
            map_platform_tier(NavigationTier::AppActivate),
            ContextOpenTier::AppActivate
        );
        assert_eq!(
            map_platform_tier(NavigationTier::WindowFocus),
            ContextOpenTier::WindowFocus
        );
        assert_eq!(
            map_platform_tier(NavigationTier::ExactPane),
            ContextOpenTier::ExactPane
        );
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
