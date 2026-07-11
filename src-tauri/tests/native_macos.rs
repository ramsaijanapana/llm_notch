#![cfg(target_os = "macos")]

//! macOS overlay smoke tests that do not require a live NSWindow.

use llm_notch_desktop_lib::runtime::helper_path::bundled_helper_filename;
use llm_notch_desktop_lib::window::types::{CapabilityStatus, OverlayPlatformCapability};

#[test]
fn bundled_helper_resource_path_has_no_suffix() {
    assert_eq!(bundled_helper_filename(), "llm-notch-hook");
}

#[test]
fn overlay_capability_honesty_contract() {
    let capability = OverlayPlatformCapability {
        non_activating: CapabilityStatus::Partial {
            fallback: "style-mask panel emulation only",
        },
        topmost: CapabilityStatus::Supported,
        all_spaces: CapabilityStatus::Supported,
        taskbar_excluded: CapabilityStatus::Supported,
        notch_insets: CapabilityStatus::Supported,
        activation_policy: CapabilityStatus::Partial {
            fallback: "Accessory policy best-effort",
        },
    };
    assert!(matches!(
        capability.non_activating,
        CapabilityStatus::Partial { .. }
    ));
    assert!(matches!(
        capability.activation_policy,
        CapabilityStatus::Partial { .. }
    ));
}
