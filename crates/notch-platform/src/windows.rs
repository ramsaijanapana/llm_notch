use std::path::Path;

use crate::{
    HostActivationBridge, HostBridgeOutcome, NavigationDisposition, NavigationOutcome,
    NavigationTier, ProcessDescriptor, TerminalHost, TerminalLocator, TerminalNavigator,
    VerifiedTerminalMetadata,
};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindow, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

#[derive(Debug, Default)]
pub struct WindowsTerminalNavigator;

/// Executes HWND-based activation without enumerating windows or inspecting titles.
#[derive(Debug, Default)]
pub struct WindowsHostActivationBridge;

impl HostActivationBridge for WindowsHostActivationBridge {
    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome {
        if locator.tier() == NavigationTier::Unsupported {
            return NavigationOutcome::unsupported(locator.explanation());
        }

        if locator.tier() == NavigationTier::ExactPane {
            match try_exact_pane_host_bridge(locator) {
                HostBridgeOutcome::Activated { message } => {
                    return NavigationOutcome {
                        tier: NavigationTier::ExactPane,
                        disposition: NavigationDisposition::Activated,
                        message,
                    };
                }
                HostBridgeOutcome::Unavailable { message } => {
                    if activate_verified_window(locator).is_some_and(|outcome| {
                        outcome.disposition == NavigationDisposition::Activated
                    }) {
                        return NavigationOutcome {
                            tier: NavigationTier::WindowFocus,
                            disposition: NavigationDisposition::Activated,
                            message: format!("{message}; focused the verified host window instead"),
                        };
                    }
                    return activation_failed(locator.tier(), message);
                }
                HostBridgeOutcome::NotApplicable => {}
            }
        }

        activate_verified_window(locator).unwrap_or_else(|| {
            activation_failed(
                locator.tier(),
                "Windows activation requires a verified native window handle",
            )
        })
    }
}

fn activated_tier(discovered: NavigationTier) -> NavigationTier {
    match discovered {
        NavigationTier::ExactPane => NavigationTier::WindowFocus,
        tier => tier,
    }
}

fn activation_failed(tier: NavigationTier, message: impl Into<String>) -> NavigationOutcome {
    NavigationOutcome {
        tier: activated_tier(tier),
        disposition: NavigationDisposition::ActivationFailed,
        message: message.into(),
    }
}

fn activate_verified_window(locator: &TerminalLocator) -> Option<NavigationOutcome> {
    let Some(raw_handle) = locator.verified_metadata().window_handle else {
        return None;
    };
    if raw_handle == 0 || raw_handle > isize::MAX as u64 {
        return Some(activation_failed(
            locator.tier(),
            "the verified window handle is invalid",
        ));
    }

    #[cfg(windows)]
    {
        if focus_verified_window(raw_handle) {
            return Some(NavigationOutcome {
                tier: activated_tier(locator.tier()),
                disposition: NavigationDisposition::Activated,
                message: if locator.tier() == NavigationTier::ExactPane {
                    "Focused the verified host window; exact pane activation requires a terminal-specific bridge"
                        .into()
                } else {
                    "Focused the verified host window".into()
                },
            });
        }

        return Some(activation_failed(
            locator.tier(),
            "Win32 rejected foreground activation for the verified window handle",
        ));
    }

    #[cfg(not(windows))]
    {
        let _ = raw_handle;
        Some(NavigationOutcome {
            tier: NavigationTier::Unsupported,
            disposition: NavigationDisposition::RequiresPlatformImplementation,
            message: "the Windows activation bridge can only execute on Windows".into(),
        })
    }
}

/// Attempts a terminal-host-specific exact-pane bridge before HWND fallback.
pub fn try_exact_pane_host_bridge(locator: &TerminalLocator) -> HostBridgeOutcome {
    if locator.tier() != NavigationTier::ExactPane {
        return HostBridgeOutcome::NotApplicable;
    }

    match locator.host() {
        TerminalHost::WindowsTerminal => {
            activate_windows_terminal_exact_pane(locator.verified_metadata(), spawn_wt_command)
        }
        TerminalHost::Other(name) if name == "conemu" => HostBridgeOutcome::Unavailable {
            message: "ConEmu exact-pane activation is not implemented; wt.exe-style session targeting is unavailable for this host"
                .into(),
        },
        _ => HostBridgeOutcome::NotApplicable,
    }
}

