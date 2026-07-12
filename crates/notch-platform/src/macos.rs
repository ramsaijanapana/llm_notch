use std::path::Path;

use crate::{
    HostActivationBridge, HostBridgeOutcome, NavigationDisposition, NavigationOutcome,
    NavigationTier, ProcessDescriptor, TerminalHost, TerminalLocator, TerminalNavigator,
    VerifiedTerminalMetadata,
};

/// macOS terminal discovery from verified process metadata.
#[derive(Debug, Default)]
pub struct MacOsTerminalNavigator;

/// Activates Terminal.app and iTerm2 via `open` / AppleScript; stubs unsupported hosts.
#[derive(Debug, Default)]
pub struct MacOsHostActivationBridge;

impl HostActivationBridge for MacOsHostActivationBridge {
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
                    if let Some(outcome) = activate_macos_application(locator, spawn_open_bundle) {
                        if outcome.disposition == NavigationDisposition::Activated {
                            return NavigationOutcome {
                                tier: NavigationTier::WindowFocus,
                                disposition: NavigationDisposition::Activated,
                                message: format!(
                                    "{message}; activated the host application instead"
                                ),
                            };
                        }
                    }
                    return activation_failed(locator.tier(), message);
                }
                HostBridgeOutcome::NotApplicable => {}
            }
        }

        activate_macos_application(locator, spawn_open_bundle).unwrap_or_else(|| {
            activation_failed(
                locator.tier(),
                unsupported_host_message(locator.host()),
            )
        })
    }
}

impl TerminalNavigator for MacOsTerminalNavigator {
    fn discover(&self, process: &ProcessDescriptor) -> TerminalLocator {
        let host = process
            .terminal_executable
            .as_deref()
            .map(classify_macos_host)
            .filter(|host| *host != TerminalHost::Unknown)
            .or_else(|| {
                process
                    .parent_executable
                    .as_deref()
                    .map(classify_macos_host)
                    .filter(|host| *host != TerminalHost::Unknown)
            })
            .unwrap_or_else(|| classify_macos_host(&process.executable));

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
                "{:?} metadata resolved; activation requires the macOS host bridge",
                locator.host()
            ),
        }
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

fn unsupported_host_message(host: &TerminalHost) -> String {
    match host {
        TerminalHost::Unknown => {
            "macOS activation requires a recognized terminal or editor host".into()
        }
        TerminalHost::Other(name) => format!(
            "{name} host activation is not implemented on macOS; open the session manually"
        ),
        host => format!(
            "{host:?} host activation is not implemented on macOS; open the session manually"
        ),
    }
}

/// Classifies a verified executable path without consulting mutable window titles.
pub fn classify_macos_host(executable: &str) -> TerminalHost {
    let name = Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable)
        .to_ascii_lowercase();

    match name.as_str() {
        "terminal" | "terminal.app" => TerminalHost::MacTerminal,
        "iterm2" | "iterm2.app" => TerminalHost::ITerm2,
        "code" | "code.app" | "visual studio code.app" => TerminalHost::VsCode,
        "cursor" | "cursor.app" => TerminalHost::Cursor,
        _ => TerminalHost::Unknown,
    }
}

fn resolve_tier(
    host: &TerminalHost,
    metadata: &VerifiedTerminalMetadata,
) -> (NavigationTier, &'static str) {
    let exact_pane = match host {
        TerminalHost::MacTerminal | TerminalHost::ITerm2 => {
            metadata.pane_id.is_some()
                && (metadata.tab_id.is_some() || metadata.terminal_session_id.is_some())
        }
        TerminalHost::VsCode | TerminalHost::Cursor => {
            metadata.terminal_session_id.is_some()
                && metadata.tab_id.is_some()
                && metadata.pane_id.is_some()
        }
        _ => false,
    };

    if exact_pane {
        return (
            NavigationTier::ExactPane,
            "verified host-specific tab and pane metadata is available",
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
        "no verified application or pane navigation metadata is available",
    )
}

/// Returns the bundle identifier used by `open -b` for a supported host.
pub fn bundle_id_for_host(host: &TerminalHost) -> Option<&'static str> {
    match host {
        TerminalHost::MacTerminal => Some("com.apple.Terminal"),
        TerminalHost::ITerm2 => Some("com.googlecode.iterm2"),
        TerminalHost::VsCode => Some("com.microsoft.VSCode"),
        TerminalHost::Cursor => Some("com.todesktop.230313mzl4w4u92"),
        _ => None,
    }
}

