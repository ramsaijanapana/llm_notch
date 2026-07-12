#[path = "../src/services/remote.rs"]
mod remote_service;

use std::path::PathBuf;
use std::sync::Arc;

use notch_core::SqliteRepository;
use notch_remote::{RemoteHostConfig, SshHostKeyPolicy};
use parking_lot::Mutex;

#[test]
fn remote_backend_probes_without_fabricating_hosts() {
    let mut registry = remote_service::DesktopRemoteRegistry::with_config(
        remote_service::RemoteRegistryConfig::new(
            "ssh",
            "scp",
            std::path::PathBuf::from("/definitely/missing/llm-notch-relay"),
        ),
    );
    let status = registry.backend_status();
    assert_eq!(status.availability, remote_service::RemoteAvailability::Unavailable);
    assert_eq!(status.relay_binary_present, Some(false));
    assert!(status.ssh_executable_present.is_some());
    assert!(registry.list_hosts().is_empty());
    assert!(registry.preview_deploy("dev-box").is_err());
}

#[test]
fn persisted_hosts_survive_registry_restart() {
    let repository = Arc::new(SqliteRepository::in_memory().expect("in-memory sqlite"));
    let config = remote_service::RemoteRegistryConfig::new(
        "ssh",
        "scp",
        PathBuf::from("/definitely/missing/llm-notch-relay"),
    );
    {
        let mut registry = remote_service::DesktopRemoteRegistry::with_config_and_repository(
            config.clone(),
            Some(Arc::clone(&repository)),
        );
        registry
            .upsert_host(RemoteHostConfig {
                id: "dev-box".into(),
                destination: "dev@example.internal".into(),
                port: None,
                identity_file: None,
                known_hosts_file: None,
                host_key_policy: SshHostKeyPolicy::Strict,
                connect_timeout_seconds: 10,
            })
            .expect("upsert host");
        assert_eq!(registry.list_hosts().len(), 1);
    }

    let mut reloaded = remote_service::DesktopRemoteRegistry::with_config_and_repository(
        config,
        Some(repository),
    );
    let hosts = reloaded.list_hosts();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].config.id, "dev-box");
    assert_eq!(
        hosts[0].connection_state,
        remote_service::RemoteConnectionState::Disconnected
    );
}

#[test]
fn remove_host_command_roundtrip() {
    let repository = Arc::new(SqliteRepository::in_memory().expect("in-memory sqlite"));
    let registry: Arc<Mutex<remote_service::DesktopRemoteRegistry>> = Arc::new(Mutex::new(
        remote_service::DesktopRemoteRegistry::with_config_and_repository(
            remote_service::RemoteRegistryConfig::new(
                "ssh",
                "scp",
                PathBuf::from("/definitely/missing/llm-notch-relay"),
            ),
            Some(Arc::clone(&repository)),
        ),
    ));
    registry
        .lock()
        .upsert_host(RemoteHostConfig {
            id: "lab".into(),
            destination: "dev@lab.internal".into(),
            port: None,
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        })
        .expect("upsert host");
    registry.lock().remove_host("lab").expect("remove host");
    assert!(registry.lock().list_hosts().is_empty());

    let mut reloaded = remote_service::DesktopRemoteRegistry::with_config_and_repository(
        remote_service::RemoteRegistryConfig::new(
            "ssh",
            "scp",
            PathBuf::from("/definitely/missing/llm-notch-relay"),
        ),
        Some(repository),
    );
    assert!(reloaded.list_hosts().is_empty());
}

fn relay_binary_path() -> Option<PathBuf> {
    std::env::var("CARGO_BIN_EXE_llm-notch-relay")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

#[test]
fn poll_active_sessions_acknowledges_heartbeat_without_status_churn() {
    let Some(relay_path) = relay_binary_path() else {
        return;
    };
    use notch_remote::RemoteHostConfig;

    let mut registry = remote_service::DesktopRemoteRegistry::with_config(
        remote_service::RemoteRegistryConfig::new("ssh", "scp", relay_path),
    );
    let host = RemoteHostConfig {
        id: "local-relay".into(),
        destination: "dev@example.internal".into(),
        port: None,
        identity_file: None,
        known_hosts_file: None,
        host_key_policy: SshHostKeyPolicy::Strict,
        connect_timeout_seconds: 10,
    };
    registry.register_host(host).unwrap();
    registry
        .start_relay("local-relay")
        .expect("relay start");

    let poll = registry.poll_active_sessions(1_700_000_000_000);
    assert!(
        poll.connection_updates
            .iter()
            .any(|update| update.connection_state == remote_service::RemoteConnectionState::Streaming)
    );
    assert!(poll.session_events.is_empty());
    let second = registry.poll_active_sessions(1_700_000_001_000);
    assert!(second.connection_updates.is_empty());
    assert!(second.session_events.is_empty());
}
