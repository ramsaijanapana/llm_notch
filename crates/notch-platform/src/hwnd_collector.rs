//! Verified HWND discovery for Windows terminal hosts.
//!
//! Discovers a native window handle only when Win32 confirms the HWND belongs to a
//! visible top-level window owned by a classified terminal host in the current
//! process tree. Never parses titles or invents handles.

use crate::windows::classify_windows_host;
use crate::{TerminalHost, ENV_WINDOW_HANDLE};

/// Maximum parent hops when walking the process tree for a terminal host window.
pub const MAX_PARENT_WALK_DEPTH: usize = 32;

/// Returns the first visible top-level HWND for `pid`, validated with `IsWindow`.
#[cfg(windows)]
pub fn hwnd_for_pid(pid: u32) -> Option<u64> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;

    let mut state: (u32, Option<HWND>) = (pid, None);
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
    state
        .1
        .map(|hwnd: HWND| hwnd.0 as u64)
        .filter(|handle| validate_window_handle(*handle))
}

#[cfg(not(windows))]
pub fn hwnd_for_pid(_pid: u32) -> Option<u64> {
    None
}

/// Walks the process tree from `start_pid` and returns a verified terminal HWND.
pub fn discover_terminal_window_handle_from_pid(start_pid: u32) -> Option<u64> {
    discover_terminal_window_handle_with(
        start_pid,
        parent_pid_for_current_platform,
        executable_for_current_platform,
        hwnd_for_pid,
    )
}

/// Walks the process tree from the current process and returns a verified terminal HWND.
pub fn discover_terminal_window_handle() -> Option<u64> {
    discover_terminal_window_handle_from_pid(std::process::id())
}

/// Injectable discovery for unit tests and custom collectors.
pub fn discover_terminal_window_handle_with(
    start_pid: u32,
    resolve_parent: impl Fn(u32) -> Option<u32>,
    resolve_executable: impl Fn(u32) -> Option<String>,
    resolve_hwnd: impl Fn(u32) -> Option<u64>,
) -> Option<u64> {
    let mut current = start_pid;
    for _ in 0..MAX_PARENT_WALK_DEPTH {
        if let Some(executable) = resolve_executable(current) {
            let host = classify_windows_host(&executable);
            if host_owns_top_level_window(&host) {
                if let Some(handle) = resolve_hwnd(current) {
                    return Some(handle);
                }
            }
        }
        current = resolve_parent(current)?;
    }
    None
}

/// Sets `LLM_NOTCH_WINDOW_HANDLE` when discovery succeeds and the env var is unset.
pub fn export_discovered_window_handle_to_env() -> Option<u64> {
    if std::env::var(ENV_WINDOW_HANDLE)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return parse_window_handle(std::env::var(ENV_WINDOW_HANDLE).ok().as_deref());
    }
    let handle = discover_terminal_window_handle()?;
    // SAFETY: single-threaded test/collector usage; env mutation is intentional here.
    unsafe {
        std::env::set_var(ENV_WINDOW_HANDLE, handle.to_string());
    }
    Some(handle)
}

pub fn parse_window_handle(value: Option<&str>) -> Option<u64> {
    let raw = value?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .parse::<u64>()
        .ok()
        .filter(|handle| validate_window_handle(*handle))
}

pub fn validate_window_handle(raw_handle: u64) -> bool {
    if raw_handle == 0 || raw_handle > isize::MAX as u64 {
        return false;
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;
        // SAFETY: handle is range-checked; `IsWindow` validates the HWND.
        let hwnd = HWND(raw_handle as usize as *mut core::ffi::c_void);
        return unsafe { IsWindow(Some(hwnd)).as_bool() };
    }
    #[cfg(not(windows))]
    {
        let _ = raw_handle;
        false
    }
}

fn host_owns_top_level_window(host: &TerminalHost) -> bool {
    matches!(
        host,
        TerminalHost::WindowsTerminal
            | TerminalHost::VsCode
            | TerminalHost::Cursor
            | TerminalHost::WezTerm
            | TerminalHost::Other(_)
    )
}