fn resolve_bundle_id(
    host: &TerminalHost,
    application_id: Option<&str>,
) -> Option<&'static str> {
    if let Some(id) = application_id {
        if id.contains('.') && !id.ends_with(".app") {
            // Already looks like a bundle identifier from a collector.
            return bundle_id_for_host(host);
        }
    }
    bundle_id_for_host(host)
}

fn activate_macos_application(
    locator: &TerminalLocator,
    spawn_open: impl FnOnce(&str) -> bool,
) -> Option<NavigationOutcome> {
    let bundle_id = resolve_bundle_id(
        locator.host(),
        locator.verified_metadata().application_id.as_deref(),
    )?;

    #[cfg(target_os = "macos")]
    {
        if spawn_open(bundle_id) {
            return Some(NavigationOutcome {
                tier: activated_tier(locator.tier()),
                disposition: NavigationDisposition::Activated,
                message: format!("Activated {:?} via bundle identifier", locator.host()),
            });
        }
        return Some(activation_failed(
            locator.tier(),
            format!("open -b rejected activation for {bundle_id}"),
        ));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (bundle_id, spawn_open);
        Some(NavigationOutcome {
            tier: NavigationTier::Unsupported,
            disposition: NavigationDisposition::RequiresPlatformImplementation,
            message: "the macOS activation bridge can only execute on macOS".into(),
        })
    }
}

/// Attempts a terminal-host-specific exact-pane bridge before application fallback.
pub fn try_exact_pane_host_bridge(locator: &TerminalLocator) -> HostBridgeOutcome {
    if locator.tier() != NavigationTier::ExactPane {
        return HostBridgeOutcome::NotApplicable;
    }

    match locator.host() {
        TerminalHost::MacTerminal => {
            activate_mac_terminal_exact_pane(locator.verified_metadata(), spawn_osascript)
        }
        TerminalHost::ITerm2 => {
            activate_iterm2_exact_pane(locator.verified_metadata(), spawn_osascript)
        }
        TerminalHost::VsCode | TerminalHost::Cursor => HostBridgeOutcome::Unavailable {
            message: format!(
                "{:?} exact-pane activation is not implemented on macOS; editor panel selection requires a host-specific bridge",
                locator.host()
            ),
        },
        _ => HostBridgeOutcome::NotApplicable,
    }
}

fn parse_mac_index(field: &str, label: &str) -> Result<u32, String> {
    let trimmed = field.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("{label} `{field}` is not a 1-based tab or pane index"));
    }
    let parsed = trimmed
        .parse::<u32>()
        .map_err(|_| format!("{label} `{field}` is not a valid index"))?;
    if parsed == 0 {
        return Err(format!("{label} `{field}` must be a 1-based index"));
    }
    Ok(parsed)
}

/// Builds AppleScript that activates Terminal.app and selects a tab by index.
///
/// Terminal.app does not expose verified split-pane selection; callers with `pane_id`
/// should report `Unavailable` before invoking this helper.
pub fn build_mac_terminal_tab_script(tab_id: &str) -> Result<String, String> {
    let tab_index = parse_mac_index(tab_id, "tab_id")?;
    Ok(format!(
        r#"tell application "Terminal"
    activate
    set targetWindow to front window
    set selected tab of targetWindow to tab {tab_index} of targetWindow
end tell"#
    ))
}

/// Builds AppleScript that activates iTerm2 and selects a tab and session by index.
pub fn build_iterm2_exact_pane_script(
    metadata: &VerifiedTerminalMetadata,
) -> Result<String, String> {
    let tab_raw = metadata
        .tab_id
        .as_deref()
        .or(metadata.terminal_session_id.as_deref())
        .ok_or_else(|| {
            "tab_id or terminal_session_id is required for iTerm2 exact-pane activation"
                .to_string()
        })?;
    let pane_raw = metadata
        .pane_id
        .as_deref()
        .ok_or_else(|| "pane_id is required for iTerm2 exact-pane activation".to_string())?;

    let tab_index = parse_mac_index(tab_raw, "tab_id")?;
    let pane_index = parse_mac_index(pane_raw, "pane_id")?;

    let window_clause = if let Some(session_id) = metadata.terminal_session_id.as_deref() {
        if metadata.tab_id.is_some() {
            let window_index = parse_mac_index(session_id, "terminal_session_id")?;
            format!("tell window {window_index}")
        } else {
            // session id is being used as the tab route when tab_id is absent.
            String::new()
        }
    } else {
        String::new()
    };

    if window_clause.is_empty() {
        Ok(format!(
            r#"tell application "iTerm"
    activate
    tell current window
        tell tab {tab_index}
            select session {pane_index}
        end tell
    end tell
end tell"#
        ))
    } else {
        Ok(format!(
            r#"tell application "iTerm"
    activate
    {window_clause}
        tell tab {tab_index}
            select session {pane_index}
        end tell
    end tell
end tell"#
        ))
    }
}

