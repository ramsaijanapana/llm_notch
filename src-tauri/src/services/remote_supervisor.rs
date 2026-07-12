//! Background poll loop for active SSH relay sessions.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::watch;
use tracing::warn;

use crate::state::HostState;

use super::remote::{
    SharedRemoteRegistry, REMOTE_CONNECTION_CHANGED_EVENT, REMOTE_RELAY_POLL_INTERVAL_MS,
};

/// Periodically polls relay sessions until the desktop host shuts down.
pub async fn run_relay_supervisor(
    app: AppHandle,
    host: Arc<HostState>,
    registry: SharedRemoteRegistry,
    shutdown: &mut watch::Receiver<bool>,
) {
    let mut timer = tokio::time::interval(Duration::from_millis(REMOTE_RELAY_POLL_INTERVAL_MS));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = timer.tick() => {
                let app = app.clone();
                let host = Arc::clone(&host);
                let registry = Arc::clone(&registry);
                let poll_result = match tauri::async_runtime::spawn_blocking(move || {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    registry.lock().poll_active_sessions(now_ms)
                })
                .await
                {
                    Ok(poll_result) => poll_result,
                    Err(error) => {
                        warn!(%error, "remote relay poll task failed");
                        continue;
                    }
                };
                for event in poll_result.session_events {
                    if let Err(error) = host.ingest_relay_session_event(&event) {
                        warn!(
                            %error,
                            host_id = %event.host_id,
                            external_session_id = %event.external_session_id,
                            "relay session event ingest failed"
                        );
                    }
                }
                for status in poll_result.connection_updates {
                    if let Err(error) = app.emit(REMOTE_CONNECTION_CHANGED_EVENT, &status) {
                        warn!(
                            %error,
                            host_id = %status.host_id,
                            "remote connection status emit failed"
                        );
                    }
                }
            }
        }
    }
    tracing::info!("remote relay supervisor stopped");
}