#[cfg(windows)]
fn parent_pid_for_current_platform(pid: u32) -> Option<u32> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()? };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = false;
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID == pid {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    if !found {
        return None;
    }
    let parent = entry.th32ParentProcessID;
    if parent == 0 || parent == pid {
        None
    } else {
        Some(parent)
    }
}

#[cfg(not(windows))]
fn parent_pid_for_current_platform(_pid: u32) -> Option<u32> {
    None
}

#[cfg(windows)]
fn executable_for_current_platform(pid: u32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()? };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID == pid {
                    let len = entry
                        .szExeFile
                        .iter()
                        .position(|ch| *ch == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let name = OsString::from_wide(&entry.szExeFile[..len]);
                    return name.into_string().ok();
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn executable_for_current_platform(_pid: u32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(parents: &[(u32, u32, &str)]) -> (impl Fn(u32) -> Option<u32>, impl Fn(u32) -> Option<String>, impl Fn(u32) -> Option<u64>) {
        let parent_map: std::collections::HashMap<u32, u32> = parents
            .iter()
            .map(|(pid, parent, _)| (*pid, *parent))
            .collect();
        let exe_map: std::collections::HashMap<u32, String> = parents
            .iter()
            .map(|(pid, _, exe)| (*pid, (*exe).to_string()))
            .collect();
        let hwnd_map: std::collections::HashMap<u32, u64> =
            [(42, 100), (7, 200)].into_iter().collect();

        (
            move |pid| parent_map.get(&pid).copied().filter(|parent| *parent != 0),
            move |pid| exe_map.get(&pid).cloned(),
            move |pid| hwnd_map.get(&pid).copied(),
        )
    }

    #[test]
    fn discovery_walks_parents_until_terminal_host_hwnd() {
        let (parent, exe, hwnd) = chain(&[
            (100, 42, "pwsh.exe"),
            (42, 7, "OpenConsole.exe"),
            (7, 0, "WindowsTerminal.exe"),
        ]);
        let handle =
            discover_terminal_window_handle_with(100, parent, exe, hwnd).expect("hwnd");
        assert_eq!(handle, 200);
    }

    #[test]
    fn discovery_skips_non_window_hosts() {
        let (parent, exe, hwnd) = chain(&[
            (100, 42, "pwsh.exe"),
            (42, 7, "conhost.exe"),
            (7, 0, "WindowsTerminal.exe"),
        ]);
        let handle =
            discover_terminal_window_handle_with(100, parent, exe, hwnd).expect("hwnd");
        assert_eq!(handle, 200);
    }

    #[test]
    fn discovery_returns_none_without_terminal_host() {
        let (parent, exe, hwnd) = chain(&[(100, 42, "pwsh.exe"), (42, 0, "conhost.exe")]);
        assert!(discover_terminal_window_handle_with(100, parent, exe, hwnd).is_none());
    }

    #[test]
    fn parse_window_handle_rejects_zero() {
        assert!(parse_window_handle(Some("0")).is_none());
        assert!(parse_window_handle(Some("not-a-number")).is_none());
    }

    #[test]
    fn host_owns_top_level_window_includes_editors_and_wezterm() {
        assert!(host_owns_top_level_window(&TerminalHost::WindowsTerminal));
        assert!(host_owns_top_level_window(&TerminalHost::VsCode));
        assert!(host_owns_top_level_window(&TerminalHost::Cursor));
        assert!(host_owns_top_level_window(&TerminalHost::WezTerm));
        assert!(host_owns_top_level_window(&TerminalHost::Other("conemu".into())));
        assert!(!host_owns_top_level_window(&TerminalHost::PowerShell));
        assert!(!host_owns_top_level_window(&TerminalHost::ConsoleHost));
    }

    #[cfg(windows)]
    #[test]
    fn current_process_discovery_is_best_effort() {
        let _ = discover_terminal_window_handle();
    }
}
