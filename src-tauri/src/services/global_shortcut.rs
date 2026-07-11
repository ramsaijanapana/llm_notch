//! Global shortcut registration with transactional replacement.

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tracing::{info, warn};

pub const DEFAULT_DASHBOARD_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";

pub trait ShortcutHandler: Send + Sync + 'static {
    fn on_dashboard_shortcut(&self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRegistration {
    pub accelerator: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("invalid accelerator: {0}")]
    InvalidAccelerator(String),
    #[error("shortcut conflict: {0}")]
    Conflict(String),
    #[error("shortcut registration failed: {0}")]
    Registration(String),
}

pub struct GlobalShortcutService<R: Runtime> {
    active: Option<ShortcutRegistration>,
    _handler: Option<Arc<dyn ShortcutHandler>>,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> Default for GlobalShortcutService<R> {
    fn default() -> Self {
        Self {
            active: None,
            _handler: None,
            _runtime: std::marker::PhantomData,
        }
    }
}

impl<R: Runtime> GlobalShortcutService<R> {
    /// Replaces the active shortcut transactionally: unregister old, register new, rollback on failure.
    pub fn replace_registration(
        &mut self,
        app: &AppHandle<R>,
        accelerator: &str,
        handler: Arc<dyn ShortcutHandler>,
    ) -> Result<(), ShortcutError> {
        let parsed = parse_accelerator(accelerator)?;
        let previous = self.active.clone();

        if let Some(previous) = &previous {
            unregister_if_active(app, &previous.accelerator)?;
        }

        let handler_for_callback = Arc::clone(&handler);
        let register_result =
            app.global_shortcut()
                .on_shortcut(parsed, move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        handler_for_callback.on_dashboard_shortcut();
                    }
                });

        if let Err(error) = register_result {
            if let Some(previous) = previous {
                if let Err(rollback) = self.register_parsed(app, &previous.accelerator, handler) {
                    warn!(
                        original = %error,
                        rollback = %rollback,
                        "shortcut replacement failed and rollback also failed"
                    );
                }
            }
            return Err(map_registration_error(accelerator, error));
        }

        self.active = Some(ShortcutRegistration {
            accelerator: accelerator.into(),
        });
        self._handler = Some(handler);
        info!(accelerator, "global shortcut registered");
        Ok(())
    }

    fn register_parsed(
        &mut self,
        app: &AppHandle<R>,
        accelerator: &str,
        handler: Arc<dyn ShortcutHandler>,
    ) -> Result<(), ShortcutError> {
        self.replace_registration(app, accelerator, handler)
    }
}

fn parse_accelerator(accelerator: &str) -> Result<Shortcut, ShortcutError> {
    accelerator
        .parse::<Shortcut>()
        .map_err(|e| ShortcutError::InvalidAccelerator(e.to_string()))
}

fn unregister_if_active<R: Runtime>(
    app: &AppHandle<R>,
    accelerator: &str,
) -> Result<(), ShortcutError> {
    let parsed = parse_accelerator(accelerator)?;
    app.global_shortcut()
        .unregister(parsed)
        .map_err(|e| ShortcutError::Registration(e.to_string()))
}

fn map_registration_error(
    accelerator: &str,
    error: tauri_plugin_global_shortcut::Error,
) -> ShortcutError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("already") {
        ShortcutError::Conflict(format!("{accelerator} is already registered: {message}"))
    } else {
        ShortcutError::Registration(message)
    }
}

/// Thread-safe holder for shortcut registration state used during setup.
pub type SharedShortcutService<R> = Arc<Mutex<GlobalShortcutService<R>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_accelerator_is_documented_combo() {
        assert_eq!(DEFAULT_DASHBOARD_SHORTCUT, "CmdOrCtrl+Shift+Space");
    }

    #[test]
    fn parse_accepts_default_combo() {
        let parsed = parse_accelerator(DEFAULT_DASHBOARD_SHORTCUT).expect("parse");
        assert!(parsed.mods.shift());
        assert_ne!(parsed.mods.bits(), 0);
    }

    #[test]
    fn parse_rejects_empty_string() {
        assert!(parse_accelerator("").is_err());
    }
}
