//! Pure cross-platform window geometry helpers.
//!
//! All coordinates use a top-left origin in physical screen space, matching Tauri monitor
//! descriptors. Negative origins are valid for displays positioned left or above the primary
//! monitor.

use super::types::{
    DisplaySnapshot, NotchInsets, OverlayMode, PhysicalPoint, PhysicalRect, PhysicalSize,
};

/// Additional margin below notch insets when positioning the overlay (physical pixels).
pub const DEFAULT_OVERLAY_TOP_MARGIN_PX: i32 = 4;

/// Compute the overlay's top-center position within a display work area.
#[must_use]
pub fn overlay_top_center_position(
    work_area: &PhysicalRect,
    window_size: &PhysicalSize,
    notch_insets: &NotchInsets,
    margin_top: i32,
) -> PhysicalPoint {
    let x = work_area.origin.x + ((work_area.size.width as i32 - window_size.width as i32) / 2);
    let y = work_area.origin.y + notch_insets.top + margin_top;
    clamp_position_to_work_area(PhysicalPoint { x, y }, window_size, work_area, notch_insets)
}

/// Position the overlay using a full display snapshot and overlay mode.
#[must_use]
pub fn overlay_position_for_display(display: &DisplaySnapshot, mode: OverlayMode) -> PhysicalPoint {
    let window_size = mode.physical_size(display.scale_factor);
    overlay_top_center_position(
        &display.work_area,
        &window_size,
        &display.notch_insets,
        DEFAULT_OVERLAY_TOP_MARGIN_PX,
    )
}

/// Clamp a window position so its frame stays inside the work area, respecting notch insets.
#[must_use]
pub fn clamp_position_to_work_area(
    position: PhysicalPoint,
    window_size: &PhysicalSize,
    work_area: &PhysicalRect,
    notch_insets: &NotchInsets,
) -> PhysicalPoint {
    let min_x = work_area.origin.x + notch_insets.left;
    let min_y = work_area.origin.y + notch_insets.top;
    let max_x = work_area.right() - notch_insets.right - window_size.width as i32;
    let max_y = work_area.bottom() - notch_insets.bottom - window_size.height as i32;

    let x = if max_x < min_x {
        min_x
    } else {
        position.x.clamp(min_x, max_x)
    };

    let y = if max_y < min_y {
        min_y
    } else {
        position.y.clamp(min_y, max_y)
    };

    PhysicalPoint { x, y }
}

/// Build a display snapshot from raw monitor inputs.
#[cfg(test)]
#[must_use]
pub fn display_from_inputs(
    name: Option<String>,
    bounds_origin: PhysicalPoint,
    bounds_size: PhysicalSize,
    work_area_origin: PhysicalPoint,
    work_area_size: PhysicalSize,
    scale_factor: f64,
    notch_insets: NotchInsets,
) -> DisplaySnapshot {
    DisplaySnapshot {
        name,
        bounds: PhysicalRect {
            origin: bounds_origin,
            size: bounds_size,
        },
        work_area: PhysicalRect {
            origin: work_area_origin,
            size: work_area_size,
        },
        scale_factor,
        notch_insets,
    }
}

/// Convert a top-left physical position to macOS AppKit bottom-left frame origin.
#[cfg(test)]
#[must_use]
pub fn physical_top_left_to_macos_frame_origin(
    position: PhysicalPoint,
    window_height: u32,
    screen_height: u32,
) -> PhysicalPoint {
    PhysicalPoint {
        x: position.x,
        y: (screen_height as i32) - position.y - window_height as i32,
    }
}

/// Convert macOS AppKit bottom-left frame origin to top-left physical coordinates.
#[cfg(test)]
#[must_use]
pub fn macos_frame_origin_to_physical_top_left(
    frame_origin: PhysicalPoint,
    window_height: u32,
    screen_height: u32,
) -> PhysicalPoint {
    PhysicalPoint {
        x: frame_origin.x,
        y: (screen_height as i32) - frame_origin.y - window_height as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_display(work_origin_y: i32, scale: f64, notch_top: i32) -> DisplaySnapshot {
        display_from_inputs(
            Some("display-1".into()),
            PhysicalPoint {
                x: 0,
                y: work_origin_y,
            },
            PhysicalSize {
                width: 1920,
                height: 1080,
            },
            PhysicalPoint {
                x: 0,
                y: work_origin_y,
            },
            PhysicalSize {
                width: 1920,
                height: 1040,
            },
            scale,
            NotchInsets {
                top: notch_top,
                ..NotchInsets::default()
            },
        )
    }

    #[test]
    fn top_center_is_horizontally_centered_with_notch_margin() {
        let display = sample_display(0, 1.0, 32);
        let pos = overlay_position_for_display(&display, OverlayMode::Compact);
        assert_eq!(pos.x, (1920 - 360) / 2);
        assert_eq!(pos.y, 32 + DEFAULT_OVERLAY_TOP_MARGIN_PX);
    }

    #[test]
    fn negative_monitor_origin_is_preserved() {
        let display = display_from_inputs(
            None,
            PhysicalPoint { x: -1920, y: -1080 },
            PhysicalSize {
                width: 1920,
                height: 1080,
            },
            PhysicalPoint { x: -1920, y: -1080 },
            PhysicalSize {
                width: 1920,
                height: 1040,
            },
            1.0,
            NotchInsets::default(),
        );
        let pos = overlay_position_for_display(&display, OverlayMode::Compact);
        assert_eq!(pos.x, -1920 + (1920 - 360) / 2);
        assert_eq!(pos.y, -1080 + DEFAULT_OVERLAY_TOP_MARGIN_PX);
    }

    #[test]
    fn clamping_keeps_window_inside_work_area_when_wider_than_space() {
        let work = PhysicalRect {
            origin: PhysicalPoint { x: 100, y: 50 },
            size: PhysicalSize {
                width: 200,
                height: 200,
            },
        };
        let window = PhysicalSize {
            width: 400,
            height: 44,
        };
        let clamped = clamp_position_to_work_area(
            PhysicalPoint { x: 0, y: 0 },
            &window,
            &work,
            &NotchInsets::default(),
        );
        assert_eq!(clamped.x, work.origin.x);
        assert_eq!(clamped.y, work.origin.y);
    }

    #[test]
    fn dpi_conversion_rounds_deterministically() {
        assert_eq!(crate::window::types::logical_to_physical(360.0, 1.25), 450);
        assert_eq!(crate::window::types::logical_to_physical(44.0, 1.25), 55);
        assert!(
            (crate::window::types::physical_to_logical(450, 1.25) - 360.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn peek_mode_is_larger_than_compact_at_same_dpi() {
        let display = sample_display(0, 2.0, 0);
        let compact = OverlayMode::Compact.physical_size(display.scale_factor);
        let peek = OverlayMode::Peek.physical_size(display.scale_factor);
        assert!(peek.width > compact.width);
        assert!(peek.height > compact.height);
        assert_eq!(compact.width, 720);
        assert_eq!(compact.height, 88);
        assert_eq!(peek.width, 800);
        assert_eq!(peek.height, 480);
    }

    #[test]
    fn macos_coordinate_conversion_is_reversible() {
        let top_left = PhysicalPoint { x: 780, y: 36 };
        let frame = physical_top_left_to_macos_frame_origin(top_left, 44, 1080);
        let back = macos_frame_origin_to_physical_top_left(frame, 44, 1080);
        assert_eq!(back, top_left);
    }
}