/// Builds the `wt.exe` argument list for verified tab/pane indices.
///
/// Returns an error when metadata uses identifiers that the Windows Terminal CLI
/// cannot target (for example `WT_SESSION` GUIDs).
pub fn build_wt_exact_pane_command(
    metadata: &VerifiedTerminalMetadata,
) -> Result<Vec<String>, String> {
    let tab_id = metadata.tab_id.as_deref().ok_or_else(|| {
        "tab_id is required for Windows Terminal exact-pane activation".to_string()
    })?;
    let pane_id = metadata.pane_id.as_deref().ok_or_else(|| {
        "pane_id is required for Windows Terminal exact-pane activation".to_string()
    })?;
    let tab_index = parse_wt_index(tab_id)
        .ok_or_else(|| format!("tab_id `{tab_id}` is not a Windows Terminal tab index"))?;
    let pane_index = parse_wt_index(pane_id)
        .ok_or_else(|| format!("pane_id `{pane_id}` is not a Windows Terminal pane index"))?;

    let mut args = Vec::new();
    if let Some(session_id) = metadata.terminal_session_id.as_deref() {
        match parse_wt_window_target(session_id) {
            WtWindowTarget::Last => {
                args.push("-w".into());
                args.push("0".into());
            }
            WtWindowTarget::Index(index) => {
                args.push("-w".into());
                args.push(index.to_string());
            }
            WtWindowTarget::Name(name) => {
                args.push("-w".into());
                args.push(name);
            }
            WtWindowTarget::UnsupportedSessionGuid => {
                return Err(
                    "terminal_session_id looks like a WT_SESSION GUID; wt.exe cannot focus tabs by session GUID yet"
                        .into(),
                );
            }
        }
    }

    args.push("focus-tab".into());
    args.push("-t".into());
    args.push(tab_index.to_string());
    args.push(";".into());
    args.push("focus-pane".into());
    args.push("-t".into());
    args.push(pane_index.to_string());
    Ok(args)
}

fn activate_windows_terminal_exact_pane(
    metadata: &VerifiedTerminalMetadata,
    spawn: impl FnOnce(&[String]) -> bool,
) -> HostBridgeOutcome {
    match build_wt_exact_pane_command(metadata) {
        Ok(args) => {
            if spawn(&args) {
                HostBridgeOutcome::Activated {
                    message: "Activated Windows Terminal tab and pane via wt.exe".into(),
                }
            } else {
                HostBridgeOutcome::Unavailable {
                    message: "wt.exe is unavailable or rejected the exact-pane command".into(),
                }
            }
        }
        Err(reason) => HostBridgeOutcome::Unavailable { message: reason },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WtWindowTarget {
    Last,
    Index(i32),
    Name(String),
    UnsupportedSessionGuid,
}

fn parse_wt_index(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    trimmed.parse().ok()
}

fn parse_wt_window_target(session_id: &str) -> WtWindowTarget {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return WtWindowTarget::Last;
    }
    if trimmed.eq_ignore_ascii_case("last") || trimmed == "0" {
        return WtWindowTarget::Last;
    }
    if looks_like_session_guid(trimmed) {
        return WtWindowTarget::UnsupportedSessionGuid;
    }
    if let Ok(index) = trimmed.parse::<i32>() {
        return WtWindowTarget::Index(index);
    }
    WtWindowTarget::Name(trimmed.to_string())
}

fn looks_like_session_guid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && value
            .chars()
            .filter(|ch| *ch != '-')
            .all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(windows)]
fn spawn_wt_command(args: &[String]) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    Command::new("wt")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
}

#[cfg(not(windows))]
fn spawn_wt_command(_args: &[String]) -> bool {
    false
}

#[cfg(windows)]
fn focus_verified_window(raw_handle: u64) -> bool {
    // SAFETY: the numeric value is never dereferenced by Rust. `IsWindow`
    // validates it before the same HWND is passed to user32. The handle comes
    // from the verified metadata boundary and zero/out-of-range values were
    // rejected by the caller. The window may still disappear concurrently;
    // user32 reports that as activation failure.
    unsafe {
        let hwnd = HWND(raw_handle as usize as *mut core::ffi::c_void);
        if !IsWindow(Some(hwnd)).as_bool() {
            return false;
        }
        let _ = ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd).as_bool()
    }
}