fn activate_mac_terminal_exact_pane(
    metadata: &VerifiedTerminalMetadata,
    spawn: impl FnOnce(&str) -> bool,
) -> HostBridgeOutcome {
    if metadata.pane_id.is_some() {
        return HostBridgeOutcome::Unavailable {
            message: "Terminal.app does not expose verified split-pane selection via AppleScript; tab-only activation is unavailable when pane_id is present"
                .into(),
        };
    }

    let Some(tab_id) = metadata.tab_id.as_deref() else {
        return HostBridgeOutcome::Unavailable {
            message: "tab_id is required for Terminal.app tab activation".into(),
        };
    };

    match build_mac_terminal_tab_script(tab_id) {
        Ok(script) => {
            if spawn(&script) {
                HostBridgeOutcome::Activated {
                    message: "Activated Terminal.app and selected the verified tab via AppleScript"
                        .into(),
                }
            } else {
                HostBridgeOutcome::Unavailable {
                    message: "osascript rejected the Terminal.app tab-selection script".into(),
                }
            }
        }
        Err(reason) => HostBridgeOutcome::Unavailable { message: reason },
    }
}

fn activate_iterm2_exact_pane(
    metadata: &VerifiedTerminalMetadata,
    spawn: impl FnOnce(&str) -> bool,
) -> HostBridgeOutcome {
    match build_iterm2_exact_pane_script(metadata) {
        Ok(script) => {
            if spawn(&script) {
                HostBridgeOutcome::Activated {
                    message:
                        "Activated iTerm2 and selected the verified tab/session via AppleScript"
                            .into(),
                }
            } else {
                HostBridgeOutcome::Unavailable {
                    message: "osascript rejected the iTerm2 exact-pane script".into(),
                }
            }
        }
        Err(reason) => HostBridgeOutcome::Unavailable { message: reason },
    }
}

#[cfg(target_os = "macos")]
fn spawn_open_bundle(bundle_id: &str) -> bool {
    use std::process::Command;

    Command::new("open")
        .args(["-b", bundle_id])
        .spawn()
        .is_ok()
}

