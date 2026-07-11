#![cfg(target_os = "windows")]

//! Native Windows smoke tests for overlay platform guarantees.

use llm_notch_desktop_lib::runtime::helper_path::{
    bundled_helper_filename, bundled_helper_in_resource_dir,
};
use llm_notch_desktop_lib::window::types::CapabilityStatus;
use llm_notch_desktop_lib::window::windows::{
    ensure_process_per_monitor_dpi_awareness, expected_overlay_ex_styles,
    overlay_mouse_activate_policy, validate_per_monitor_dpi_awareness,
};
use windows::Win32::UI::WindowsAndMessaging::{WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW};

#[test]
fn per_monitor_dpi_awareness_is_active() {
    ensure_process_per_monitor_dpi_awareness();
    let status = validate_per_monitor_dpi_awareness();
    assert!(
        status.acceptable_for_overlay(),
        "expected per-monitor DPI awareness, got {status:?}"
    );
}

#[test]
fn overlay_ex_style_expectations_match_no_activate_toolwindow() {
    let styles = expected_overlay_ex_styles();
    assert!(styles.contains(WS_EX_NOACTIVATE));
    assert!(styles.contains(WS_EX_TOOLWINDOW));
}

#[test]
fn mouse_activate_policy_can_report_supported() {
    let status = overlay_mouse_activate_policy();
    assert!(matches!(
        status,
        CapabilityStatus::Supported | CapabilityStatus::Partial { .. }
    ));
}

#[test]
fn bundled_helper_resource_path_uses_exe_suffix() {
    assert_eq!(bundled_helper_filename(), "llm-notch-hook.exe");
    let path = bundled_helper_in_resource_dir(std::path::Path::new("C:\\app\\resources"));
    assert!(path.ends_with("llm-notch-hook.exe"));
}
