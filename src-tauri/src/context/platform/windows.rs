//! Windows context activation via HWND foreground selection.

use notch_protocol::ContextOpenTier;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    ShowWindow, SW_RESTORE,
};

use crate::context::activate::ActivationOutcome;
use crate::context::locator::{ContextLocator, HostKind};

pub fn activate(locator: &ContextLocator, target_tier: ContextOpenTier) -> ActivationOutcome {
    let host = locator.host();
    let pid = locator.process().map(|identity| identity.pid);

    match target_tier {
        ContextOpenTier::ExactPane => {
            if let Some(hwnd) = pid.and_then(find_main_window_for_pid) {
                if focus_window(hwnd) {
                    return ActivationOutcome {
                        achieved_tier: ContextOpenTier::WindowFocus,
                        activated: true,
                        detail: Some(
                            "Exact pane selection is not verified on Windows; focused the host window instead."
                                .into(),
                        ),
                    };
                }
            }
            activate_app(host, pid, ContextOpenTier::AppActivate)
        }
        ContextOpenTier::WindowFocus => {
            if let Some(hwnd) = pid.and_then(find_main_window_for_pid) {
                if focus_window(hwnd) {
                    return ActivationOutcome {
                        achieved_tier: ContextOpenTier::WindowFocus,
                        activated: true,
                        detail: None,
                    };
                }
            }
            activate_app(host, pid, ContextOpenTier::AppActivate)
        }
        ContextOpenTier::AppActivate => activate_app(host, pid, ContextOpenTier::AppActivate),
        ContextOpenTier::None => ActivationOutcome {
            achieved_tier: ContextOpenTier::None,
            activated: false,
            detail: Some("Context navigation tier is none.".into()),
        },
    }
}

fn activate_app(
    host: HostKind,
    pid: Option<u32>,
    fallback_tier: ContextOpenTier,
) -> ActivationOutcome {
    if let Some(hwnd) = pid.and_then(find_main_window_for_pid) {
        if focus_window(hwnd) {
            return ActivationOutcome {
                achieved_tier: fallback_tier,
                activated: true,
                detail: host_specific_detail(host),
            };
        }
    }
    ActivationOutcome {
        achieved_tier: ContextOpenTier::None,
        activated: false,
        detail: Some(format!(
            "Could not foreground {host:?}; switch to the agent window manually."
        )),
    }
}

fn host_specific_detail(host: HostKind) -> Option<String> {
    match host {
        HostKind::WindowsTerminal => {
            Some("Focused Windows Terminal window (tab selection is best-effort).".into())
        }
        HostKind::VsCode | HostKind::Cursor => {
            Some("Activated editor application window.".into())
        }
        _ => None,
    }
}

fn find_main_window_for_pid(pid: u32) -> Option<HWND> {
    let mut state = (pid, None);
    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut (u32, Option<HWND>)) };
        let mut window_pid = 0u32;
        unsafe {
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        }
        if window_pid == state.0 && unsafe { IsWindowVisible(hwnd).as_bool() } {
            state.1 = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }
    unsafe {
        let _ = EnumWindows(
            Some(enum_window),
            LPARAM((&mut state as *mut (u32, Option<HWND>)).addr() as isize),
        );
    }
    state.1
}

fn focus_window(hwnd: HWND) -> bool {
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
    use notch_protocol::{ContextOpenTier, ProcessIdentity};

    #[test]
    fn finds_window_for_current_pid() {
        let hwnd = find_main_window_for_pid(std::process::id());
        let _ = hwnd;
    }

    #[test]
    fn windows_activation_never_claims_exact_pane() {
        let locator = ContextLocator::encode(
            HostKind::WindowsTerminal,
            Some(ProcessIdentity {
                pid: std::process::id(),
                started_at_ms: 0,
            }),
            None,
        )
        .expect("encode");
        let outcome = activate(&locator, ContextOpenTier::ExactPane);
        assert_ne!(outcome.achieved_tier, ContextOpenTier::ExactPane);
    }
}
