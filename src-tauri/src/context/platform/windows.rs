//! Windows context activation via notch-platform host bridges with HWND fallback.

use notch_platform::NavigationDisposition;
use notch_protocol::ContextOpenTier;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

use crate::context::activate::ActivationOutcome;
use crate::context::locator::{ContextLocator, HostKind};
use crate::context::platform::bridge::{activate_via_platform_bridge, map_navigation_outcome};
use notch_platform::hwnd_for_pid;

pub fn activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    let bridge_outcome = activate_via_platform_bridge(locator);
    if bridge_outcome.disposition == NavigationDisposition::Activated {
        return map_navigation_outcome(bridge_outcome);
    }

    let bridge_detail = (!bridge_outcome.message.is_empty()).then_some(bridge_outcome.message);
    legacy_activate(locator, target_tier, bridge_detail)
}

fn legacy_activate(
    locator: &ContextLocator,
    target_tier: ContextOpenTier,
    bridge_detail: Option<String>,
) -> ActivationOutcome {
    let host = locator.host();
    let pid = locator.process().map(|identity| identity.pid);

    match target_tier {
        ContextOpenTier::ExactPane => {
            if let Some(raw) = pid.and_then(hwnd_for_pid) {
                if focus_window(raw) {
                    return ActivationOutcome {
                        achieved_tier: ContextOpenTier::WindowFocus,
                        activated: true,
                        detail: Some(merge_detail(
                            bridge_detail,
                            "Exact pane selection is not verified on Windows; focused the host window instead.",
                        )),
                    };
                }
            }
            activate_app(host, pid, ContextOpenTier::AppActivate, bridge_detail)
        }
        ContextOpenTier::WindowFocus => {
            if let Some(raw) = pid.and_then(hwnd_for_pid) {
                if focus_window(raw) {
                    return ActivationOutcome {
                        achieved_tier: ContextOpenTier::WindowFocus,
                        activated: true,
                        detail: bridge_detail,
                    };
                }
            }
            activate_app(host, pid, ContextOpenTier::AppActivate, bridge_detail)
        }
        ContextOpenTier::AppActivate => {
            activate_app(host, pid, ContextOpenTier::AppActivate, bridge_detail)
        }
        ContextOpenTier::None => ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some(merge_detail(
                bridge_detail,
                "Context navigation tier is none.",
            )),
        },
    }
}

fn activate_app(
    host: HostKind,
    pid: Option<u32>,
    fallback_tier: ContextOpenTier,
    bridge_detail: Option<String>,
) -> ActivationOutcome {
    if let Some(raw) = pid.and_then(hwnd_for_pid) {
        if focus_window(raw) {
            return ActivationOutcome {
                achieved_tier: fallback_tier,
                activated: true,
                detail: bridge_detail.or_else(|| host_specific_detail(host)),
            };
        }
    }
    ActivationOutcome {
        achieved_tier: ContextOpenTier::None,
        activated: false,
        detail: Some(merge_detail(
            bridge_detail,
            format!("Could not foreground {host:?}; switch to the agent window manually."),
        )),
    }
}

fn merge_detail(bridge_detail: Option<String>, fallback: impl Into<String>) -> String {
    match bridge_detail {
        Some(detail) if !detail.is_empty() => detail,
        _ => fallback.into(),
    }
}

fn host_specific_detail(host: HostKind) -> Option<String> {
    match host {
        HostKind::WindowsTerminal => Some(
            "Focused Windows Terminal window via HWND fallback (tab selection is best-effort)."
                .into(),
        ),
        HostKind::VsCode | HostKind::Cursor => Some("Activated editor application window.".into()),
        _ => None,
    }
}

fn focus_window(raw_handle: u64) -> bool {
    let hwnd = HWND(raw_handle as usize as *mut core::ffi::c_void);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let foreground = GetForegroundWindow();
        let mut foreground_thread = 0u32;
        let target_thread;
        if !foreground.0.is_null() {
            foreground_thread = GetWindowThreadProcessId(foreground, None);
        }
        target_thread = GetWindowThreadProcessId(hwnd, None);
        let current_thread = GetCurrentThreadId();
        let attached = if foreground_thread != 0 && foreground_thread != target_thread {
            AttachThreadInput(foreground_thread, target_thread, true).as_bool()
                && AttachThreadInput(current_thread, target_thread, true).as_bool()
        } else {
            false
        };
        let focused = SetForegroundWindow(hwnd).as_bool();
        if attached {
            let _ = AttachThreadInput(foreground_thread, target_thread, false);
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
        focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::locator::ContextLocator;
    use crate::context::platform::bridge::terminal_locator_for_activation;
    use notch_platform::{NavigationTier, hwnd_for_pid, try_exact_pane_host_bridge};
    use notch_protocol::{ContextOpenTier, ProcessIdentity};

    #[test]
    fn finds_window_for_current_pid() {
        let hwnd = hwnd_for_pid(std::process::id());
        let _ = hwnd;
    }

    #[test]
    fn windows_activation_never_claims_exact_pane_without_verified_metadata() {
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 0,
            }),
            Some("workspace-label"),
            None,
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert_ne!(terminal.tier(), NavigationTier::ExactPane);
        assert_eq!(
            try_exact_pane_host_bridge(&terminal),
            notch_platform::HostBridgeOutcome::NotApplicable
        );

        let outcome = activate(&locator, ContextOpenTier::ExactPane);
        assert_ne!(outcome.achieved_tier, ContextOpenTier::ExactPane);
    }

    #[test]
    fn platform_bridge_enriches_hwnd_for_attributed_process() {
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 0,
            }),
            None,
            None,
        )
        .expect("encode");
        let terminal = terminal_locator_for_activation(&locator);
        assert!(
            terminal.verified_metadata().window_handle.is_some()
                || terminal.tier() == NavigationTier::AppActivate
        );
    }
}
