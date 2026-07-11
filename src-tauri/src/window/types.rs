//! Shared window types used by geometry, coordinator, and platform adapters.

use std::fmt;

/// Physical pixel point in global screen coordinates (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalPoint {
    pub x: i32,
    pub y: i32,
}

/// Physical pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
}

/// Physical pixel rectangle in global screen coordinates (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalRect {
    pub origin: PhysicalPoint,
    pub size: PhysicalSize,
}

impl PhysicalRect {
    pub fn right(&self) -> i32 {
        self.origin.x + self.size.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.origin.y + self.size.height as i32
    }
}

/// Logical pixel size (density-independent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

/// Notch / safe-area insets in physical pixels relative to the display work area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotchInsets {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

/// Snapshot of a display used for pure geometry calculations.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySnapshot {
    pub name: Option<String>,
    pub bounds: PhysicalRect,
    pub work_area: PhysicalRect,
    pub scale_factor: f64,
    pub notch_insets: NotchInsets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayDescriptor {
    pub id: String,
    pub label: String,
    pub primary: bool,
}

/// Overlay presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMode {
    #[default]
    Compact,
    Peek,
}

impl OverlayMode {
    pub const COMPACT_LOGICAL: LogicalSize = LogicalSize {
        width: 360.0,
        height: 44.0,
    };

    pub const PEEK_LOGICAL: LogicalSize = LogicalSize {
        width: 400.0,
        height: 240.0,
    };

    pub fn logical_size(self) -> LogicalSize {
        match self {
            Self::Compact => Self::COMPACT_LOGICAL,
            Self::Peek => Self::PEEK_LOGICAL,
        }
    }

    pub fn physical_size(self, scale_factor: f64) -> PhysicalSize {
        let logical = self.logical_size();
        PhysicalSize {
            width: logical_to_physical(logical.width, scale_factor).max(1) as u32,
            height: logical_to_physical(logical.height, scale_factor).max(1) as u32,
        }
    }
}

/// Dashboard default and minimum sizes (logical pixels).
pub mod dashboard {
    use super::LogicalSize;

    pub const DEFAULT: LogicalSize = LogicalSize {
        width: 900.0,
        height: 640.0,
    };

    pub const MIN: LogicalSize = LogicalSize {
        width: 720.0,
        height: 520.0,
    };
}

/// Whether a native capability is fully available, partially emulated, or unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Supported,
    Partial {
        fallback: &'static str,
    },
    #[allow(dead_code)]
    Unavailable {
        reason: &'static str,
    },
}

impl fmt::Display for CapabilityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supported => write!(f, "supported"),
            Self::Partial { fallback } => write!(f, "partial fallback: {fallback}"),
            Self::Unavailable { reason } => write!(f, "unavailable: {reason}"),
        }
    }
}

/// Platform-specific overlay capability report returned after native configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayPlatformCapability {
    pub non_activating: CapabilityStatus,
    pub topmost: CapabilityStatus,
    pub all_spaces: CapabilityStatus,
    pub taskbar_excluded: CapabilityStatus,
    pub notch_insets: CapabilityStatus,
    pub activation_policy: CapabilityStatus,
}

impl OverlayPlatformCapability {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn stub() -> Self {
        Self {
            non_activating: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
            topmost: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
            all_spaces: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
            taskbar_excluded: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
            notch_insets: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
            activation_policy: CapabilityStatus::Unavailable {
                reason: "platform adapter not compiled",
            },
        }
    }
}

/// Convert logical pixels to physical pixels using a display scale factor.
#[must_use]
pub fn logical_to_physical(value: f64, scale_factor: f64) -> i32 {
    if !value.is_finite() || !scale_factor.is_finite() || scale_factor <= 0.0 {
        return 0;
    }
    (value * scale_factor).round() as i32
}

/// Convert physical pixels to logical pixels using a display scale factor.
#[must_use]
#[cfg(test)]
pub fn physical_to_logical(value: i32, scale_factor: f64) -> f64 {
    if scale_factor <= 0.0 || !scale_factor.is_finite() {
        return 0.0;
    }
    f64::from(value) / scale_factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_mode_logical_sizes_match_spec() {
        assert_eq!(
            OverlayMode::Compact.logical_size(),
            OverlayMode::COMPACT_LOGICAL
        );
        assert_eq!(OverlayMode::Peek.logical_size(), OverlayMode::PEEK_LOGICAL);
    }

    #[test]
    fn overlay_mode_physical_size_scales_with_dpi() {
        let size = OverlayMode::Compact.physical_size(2.0);
        assert_eq!(size.width, 720);
        assert_eq!(size.height, 88);
    }
}