impl TerminalNavigator for WindowsTerminalNavigator {
    fn discover(&self, process: &ProcessDescriptor) -> TerminalLocator {
        let host = process
            .terminal_executable
            .as_deref()
            .map(classify_windows_host)
            .filter(|host| *host != TerminalHost::Unknown)
            .or_else(|| {
                process
                    .parent_executable
                    .as_deref()
                    .map(classify_windows_host)
                    .filter(|host| *host != TerminalHost::Unknown)
            })
            .unwrap_or_else(|| classify_windows_host(&process.executable));

        let (tier, explanation) = resolve_tier(&host, &process.metadata);
        TerminalLocator::resolved(process, host, tier, explanation)
    }

    fn activate(&self, locator: &TerminalLocator) -> NavigationOutcome {
        if locator.tier() == NavigationTier::Unsupported {
            return NavigationOutcome::unsupported(locator.explanation());
        }

        if locator.tier() == NavigationTier::ExactPane {
            return match try_exact_pane_host_bridge(locator) {
                HostBridgeOutcome::Activated { message } => NavigationOutcome {
                    tier: NavigationTier::ExactPane,
                    disposition: NavigationDisposition::Activated,
                    message,
                },
                HostBridgeOutcome::Unavailable { message } => NavigationOutcome {
                    tier: NavigationTier::WindowFocus,
                    disposition: NavigationDisposition::RequiresHostBridge,
                    message,
                },
                HostBridgeOutcome::NotApplicable => NavigationOutcome {
                    tier: locator.tier(),
                    disposition: NavigationDisposition::RequiresHostBridge,
                    message: format!(
                        "{:?} exact-pane metadata resolved but no host bridge is registered",
                        locator.host()
                    ),
                },
            };
        }

        NavigationOutcome {
            tier: locator.tier(),
            disposition: NavigationDisposition::RequiresHostBridge,
            message: format!(
                "{:?} metadata resolved; activation requires the platform HWND bridge",
                locator.host()
            ),
        }
    }
}

/// Classifies a verified executable path without consulting mutable window titles.
pub fn classify_windows_host(executable: &str) -> TerminalHost {
    // Windows paths may be classified on Unix CI; normalize separators before basename.
    let normalized = executable.replace("\\", "/");
    let file_name = Path::new(&normalized)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();

    match file_name.as_str() {
        "windowsterminal.exe" | "wt.exe" => TerminalHost::WindowsTerminal,
        "conemu.exe" | "conemu64.exe" | "conemuc.exe" | "conemuc64.exe" => {
            TerminalHost::Other("conemu".into())
        }
        "conhost.exe" | "cmd.exe" => TerminalHost::ConsoleHost,
        "powershell.exe" | "pwsh.exe" => TerminalHost::PowerShell,
        "code.exe" => TerminalHost::VsCode,
        "cursor.exe" => TerminalHost::Cursor,
        "wezterm.exe" | "wezterm-gui.exe" => TerminalHost::WezTerm,
        "wsl.exe" | "wslhost.exe" => TerminalHost::Wsl,
        "tmux" | "tmux.exe" => TerminalHost::Tmux,
        _ => TerminalHost::Unknown,
    }
}

