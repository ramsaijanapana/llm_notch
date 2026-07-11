//! Cross-platform window coordinator for overlay and dashboard surfaces.

use tauri::webview::WebviewWindow;
use tauri::{AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition};

use crate::window::error::{WindowError, WindowResult};
use crate::window::geometry::overlay_position_for_display;
use crate::window::types::{
    DisplayDescriptor, DisplaySnapshot, NotchInsets, OverlayMode, OverlayPlatformCapability,
    PhysicalPoint, PhysicalRect, PhysicalSize, dashboard,
};

#[cfg(target_os = "macos")]
use crate::window::macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::window::stub as platform;
#[cfg(target_os = "windows")]
use crate::window::windows as platform;

const OVERLAY_LABEL: &str = "overlay";
const DASHBOARD_LABEL: &str = "dashboard";

/// High-level API for configuring, positioning, and toggling overlay/dashboard windows.
pub struct WindowCoordinator {
    app: AppHandle,
    overlay_mode: OverlayMode,
    target_monitor: Option<String>,
    show_over_fullscreen: bool,
    overlay_capability: Option<OverlayPlatformCapability>,
}

impl WindowCoordinator {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            overlay_mode: OverlayMode::Compact,
            target_monitor: None,
            show_over_fullscreen: false,
            overlay_capability: None,
        }
    }

    /// Select a monitor by name, or `None` to follow the overlay's current monitor / primary.
    pub fn set_target_monitor(&mut self, name: Option<String>) {
        self.target_monitor = name;
    }

    pub fn set_show_over_fullscreen(&mut self, enabled: bool) -> WindowResult<()> {
        self.show_over_fullscreen = enabled;
        platform::reapply_overlay(&self.overlay_window()?, enabled)
    }

    pub fn available_displays(&self) -> WindowResult<Vec<DisplayDescriptor>> {
        let overlay = self.overlay_window()?;
        let monitors = overlay.available_monitors()?;
        let primary_id = overlay.primary_monitor()?.as_ref().map(monitor_id);
        Ok(monitors
            .iter()
            .enumerate()
            .map(|(index, monitor)| DisplayDescriptor {
                id: monitor_id(monitor),
                label: monitor
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("Display {}", index + 1)),
                primary: primary_id.as_deref() == Some(monitor_id(monitor).as_str()),
            })
            .collect())
    }

    /// Apply native overlay configuration and cache the platform capability report.
    pub fn configure_overlay(&mut self) -> WindowResult<OverlayPlatformCapability> {
        let overlay = self.overlay_window()?;
        let capability = platform::configure_overlay(&overlay, self.show_over_fullscreen)?;
        self.overlay_capability = Some(capability.clone());
        Ok(capability)
    }

    /// Toggle overlay between compact and peek sizes, repositioning on the active display.
    pub fn set_overlay_mode(&mut self, mode: OverlayMode) -> WindowResult<()> {
        self.overlay_mode = mode;
        self.apply_overlay_mode_geometry()?;
        platform::reapply_overlay(&self.overlay_window()?, self.show_over_fullscreen)?;
        Ok(())
    }

    /// Position the overlay at the top-center of the resolved display work area.
    pub fn position_overlay(&self) -> WindowResult<()> {
        let overlay = self.overlay_window()?;
        let display = self.resolve_display_for_window(&overlay)?;
        let position = overlay_position_for_display(&display, self.overlay_mode);
        overlay.set_position(PhysicalPosition::new(position.x, position.y))?;
        Ok(())
    }

    /// Configure native overlay flags and place it on the target display.
    pub fn setup_overlay(&mut self) -> WindowResult<OverlayPlatformCapability> {
        let capability = self.configure_overlay()?;
        self.apply_overlay_mode_geometry()?;
        self.position_overlay()?;
        Ok(capability)
    }

    /// Apply dashboard defaults and show the window.
    pub fn open_dashboard(&self, focus: bool) -> WindowResult<()> {
        let dashboard = self.dashboard_window()?;
        platform::configure_dashboard(&dashboard)?;
        self.ensure_dashboard_bounds(&dashboard)?;
        dashboard.show()?;
        if focus {
            dashboard.set_focus()?;
        }
        Ok(())
    }

    pub fn toggle_dashboard(&self) -> WindowResult<()> {
        let dashboard = self.dashboard_window()?;
        if dashboard.is_visible()? {
            self.hide_dashboard()
        } else {
            self.open_dashboard(true)
        }
    }

    pub fn hide_dashboard(&self) -> WindowResult<()> {
        let dashboard = self.dashboard_window()?;
        dashboard.hide()?;
        platform::on_dashboard_hidden(&self.app, self.show_over_fullscreen)?;
        Ok(())
    }

    pub fn show_overlay(&self) -> WindowResult<()> {
        let overlay = self.overlay_window()?;
        overlay.show()?;
        platform::reapply_overlay(&overlay, self.show_over_fullscreen)?;
        self.position_overlay()
    }

    pub fn hide_overlay(&self) -> WindowResult<()> {
        self.overlay_window()?.hide()?;
        Ok(())
    }

    fn apply_overlay_mode_geometry(&self) -> WindowResult<()> {
        let overlay = self.overlay_window()?;
        let display = self.resolve_display_for_window(&overlay)?;
        let size = self.overlay_mode.physical_size(display.scale_factor);
        overlay.set_size(tauri::PhysicalSize::new(size.width, size.height))?;
        Ok(())
    }

    fn ensure_dashboard_bounds(&self, dashboard: &WebviewWindow) -> WindowResult<()> {
        dashboard.set_min_size(Some(LogicalSize::new(
            dashboard::MIN.width,
            dashboard::MIN.height,
        )))?;
        dashboard.set_size(LogicalSize::new(
            dashboard::DEFAULT.width,
            dashboard::DEFAULT.height,
        ))?;
        if dashboard.outer_position().is_err() {
            dashboard.center()?;
        }
        Ok(())
    }

    fn resolve_display_for_window(&self, window: &WebviewWindow) -> WindowResult<DisplaySnapshot> {
        let monitor = self.resolve_monitor(window)?;
        let mut display = display_from_monitor(&monitor);
        display.notch_insets = platform::notch_insets(window).unwrap_or_default();
        Ok(display)
    }

    fn resolve_monitor(&self, window: &WebviewWindow) -> WindowResult<Monitor> {
        if let Some(id) = &self.target_monitor {
            let monitors = window.available_monitors()?;
            if let Some(found) = monitors
                .into_iter()
                .find(|monitor| monitor_id(monitor) == *id || monitor.name().as_deref() == Some(id))
            {
                return Ok(found);
            }
            return Err(WindowError::MonitorNotFound(id.clone()));
        }

        if let Some(current) = window.current_monitor()? {
            return Ok(current);
        }

        if let Some(primary) = window.primary_monitor()? {
            return Ok(primary);
        }

        window
            .available_monitors()?
            .into_iter()
            .next()
            .ok_or(WindowError::NoMonitor)
    }

    fn overlay_window(&self) -> WindowResult<WebviewWindow> {
        self.app
            .get_webview_window(OVERLAY_LABEL)
            .ok_or(WindowError::WindowNotFound(OVERLAY_LABEL))
    }

    fn dashboard_window(&self) -> WindowResult<WebviewWindow> {
        self.app
            .get_webview_window(DASHBOARD_LABEL)
            .ok_or(WindowError::WindowNotFound(DASHBOARD_LABEL))
    }
}

