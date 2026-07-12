//! Platform-neutral terminal navigation discovery.
//!
//! Navigation is resolved only from metadata verified by a process or terminal
//! collector. These backends deliberately do not guess from window titles or
//! claim that a window was activated when no host bridge has run.

mod hwnd_collector;
mod macos;
mod stub;
mod types;
mod windows;
mod wt_collector;

pub use hwnd_collector::{
    discover_terminal_window_handle, discover_terminal_window_handle_from_pid,
    export_discovered_window_handle_to_env, hwnd_for_pid, parse_window_handle,
    validate_window_handle,
};
pub use macos::{
    MacOsHostActivationBridge, MacOsTerminalNavigator, build_iterm2_exact_pane_script,
    build_mac_terminal_tab_script, bundle_id_for_host, classify_macos_host,
    try_exact_pane_host_bridge as try_macos_exact_pane_host_bridge,
};
pub use stub::{UnsupportedHostActivationBridge, UnsupportedTerminalNavigator};
pub use types::{
    HostActivationBridge, HostBridgeOutcome, NavigationDisposition, NavigationOutcome,
    NavigationTier, PlatformKind, ProcessDescriptor, TerminalHost, TerminalLocator,
    TerminalNavigator, VerifiedTerminalMetadata,
};
pub use windows::{
    WindowsHostActivationBridge, WindowsTerminalNavigator, build_wt_exact_pane_command,
    classify_windows_host, try_exact_pane_host_bridge,
};
pub use wt_collector::{
    ENV_PANE_ID, ENV_TAB_ID, ENV_TERMINAL_SESSION_ID, ENV_WINDOW_HANDLE, ENV_WT_PROFILE_ID,
    ENV_WT_PROFILE_NAME, ENV_WT_SESSION, WtCollectorOverrides, WtCollectorSnapshot,
    collect_wt_metadata, collect_wt_metadata_from_env, collector_env_exports,
};

/// Builds a navigator for an explicit platform, primarily for dependency injection and tests.
pub fn navigator_for(platform: PlatformKind) -> Box<dyn TerminalNavigator> {
    match platform {
        PlatformKind::Windows => Box::new(WindowsTerminalNavigator),
        PlatformKind::MacOs => Box::new(MacOsTerminalNavigator),
        PlatformKind::Other => Box::new(UnsupportedTerminalNavigator),
    }
}

/// Builds the navigator appropriate for the current compilation target.
pub fn current_navigator() -> Box<dyn TerminalNavigator> {
    #[cfg(target_os = "windows")]
    {
        return navigator_for(PlatformKind::Windows);
    }

    #[cfg(target_os = "macos")]
    {
        return navigator_for(PlatformKind::MacOs);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        navigator_for(PlatformKind::Other)
    }
}

/// Builds the activation bridge appropriate for the current compilation target.
pub fn current_activation_bridge() -> Box<dyn HostActivationBridge> {
    #[cfg(target_os = "windows")]
    {
        return Box::new(WindowsHostActivationBridge);
    }

    #[cfg(target_os = "macos")]
    {
        return Box::new(macos::MacOsHostActivationBridge);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Box::new(stub::UnsupportedHostActivationBridge)
    }
}
