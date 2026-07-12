use crate::services::remote::{
    RemoteBackendStatus, RemoteConnectionStatusView, RemoteDeploymentPlanView,
    RemoteDeploymentResultView, RemoteHostConfigInput, RemoteHostView, SharedRemoteRegistry,
};
use tauri::State;

/// Lists configured SSH remote hosts from persisted storage.
#[tauri::command]
pub fn list_remote_hosts(registry: State<'_, SharedRemoteRegistry>) -> Vec<RemoteHostView> {
    registry.lock().list_hosts()
}

/// Adds or updates a persisted SSH remote host.
#[tauri::command]
pub fn upsert_remote_host(
    registry: State<'_, SharedRemoteRegistry>,
    config: RemoteHostConfigInput,
) -> Result<RemoteHostView, String> {
    registry.lock().upsert_host_input(config)
}

/// Removes a persisted SSH remote host.
#[tauri::command]
pub fn remove_remote_host(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> Result<(), String> {
    registry.lock().remove_host(&host_id)
}

/// Reports whether the SSH relay backend is available in this build.
#[tauri::command]
pub fn get_remote_backend_status(registry: State<'_, SharedRemoteRegistry>) -> RemoteBackendStatus {
    registry.lock().backend_status()
}

/// Previews relay deployment steps for a host. Fails honestly when the backend is unavailable.
#[tauri::command]
pub fn preview_remote_deploy(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> Result<RemoteDeploymentPlanView, String> {
    registry.lock().preview_deploy(&host_id)
}

/// Executes relay deployment for a host. Fails honestly when SSH/SCP or verification fails.
#[tauri::command]
pub fn execute_remote_deploy(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> Result<RemoteDeploymentResultView, String> {
    registry.lock().execute_deploy(&host_id)
}

/// Starts the stdio relay for a host. Fails honestly when the backend is unavailable.
#[tauri::command]
pub fn start_remote_relay(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> Result<RemoteConnectionStatusView, String> {
    registry.lock().start_relay(&host_id)
}

/// Stops the stdio relay for a host. Fails honestly when the backend is unavailable.
#[tauri::command]
pub fn stop_remote_relay(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> Result<RemoteConnectionStatusView, String> {
    registry.lock().stop_relay(&host_id)
}

/// Returns the current relay connection status for a host.
#[tauri::command]
pub fn get_remote_connection_status(
    registry: State<'_, SharedRemoteRegistry>,
    host_id: String,
) -> RemoteConnectionStatusView {
    registry.lock().connection_status(&host_id)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::services::remote::{DesktopRemoteRegistry, RemoteAvailability, RemoteRegistryConfig};

    fn test_registry() -> SharedRemoteRegistry {
        Arc::new(Mutex::new(DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new(
                "ssh",
                "scp",
                PathBuf::from("/definitely/missing/llm-notch-relay"),
            ),
        )))
    }

    #[test]
    fn commands_report_honest_probe_results_without_configured_hosts() {
        let registry = test_registry();
        let status = registry.lock().backend_status();
        assert_eq!(status.availability, RemoteAvailability::Unavailable);
        assert_eq!(status.relay_binary_present, Some(false));
        assert!(registry.lock().list_hosts().is_empty());
        assert!(registry.lock().preview_deploy("lab").is_err());
        assert!(registry.lock().start_relay("lab").is_err());
        assert!(registry.lock().stop_relay("lab").is_err());

        let connection = registry.lock().connection_status("lab");
        assert_eq!(connection.availability, RemoteAvailability::Unavailable);
        assert!(connection.message.as_deref().unwrap().contains("not configured"));
    }
}