fn resolve_tier(
    host: &TerminalHost,
    metadata: &VerifiedTerminalMetadata,
) -> (NavigationTier, &'static str) {
    let exact_pane = match host {
        TerminalHost::WindowsTerminal | TerminalHost::VsCode | TerminalHost::Cursor => {
            metadata.terminal_session_id.is_some()
                && metadata.tab_id.is_some()
                && metadata.pane_id.is_some()
        }
        TerminalHost::Other(name) if name == "conemu" => {
            metadata.terminal_session_id.is_some()
                && metadata.tab_id.is_some()
                && metadata.pane_id.is_some()
        }
        TerminalHost::WezTerm => metadata.pane_id.is_some(),
        TerminalHost::Wsl | TerminalHost::Tmux => {
            metadata.wsl_distribution.is_some()
                && metadata.tmux_session.is_some()
                && metadata.pane_id.is_some()
        }
        _ => false,
    };

    if exact_pane {
        return (
            NavigationTier::ExactPane,
            "verified host-specific session and pane metadata is available",
        );
    }
    if metadata.window_handle.is_some() {
        return (
            NavigationTier::WindowFocus,
            "a verified native window handle is available",
        );
    }
    if metadata.application_id.is_some() {
        return (
            NavigationTier::AppActivate,
            "a verified application identity is available",
        );
    }

    (
        NavigationTier::Unsupported,
        "no verified application, window, or pane navigation metadata is available",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(terminal: &str, metadata: VerifiedTerminalMetadata) -> ProcessDescriptor {
        ProcessDescriptor {
            process_id: 42,
            process_started_at_ms: Some(100),
            executable: "agent.exe".into(),
            parent_executable: None,
            terminal_executable: Some(terminal.into()),
            metadata,
        }
    }

    #[test]
    fn windows_terminal_requires_complete_verified_route_for_exact_pane() {
        let locator = WindowsTerminalNavigator.discover(&process(
            r"C:\Program Files\WindowsApps\WindowsTerminal.exe",
            VerifiedTerminalMetadata {
                terminal_session_id: Some("0".into()),
                tab_id: Some("2".into()),
                pane_id: Some("3".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.host(), &TerminalHost::WindowsTerminal);
        assert_eq!(locator.tier(), NavigationTier::ExactPane);
        let outcome = WindowsTerminalNavigator.activate(&locator);
        match outcome.disposition {
            NavigationDisposition::Activated => {
                assert_eq!(outcome.tier, NavigationTier::ExactPane);
            }
            NavigationDisposition::RequiresHostBridge => {
                assert!(outcome.message.contains("wt.exe"));
            }
            other => panic!("unexpected disposition: {other:?}"),
        }
    }

    #[test]
    fn vscode_and_cursor_use_verified_window_or_application_metadata() {
        let vscode = WindowsTerminalNavigator.discover(&process(
            "Code.exe",
            VerifiedTerminalMetadata {
                window_handle: Some(123),
                ..Default::default()
            },
        ));
        let cursor = WindowsTerminalNavigator.discover(&process(
            "Cursor.exe",
            VerifiedTerminalMetadata {
                application_id: Some("cursor".into()),
                ..Default::default()
            },
        ));

        assert_eq!(vscode.host(), &TerminalHost::VsCode);
        assert_eq!(vscode.tier(), NavigationTier::WindowFocus);
        assert_eq!(cursor.host(), &TerminalHost::Cursor);
        assert_eq!(cursor.tier(), NavigationTier::AppActivate);
    }

    #[test]
    fn wezterm_global_pane_id_resolves_exact_pane() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "wezterm-gui.exe",
            VerifiedTerminalMetadata {
                pane_id: Some("8".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.host(), &TerminalHost::WezTerm);
        assert_eq!(locator.tier(), NavigationTier::ExactPane);
    }

    #[test]
    fn tmux_over_wsl_requires_all_verified_route_components() {
        let complete = WindowsTerminalNavigator.discover(&process(
            "wsl.exe",
            VerifiedTerminalMetadata {
                wsl_distribution: Some("Ubuntu".into()),
                tmux_session: Some("work".into()),
                pane_id: Some("%4".into()),
                ..Default::default()
            },
        ));
        let incomplete = WindowsTerminalNavigator.discover(&process(
            "tmux",
            VerifiedTerminalMetadata {
                tmux_session: Some("work".into()),
                pane_id: Some("%4".into()),
                ..Default::default()
            },
        ));

        assert_eq!(complete.host(), &TerminalHost::Wsl);
        assert_eq!(complete.tier(), NavigationTier::ExactPane);
        assert_eq!(incomplete.host(), &TerminalHost::Tmux);
        assert_eq!(incomplete.tier(), NavigationTier::Unsupported);
    }

    #[test]
    fn unknown_host_does_not_infer_navigation_from_executable_name() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "mystery-terminal.exe",
            VerifiedTerminalMetadata::default(),
        ));

        assert_eq!(locator.host(), &TerminalHost::Unknown);
        assert_eq!(locator.tier(), NavigationTier::Unsupported);
        assert_eq!(
            WindowsTerminalNavigator.activate(&locator).disposition,
            NavigationDisposition::Unsupported
        );
    }

    #[test]
    fn activation_requires_a_verified_window_handle() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "Cursor.exe",
            VerifiedTerminalMetadata {
                application_id: Some("cursor".into()),
                ..Default::default()
            },
        ));
        let outcome = WindowsHostActivationBridge.activate(&locator);

        assert_eq!(outcome.tier, NavigationTier::AppActivate);
        assert_eq!(outcome.disposition, NavigationDisposition::ActivationFailed);
        assert!(outcome.message.contains("verified native window handle"));
    }

    #[test]
    fn invalid_window_handle_reports_actual_failure() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "Cursor.exe",
            VerifiedTerminalMetadata {
                window_handle: Some(u64::MAX),
                ..Default::default()
            },
        ));
        let outcome = WindowsHostActivationBridge.activate(&locator);

        assert_eq!(outcome.tier, NavigationTier::WindowFocus);
        assert_eq!(outcome.disposition, NavigationDisposition::ActivationFailed);
        assert!(outcome.message.contains("invalid"));
    }

    #[test]
    fn hwnd_activation_caps_exact_pane_without_host_bridge() {
        assert_eq!(
            activated_tier(NavigationTier::ExactPane),
            NavigationTier::WindowFocus
        );
        assert_eq!(
            activated_tier(NavigationTier::AppActivate),
            NavigationTier::AppActivate
        );
    }

    #[test]
    fn build_wt_exact_pane_command_uses_verified_indices() {
        let args = build_wt_exact_pane_command(&VerifiedTerminalMetadata {
            terminal_session_id: Some("2".into()),
            tab_id: Some("1".into()),
            pane_id: Some("0".into()),
            ..Default::default()
        })
        .expect("command");

        assert_eq!(
            args,
            vec![
                "-w".to_string(),
                "2".to_string(),
                "focus-tab".to_string(),
                "-t".to_string(),
                "1".to_string(),
                ";".to_string(),
                "focus-pane".to_string(),
                "-t".to_string(),
                "0".to_string(),
            ]
        );
    }

    #[test]
    fn build_wt_exact_pane_command_rejects_session_guids() {
        let error = build_wt_exact_pane_command(&VerifiedTerminalMetadata {
            terminal_session_id: Some("5720ee6d-6474-47b0-88db-fa7e10e60d37".into()),
            tab_id: Some("1".into()),
            pane_id: Some("0".into()),
            ..Default::default()
        })
        .expect_err("guid");

        assert!(error.contains("WT_SESSION GUID"));
    }

    #[test]
    fn windows_terminal_exact_pane_bridge_uses_injected_spawner() {
        let outcome = activate_windows_terminal_exact_pane(
            &VerifiedTerminalMetadata {
                tab_id: Some("1".into()),
                pane_id: Some("0".into()),
                ..Default::default()
            },
            |args| {
                assert_eq!(
                    args,
                    &[
                        "focus-tab".to_string(),
                        "-t".to_string(),
                        "1".to_string(),
                        ";".to_string(),
                        "focus-pane".to_string(),
                        "-t".to_string(),
                        "0".to_string(),
                    ]
                );
                true
            },
        );

        assert_eq!(
            outcome,
            HostBridgeOutcome::Activated {
                message: "Activated Windows Terminal tab and pane via wt.exe".into(),
            }
        );
    }

    #[test]
    fn conemu_exact_pane_bridge_is_honest_when_unavailable() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "ConEmu64.exe",
            VerifiedTerminalMetadata {
                terminal_session_id: Some("1".into()),
                tab_id: Some("0".into()),
                pane_id: Some("0".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.host(), &TerminalHost::Other("conemu".into()));
        assert_eq!(
            try_exact_pane_host_bridge(&locator),
            HostBridgeOutcome::Unavailable {
                message: "ConEmu exact-pane activation is not implemented; wt.exe-style session targeting is unavailable for this host".into(),
            }
        );
    }

    #[test]
    fn host_bridge_reports_guid_unavailability_without_hwnd() {
        let locator = WindowsTerminalNavigator.discover(&process(
            "WindowsTerminal.exe",
            VerifiedTerminalMetadata {
                terminal_session_id: Some("5720ee6d-6474-47b0-88db-fa7e10e60d37".into()),
                tab_id: Some("1".into()),
                pane_id: Some("0".into()),
                ..Default::default()
            },
        ));

        assert_eq!(
            try_exact_pane_host_bridge(&locator),
            HostBridgeOutcome::Unavailable {
                message: "terminal_session_id looks like a WT_SESSION GUID; wt.exe cannot focus tabs by session GUID yet".into(),
            }
        );

        let outcome = WindowsHostActivationBridge.activate(&locator);
        assert_eq!(outcome.disposition, NavigationDisposition::ActivationFailed);
        assert!(outcome.message.contains("WT_SESSION GUID"));
    }
}
