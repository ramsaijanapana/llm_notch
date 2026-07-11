//! Unsupported platform activation stub.

use notch_protocol::ContextOpenTier;

use crate::context::activate::ActivationOutcome;

pub fn unsupported_platform() -> ActivationOutcome {
    ActivationOutcome {
        achieved_tier: ContextOpenTier::None,
        activated: false,
        detail: Some(
            "Context navigation is only supported on macOS and Windows hosts.".into(),
        ),
    }
}