#[cfg(not(target_os = "macos"))]
fn spawn_open_bundle(_bundle_id: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn spawn_osascript(script: &str) -> bool {
    use std::process::Command;

    Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn spawn_osascript(_script: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(terminal: &str, metadata: VerifiedTerminalMetadata) -> ProcessDescriptor {
        ProcessDescriptor {
            process_id: 7,
            process_started_at_ms: None,
            executable: "agent".into(),
            parent_executable: None,
            terminal_executable: Some(terminal.into()),
            metadata,
        }
    }

    #[test]
    fn macos_navigator_resolves_iterm2_with_application_identity() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "/Applications/iTerm2.app/Contents/MacOS/iTerm2",
            VerifiedTerminalMetadata {
                application_id: Some("com.googlecode.iterm2".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.host(), &TerminalHost::ITerm2);
        assert_eq!(locator.tier(), NavigationTier::AppActivate);
    }

    #[test]
    fn macos_navigator_resolves_exact_pane_when_tab_and_pane_metadata_complete() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "Terminal.app",
            VerifiedTerminalMetadata {
                tab_id: Some("2".into()),
                pane_id: Some("1".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.host(), &TerminalHost::MacTerminal);
        assert_eq!(locator.tier(), NavigationTier::ExactPane);
    }

    #[test]
    fn macos_navigator_exact_pane_requires_pane_metadata() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "iTerm2.app",
            VerifiedTerminalMetadata {
                tab_id: Some("1".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.tier(), NavigationTier::Unsupported);
    }

    #[test]
    fn macos_navigator_activate_defers_to_host_bridge() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "Terminal.app",
            VerifiedTerminalMetadata {
                application_id: Some("com.apple.Terminal".into()),
                ..Default::default()
            },
        ));
        let outcome = MacOsTerminalNavigator.activate(&locator);

        assert_eq!(outcome.disposition, NavigationDisposition::RequiresHostBridge);
        assert!(outcome.message.contains("macOS host bridge"));
    }

    #[test]
    fn terminal_exact_pane_bridge_rejects_pane_metadata_honestly() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "Terminal.app",
            VerifiedTerminalMetadata {
                tab_id: Some("1".into()),
                pane_id: Some("1".into()),
                ..Default::default()
            },
        ));

        assert_eq!(
            try_exact_pane_host_bridge(&locator),
            HostBridgeOutcome::Unavailable {
                message: "Terminal.app does not expose verified split-pane selection via AppleScript; tab-only activation is unavailable when pane_id is present".into(),
            }
        );
    }

    #[test]
    fn build_mac_terminal_tab_script_uses_one_based_indices() {
        let script = build_mac_terminal_tab_script("2").expect("script");
        assert!(script.contains("tab 2 of targetWindow"));
        assert!(script.contains(r#"tell application "Terminal""#));
    }

    #[test]
    fn build_mac_terminal_tab_script_rejects_zero_based_indices() {
        let error = build_mac_terminal_tab_script("0").expect_err("zero");
        assert!(error.contains("1-based"));
    }

    #[test]
    fn build_iterm2_exact_pane_script_targets_tab_and_session() {
        let script = build_iterm2_exact_pane_script(&VerifiedTerminalMetadata {
            tab_id: Some("2".into()),
            pane_id: Some("1".into()),
            ..Default::default()
        })
        .expect("script");

        assert!(script.contains("tell tab 2"));
        assert!(script.contains("select session 1"));
    }

    #[test]
    fn build_iterm2_exact_pane_script_supports_window_and_tab_routes() {
        let script = build_iterm2_exact_pane_script(&VerifiedTerminalMetadata {
            terminal_session_id: Some("1".into()),
            tab_id: Some("2".into()),
            pane_id: Some("3".into()),
            ..Default::default()
        })
        .expect("script");

        assert!(script.contains("tell window 1"));
        assert!(script.contains("tell tab 2"));
        assert!(script.contains("select session 3"));
    }

    #[test]
    fn iterm2_exact_pane_bridge_uses_injected_spawner() {
        let outcome = activate_iterm2_exact_pane(
            &VerifiedTerminalMetadata {
                tab_id: Some("1".into()),
                pane_id: Some("2".into()),
                ..Default::default()
            },
            |script| {
                assert!(script.contains("tell application \"iTerm\""));
                assert!(script.contains("select session 2"));
                true
            },
        );

        assert_eq!(
            outcome,
            HostBridgeOutcome::Activated {
                message: "Activated iTerm2 and selected the verified tab/session via AppleScript"
                    .into(),
            }
        );
    }

    #[test]
    fn unsupported_macos_host_reports_activation_failure() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "WezTerm.app",
            VerifiedTerminalMetadata {
                application_id: Some("com.github.wez.wezterm".into()),
                ..Default::default()
            },
        ));
        let outcome = MacOsHostActivationBridge.activate(&locator);

        assert_eq!(outcome.disposition, NavigationDisposition::ActivationFailed);
        assert!(outcome.message.contains("recognized terminal"));
    }

    #[test]
    fn vscode_exact_pane_bridge_is_honest_when_unavailable() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "Code.app",
            VerifiedTerminalMetadata {
                terminal_session_id: Some("main".into()),
                tab_id: Some("1".into()),
                pane_id: Some("0".into()),
                ..Default::default()
            },
        ));

        assert_eq!(locator.tier(), NavigationTier::ExactPane);
        assert_eq!(
            try_exact_pane_host_bridge(&locator),
            HostBridgeOutcome::Unavailable {
                message: "VsCode exact-pane activation is not implemented on macOS; editor panel selection requires a host-specific bridge".into(),
            }
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_target_reports_platform_gate_for_supported_host() {
        let locator = MacOsTerminalNavigator.discover(&process(
            "Terminal.app",
            VerifiedTerminalMetadata {
                application_id: Some("com.apple.Terminal".into()),
                ..Default::default()
            },
        ));
        let outcome = MacOsHostActivationBridge.activate(&locator);

        assert_eq!(
            outcome.disposition,
            NavigationDisposition::RequiresPlatformImplementation
        );
    }
}