fn monitor_id(monitor: &Monitor) -> String {
    let position = monitor.position();
    let size = monitor.size();
    format!(
        "{}:{}:{}:{}x{}",
        monitor.name().map(String::as_str).unwrap_or("display"),
        position.x,
        position.y,
        size.width,
        size.height
    )
}

/// Convert a Tauri monitor descriptor into a geometry [`DisplaySnapshot`].
pub fn display_from_monitor(monitor: &Monitor) -> DisplaySnapshot {
    let bounds_pos = monitor.position();
    let bounds_size = monitor.size();
    let work = monitor.work_area();

    DisplaySnapshot {
        name: monitor.name().cloned(),
        bounds: PhysicalRect {
            origin: PhysicalPoint {
                x: bounds_pos.x,
                y: bounds_pos.y,
            },
            size: PhysicalSize {
                width: bounds_size.width,
                height: bounds_size.height,
            },
        },
        work_area: PhysicalRect {
            origin: PhysicalPoint {
                x: work.position.x,
                y: work.position.y,
            },
            size: PhysicalSize {
                width: work.size.width,
                height: work.size.height,
            },
        },
        scale_factor: monitor.scale_factor(),
        notch_insets: NotchInsets::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::geometry::display_from_inputs;

    #[test]
    fn display_from_monitor_maps_work_area() {
        let snapshot = display_from_inputs(
            Some("Built-in".into()),
            PhysicalPoint { x: 0, y: 0 },
            PhysicalSize {
                width: 2560,
                height: 1600,
            },
            PhysicalPoint { x: 0, y: 0 },
            PhysicalSize {
                width: 2560,
                height: 1550,
            },
            2.0,
            NotchInsets {
                top: 64,
                ..NotchInsets::default()
            },
        );

        let pos = overlay_position_for_display(&snapshot, OverlayMode::Peek);
        assert_eq!(pos.x, (2560 - 800) / 2);
        assert_eq!(
            pos.y,
            64 + crate::window::geometry::DEFAULT_OVERLAY_TOP_MARGIN_PX
        );
    }

    #[test]
    fn dashboard_constants_match_spec() {
        assert_eq!(dashboard::DEFAULT.width, 900.0);
        assert_eq!(dashboard::DEFAULT.height, 640.0);
        assert_eq!(dashboard::MIN.width, 720.0);
        assert_eq!(dashboard::MIN.height, 520.0);
    }
}
