use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use notch_core::SqliteRepository;
use notch_remote::{
    ConnectionState, DEFAULT_REMOTE_BIN_DIRECTORY, DEFAULT_REMOTE_RUNTIME_DIRECTORY,
    DeployTransport, DeploymentExecutor, DeploymentPlan, DeploymentStep, OpenSshDeployTransport,
    OpenSshTransport, ReconnectPolicy, RelayArtifactError, RelayFrame, RelayPayload, RelaySession,
    RelaySessionError, RemoteArchitecture, RemoteHostConfig, RemoteOs, RemoteRelayManager,
    RemoteTarget, ResumeCursor, SshHostKeyPolicy, hidden_command, remote_hook_spool_guidance,
    resolve_relay_artifact,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

const NO_HOSTS_MESSAGE: &str =
    "No remote hosts are configured yet. Add an SSH destination below to get started.";

/// Poll interval for the desktop relay supervisor background task.
pub const REMOTE_RELAY_POLL_INTERVAL_MS: u64 = 250;
/// Tauri event emitted when a relay session connection state changes.
pub const REMOTE_CONNECTION_CHANGED_EVENT: &str = "remote-connection-changed";

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionWatchState {
    last_connection_state: RemoteConnectionState,
    reconnect_attempt: u16,
    next_reconnect_at_ms: i64,
    last_connected_at_ms: Option<i64>,
}

impl Default for SessionWatchState {
    fn default() -> Self {
        Self {
            last_connection_state: RemoteConnectionState::Disconnected,
            reconnect_attempt: 0,
            next_reconnect_at_ms: 0,
            last_connected_at_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteHostConfigInput {
    pub id: String,
    pub destination: String,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub host_key_policy: SshHostKeyPolicyView,
    pub connect_timeout_seconds: u16,
}

/// IPC-safe projection for remote host configuration.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHostConfigView {
    pub id: String,
    pub destination: String,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub host_key_policy: SshHostKeyPolicyView,
    pub connect_timeout_seconds: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SshHostKeyPolicyView {
    Strict,
    AcceptNew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Streaming,
    Backoff { attempt: u16, delay_ms: u64 },
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteHostView {
    pub config: RemoteHostConfigView,
    pub availability: RemoteAvailability,
    pub connection_state: RemoteConnectionState,
    pub message: Option<String>,
    pub last_connected_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackendStatus {
    pub availability: RemoteAvailability,
    pub message: Option<String>,
    pub ssh_executable_present: Option<bool>,
    pub relay_binary_present: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
pub enum RemoteDeploymentStepView {
    ProbeTarget,
    CreatePrivateDirectory {
        remote_directory: String,
    },
    UploadTemporary {
        remote_path: String,
    },
    VerifySha256 {
        expected_sha256: String,
    },
    ActivateAtomically {
        remote_path: String,
    },
    StartStdioRelay {
        remote_path: String,
        event_spool_dir: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeploymentPlanView {
    pub host_id: String,
    pub steps: Vec<RemoteDeploymentStepView>,
    pub availability: RemoteAvailability,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTargetView {
    pub os: RemoteOsView,
    pub architecture: RemoteArchitectureView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteOsView {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteArchitectureView {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeploymentResultView {
    pub host_id: String,
    pub availability: RemoteAvailability,
    pub completed_steps: Vec<RemoteDeploymentStepView>,
    pub probed_target: Option<RemoteTargetView>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionStatusView {
    pub host_id: String,
    pub availability: RemoteAvailability,
    pub connection_state: RemoteConnectionState,
    pub message: Option<String>,
}

/// Normalized relay `SessionEvent` frame ready for AppCore ingest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySessionEventIngest {
    pub host_id: String,
    pub external_session_id: String,
    pub source: String,
    pub summary: String,
    pub occurred_at_ms: i64,
    pub kind: Option<notch_protocol::SessionEventKind>,
    pub tool_name: Option<String>,
    pub attention: Option<notch_protocol::AttentionKind>,
}

/// Result of one relay supervisor poll tick.
#[derive(Debug, Default)]
pub struct RemotePollResult {
    pub connection_updates: Vec<RemoteConnectionStatusView>,
    pub session_events: Vec<RelaySessionEventIngest>,
}

#[derive(Debug, Clone)]
pub struct RemoteRegistryConfig {
    pub ssh_executable: String,
    pub scp_executable: String,
    pub relay_binary_path: PathBuf,
    pub relay_binaries_dir: PathBuf,
    pub remote_runtime_dir: String,
}

impl RemoteRegistryConfig {
    pub fn new(
        ssh_executable: impl Into<String>,
        scp_executable: impl Into<String>,
        relay_binary_path: PathBuf,
    ) -> Self {
        Self {
            ssh_executable: ssh_executable.into(),
            scp_executable: scp_executable.into(),
            relay_binary_path,
            relay_binaries_dir: PathBuf::new(),
            remote_runtime_dir: DEFAULT_REMOTE_RUNTIME_DIRECTORY.into(),
        }
    }

    pub fn with_relay_binaries_dir(mut self, relay_binaries_dir: PathBuf) -> Self {
        self.relay_binaries_dir = relay_binaries_dir;
        self
    }
}

pub type SharedRemoteRegistry = Arc<Mutex<DesktopRemoteRegistry>>;

/// Desktop remote registry backed by `notch-remote` relay lifecycle management.
pub struct DesktopRemoteRegistry {
    manager: RemoteRelayManager,
    hosts: HashMap<String, RemoteHostConfig>,
    config: RemoteRegistryConfig,
    repository: Option<Arc<SqliteRepository>>,
    session_watch: HashMap<String, SessionWatchState>,
    cached_ssh_present: Option<bool>,
    cached_scp_present: Option<bool>,
}

#[cfg(test)]
impl DesktopRemoteRegistry {
    pub(crate) fn cached_ssh_probe(&self) -> Option<bool> {
        self.cached_ssh_present
    }

    pub(crate) fn cached_scp_probe(&self) -> Option<bool> {
        self.cached_scp_present
    }
}

impl DesktopRemoteRegistry {
    pub fn with_config(config: RemoteRegistryConfig) -> Self {
        Self::with_config_and_repository(config, None)
    }

    pub fn with_config_and_repository(
        config: RemoteRegistryConfig,
        repository: Option<Arc<SqliteRepository>>,
    ) -> Self {
        let mut registry = Self {
            manager: RemoteRelayManager::new(),
            hosts: HashMap::new(),
            config,
            repository,
            session_watch: HashMap::new(),
            cached_ssh_present: None,
            cached_scp_present: None,
        };
        registry.refresh_executable_probes();
        registry.load_persisted_hosts();
        registry
    }

    fn load_persisted_hosts(&mut self) {
        let Some(repository) = self.repository.as_ref() else {
            return;
        };
        let rows = match repository.load_remote_hosts() {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "failed to load persisted remote hosts");
                return;
            }
        };
        for (id, config_json) in rows {
            match serde_json::from_str::<RemoteHostConfig>(&config_json) {
                Ok(config) if config.id == id => {
                    if config.validate().is_ok() {
                        self.hosts.insert(id, config);
                    } else {
                        tracing::warn!(host_id = %id, "skipping invalid persisted remote host");
                    }
                }
                Ok(_) => tracing::warn!(host_id = %id, "skipping remote host with mismatched id"),
                Err(error) => {
                    tracing::warn!(host_id = %id, %error, "skipping unparsable remote host")
                }
            }
        }
    }

    fn persist_host(&self, config: &RemoteHostConfig) -> Result<(), String> {
        let Some(repository) = self.repository.as_ref() else {
            return Ok(());
        };
        let config_json = serde_json::to_string(config)
            .map_err(|error| format!("host serialization failed: {error}"))?;
        repository
            .upsert_remote_host(
                &config.id,
                &config_json,
                chrono::Utc::now().timestamp_millis(),
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn delete_persisted_host(&self, host_id: &str) -> Result<(), String> {
        let Some(repository) = self.repository.as_ref() else {
            return Ok(());
        };
        repository
            .remove_remote_host(host_id)
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn register_host(&mut self, config: RemoteHostConfig) -> Result<(), String> {
        self.upsert_host(config)
    }

    pub fn upsert_host(&mut self, config: RemoteHostConfig) -> Result<(), String> {
        config.validate().map_err(|error| error.to_string())?;
        self.persist_host(&config)?;
        self.hosts.insert(config.id.clone(), config);
        Ok(())
    }

    pub fn upsert_host_input(
        &mut self,
        input: RemoteHostConfigInput,
    ) -> Result<RemoteHostView, String> {
        let config = host_config_from_input(input)?;
        self.upsert_host(config.clone())?;
        let backend = self.backend_status();
        Ok(host_view(&config, backend.availability, None, None))
    }

    pub fn remove_host(&mut self, host_id: &str) -> Result<(), String> {
        if self.hosts.get(host_id).is_none() {
            return Err(format!("remote host `{host_id}` is not configured"));
        }
        if self.manager.get(host_id).is_some() {
            self.manager.remove(host_id).map_err(map_session_error)?;
        }
        self.session_watch.remove(host_id);
        self.delete_persisted_host(host_id)?;
        self.hosts.remove(host_id);
        Ok(())
    }

    pub fn backend_status(&self) -> RemoteBackendStatus {
        let ssh_present = self
            .cached_ssh_present
            .unwrap_or_else(|| probe_ssh_executable(&self.config.ssh_executable));
        let relay_present = self.config.relay_binary_path.is_file();
        let availability = if ssh_present && relay_present {
            RemoteAvailability::Available
        } else {
            RemoteAvailability::Unavailable
        };
        let message = match availability {
            RemoteAvailability::Available => None,
            RemoteAvailability::Unavailable => {
                Some(backend_unavailable_message(ssh_present, relay_present))
            }
        };
        RemoteBackendStatus {
            availability,
            message,
            ssh_executable_present: Some(ssh_present),
            relay_binary_present: Some(relay_present),
        }
    }

    fn refresh_executable_probes(&mut self) {
        self.cached_ssh_present = Some(probe_ssh_executable(&self.config.ssh_executable));
        self.cached_scp_present = Some(probe_scp_executable(&self.config.scp_executable));
    }

    fn ssh_executable_present(&self) -> bool {
        self.cached_ssh_present
            .unwrap_or_else(|| probe_ssh_executable(&self.config.ssh_executable))
    }

    fn scp_executable_present(&self) -> bool {
        self.cached_scp_present
            .unwrap_or_else(|| probe_scp_executable(&self.config.scp_executable))
    }

    pub fn list_hosts(&mut self) -> Vec<RemoteHostView> {
        let backend = self.backend_status();
        if self.hosts.is_empty() {
            return Vec::new();
        }
        let snapshots = self.manager.snapshots();
        self.hosts
            .values()
            .map(|config| {
                let snapshot = snapshots.iter().find(|entry| entry.host_id == config.id);
                let last_connected_at_ms = self
                    .session_watch
                    .get(&config.id)
                    .and_then(|watch| watch.last_connected_at_ms);
                host_view(config, backend.availability, snapshot, last_connected_at_ms)
            })
            .collect()
    }

    pub fn preview_deploy(&self, host_id: &str) -> Result<RemoteDeploymentPlanView, String> {
        let backend = self.backend_status();
        if backend.availability == RemoteAvailability::Unavailable {
            return Err(backend
                .message
                .unwrap_or_else(|| "SSH relay backend is unavailable".into()));
        }
        let plan = self.build_deployment_plan(host_id)?;
        let hook_guidance = remote_hook_spool_guidance(&self.config.remote_runtime_dir);
        Ok(RemoteDeploymentPlanView {
            host_id: plan.host_id,
            steps: plan.steps.into_iter().map(deployment_step_view).collect(),
            availability: RemoteAvailability::Available,
            message: Some(format!(
                "Review the steps below, then execute deploy to upload and activate the relay binary. {hook_guidance}"
            )),
        })
    }

    pub fn execute_deploy(&self, host_id: &str) -> Result<RemoteDeploymentResultView, String> {
        let backend = self.backend_status();
        if backend.availability == RemoteAvailability::Unavailable {
            return Err(backend
                .message
                .unwrap_or_else(|| "SSH relay backend is unavailable".into()));
        }
        if !self.scp_executable_present() {
            return Err(
                "SCP is unavailable; install OpenSSH scp before executing remote deployment."
                    .into(),
            );
        }
        let host = self
            .hosts
            .get(host_id)
            .ok_or_else(|| format!("remote host `{host_id}` is not configured"))?
            .clone();
        let plan = self.build_deployment_plan(host_id)?;
        let transport =
            OpenSshDeployTransport::new(&self.config.ssh_executable, &self.config.scp_executable);
        self.execute_deploy_with_transport(&host, &plan, &transport)
    }

    pub(crate) fn execute_deploy_with_transport<T: DeployTransport>(
        &self,
        host: &RemoteHostConfig,
        plan: &DeploymentPlan,
        transport: &T,
    ) -> Result<RemoteDeploymentResultView, String> {
        let outcome = DeploymentExecutor::new(transport)
            .execute(host, plan)
            .map_err(|error| error.to_string())?;
        Ok(RemoteDeploymentResultView {
            host_id: plan.host_id.clone(),
            availability: RemoteAvailability::Available,
            completed_steps: outcome
                .completed_steps
                .into_iter()
                .map(deployment_step_view)
                .collect(),
            probed_target: outcome.probed_target.map(remote_target_view),
            message: Some(format!(
                "Relay artifact uploaded, verified, and activated. Use Start relay to connect. {}",
                remote_hook_spool_guidance(&self.config.remote_runtime_dir)
            )),
        })
    }

    fn build_deployment_plan(&self, host_id: &str) -> Result<DeploymentPlan, String> {
        let host = self
            .hosts
            .get(host_id)
            .ok_or_else(|| format!("remote host `{host_id}` is not configured"))?;
        let transport =
            OpenSshDeployTransport::new(&self.config.ssh_executable, &self.config.scp_executable);
        self.build_deployment_plan_with_transport(host, &transport)
    }

    pub(crate) fn build_deployment_plan_with_transport<T: DeployTransport>(
        &self,
        host: &RemoteHostConfig,
        transport: &T,
    ) -> Result<DeploymentPlan, String> {
        let target = transport
            .probe_target(host)
            .map_err(|error| format!("remote target probe failed: {error}"))?;
        self.build_deployment_plan_for_target(host, target)
    }

    pub(crate) fn build_deployment_plan_for_target(
        &self,
        host: &RemoteHostConfig,
        target: RemoteTarget,
    ) -> Result<DeploymentPlan, String> {
        let artifact = resolve_relay_artifact(
            &self.config.relay_binaries_dir,
            &self.config.relay_binary_path,
            target,
        )
        .map_err(map_relay_artifact_error)?;
        DeploymentPlan::new_with_runtime_dir(
            host,
            artifact,
            DEFAULT_REMOTE_BIN_DIRECTORY,
            &self.config.remote_runtime_dir,
        )
        .map_err(|error| error.to_string())
    }

    pub fn start_relay(&mut self, host_id: &str) -> Result<RemoteConnectionStatusView, String> {
        let backend = self.backend_status();
        if backend.availability == RemoteAvailability::Unavailable {
            return Err(backend
                .message
                .unwrap_or_else(|| "SSH relay backend is unavailable".into()));
        }
        let host = self
            .hosts
            .get(host_id)
            .ok_or_else(|| format!("remote host `{host_id}` is not configured"))?
            .clone();
        if self.manager.get(host_id).is_none() {
            let relay_path = format!("{}/llm-notch-relay", DEFAULT_REMOTE_BIN_DIRECTORY);
            let transport = Box::new(
                OpenSshTransport::with_remote_relay_path(&self.config.ssh_executable, relay_path)
                    .map_err(|error| error.to_string())?
                    .with_event_spool_dir(&self.config.remote_runtime_dir)
                    .map_err(|error| error.to_string())?,
            );
            let session = RelaySession::new(host, transport);
            self.manager.register(session).map_err(map_session_error)?;
        }
        let session = self
            .manager
            .get_mut(host_id)
            .ok_or_else(|| format!("relay session for `{host_id}` is missing"))?;
        session.start().map_err(map_session_error)?;
        self.reset_session_watch(host_id);
        Ok(self.connection_status(host_id))
    }

    pub fn stop_relay(&mut self, host_id: &str) -> Result<RemoteConnectionStatusView, String> {
        let backend = self.backend_status();
        if backend.availability == RemoteAvailability::Unavailable {
            return Err(backend
                .message
                .unwrap_or_else(|| "SSH relay backend is unavailable".into()));
        }
        if self.hosts.get(host_id).is_none() {
            return Err(format!("remote host `{host_id}` is not configured"));
        }
        if self.manager.get(host_id).is_some() {
            self.manager.remove(host_id).map_err(map_session_error)?;
        }
        self.session_watch.remove(host_id);
        Ok(self.connection_status(host_id))
    }

    /// Polls registered relay sessions: receives frames, attempts reconnects, and returns
    /// connection statuses that changed since the previous poll plus relay session events.
    pub fn poll_active_sessions(&mut self, now_ms: i64) -> RemotePollResult {
        let backend = self.backend_status();
        if backend.availability == RemoteAvailability::Unavailable {
            return RemotePollResult::default();
        }

        let host_ids = self
            .manager
            .snapshots()
            .into_iter()
            .map(|snapshot| snapshot.host_id)
            .collect::<Vec<_>>();
        let mut result = RemotePollResult::default();

        for host_id in host_ids {
            let Some(session) = self.manager.get_mut(&host_id) else {
                continue;
            };
            let watch = self.session_watch.entry(host_id.clone()).or_default();
            let connection_state = session.snapshot().state;

            match connection_state {
                ConnectionState::Streaming => match session.receive() {
                    Ok(Some(frame)) => match handle_relay_frame(&host_id, session, frame) {
                        Ok(Some(event)) => result.session_events.push(event),
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(host_id = %host_id, %error, "relay frame handling failed");
                        }
                    },
                    Ok(None) => {
                        tracing::info!(host_id = %host_id, "relay session ended");
                        schedule_reconnect(watch, now_ms);
                    }
                    Err(RelaySessionError::NotActive(_)) => {}
                    Err(error) => {
                        tracing::warn!(host_id = %host_id, %error, "relay receive failed");
                    }
                },
                ConnectionState::Disconnected => {
                    if now_ms >= watch.next_reconnect_at_ms {
                        let jitter = reconnect_jitter_basis_points(&host_id);
                        match session.tick_reconnect(jitter) {
                            Ok(true) => {
                                watch.reconnect_attempt = 0;
                                watch.next_reconnect_at_ms = 0;
                                watch.last_connected_at_ms = Some(now_ms);
                                tracing::info!(host_id = %host_id, "relay session reconnected");
                            }
                            Ok(false) => {}
                            Err(RelaySessionError::RequiresRestart)
                            | Err(RelaySessionError::NotActive(_)) => {}
                            Err(RelaySessionError::Transport(_))
                            | Err(RelaySessionError::AlreadyActive(_)) => {
                                schedule_reconnect(watch, now_ms);
                            }
                        }
                    }
                }
                ConnectionState::Failed
                | ConnectionState::Connecting
                | ConnectionState::Authenticating => {}
                ConnectionState::Backoff { .. } => {}
            }

            let snapshot = session.snapshot();
            let current_state = connection_state_view(snapshot.state);
            if current_state != watch.last_connection_state {
                if current_state == RemoteConnectionState::Streaming {
                    watch.last_connected_at_ms = Some(now_ms);
                    watch.reconnect_attempt = 0;
                    watch.next_reconnect_at_ms = 0;
                } else if watch.last_connection_state == RemoteConnectionState::Streaming
                    && matches!(
                        current_state,
                        RemoteConnectionState::Disconnected | RemoteConnectionState::Failed
                    )
                {
                    schedule_reconnect(watch, now_ms);
                }
                watch.last_connection_state = current_state;
                result.connection_updates.push(RemoteConnectionStatusView {
                    host_id: host_id.clone(),
                    availability: RemoteAvailability::Available,
                    connection_state: current_state,
                    message: snapshot.last_error.clone(),
                });
            }
        }

        result
    }

    pub fn connection_status(&mut self, host_id: &str) -> RemoteConnectionStatusView {
        let backend = self.backend_status();
        let Some(host) = self.hosts.get(host_id) else {
            return RemoteConnectionStatusView {
                host_id: host_id.into(),
                availability: RemoteAvailability::Unavailable,
                connection_state: RemoteConnectionState::Disconnected,
                message: Some(format!("remote host `{host_id}` is not configured")),
            };
        };
        let snapshot = self.manager.get_mut(host_id).map(RelaySession::snapshot);
        let view = host_view(host, backend.availability, snapshot.as_ref(), None);
        RemoteConnectionStatusView {
            host_id: view.config.id,
            availability: view.availability,
            connection_state: view.connection_state,
            message: view.message,
        }
    }

    pub fn empty_hosts_message() -> &'static str {
        NO_HOSTS_MESSAGE
    }

    fn reset_session_watch(&mut self, host_id: &str) {
        let watch = self.session_watch.entry(host_id.to_string()).or_default();
        watch.reconnect_attempt = 0;
        watch.next_reconnect_at_ms = 0;
    }
}

fn handle_relay_frame(
    host_id: &str,
    session: &mut RelaySession,
    frame: RelayFrame,
) -> Result<Option<RelaySessionEventIngest>, RelaySessionError> {
    let ingest = match &frame.payload {
        RelayPayload::Heartbeat => {
            tracing::trace!(host_id = %host_id, sequence = frame.sequence, "relay heartbeat");
            None
        }
        RelayPayload::Checkpoint => {
            tracing::debug!(host_id = %host_id, sequence = frame.sequence, "relay checkpoint");
            None
        }
        RelayPayload::SessionEvent { .. } => {
            if let Some(event) = extract_relay_session_event(host_id, &frame) {
                tracing::info!(
                    host_id = %host_id,
                    relay_session_id = %event.external_session_id,
                    relay_source = %event.source,
                    occurred_at_ms = event.occurred_at_ms,
                    summary = %event.summary,
                    sequence = frame.sequence,
                    "relay session-event frame received"
                );
                Some(event)
            } else {
                None
            }
        }
        RelayPayload::Error { code } => {
            tracing::warn!(
                host_id = %host_id,
                code = %code,
                sequence = frame.sequence,
                "relay error frame"
            );
            None
        }
    };
    session.acknowledge(ResumeCursor {
        last_sequence: frame.sequence,
    })?;
    Ok(ingest)
}

fn extract_relay_session_event(
    host_id: &str,
    frame: &RelayFrame,
) -> Option<RelaySessionEventIngest> {
    match &frame.payload {
        RelayPayload::SessionEvent {
            session_id,
            source,
            summary,
            occurred_at_ms,
            kind,
            tool_name,
            attention,
        } => Some(RelaySessionEventIngest {
            host_id: host_id.to_string(),
            external_session_id: session_id.clone(),
            source: source.clone(),
            summary: summary.clone(),
            occurred_at_ms: *occurred_at_ms,
            kind: *kind,
            tool_name: tool_name.clone(),
            attention: *attention,
        }),
        _ => None,
    }
}

fn schedule_reconnect(watch: &mut SessionWatchState, now_ms: i64) {
    let policy = ReconnectPolicy::default();
    let delay_ms = policy
        .delay_ms(watch.reconnect_attempt, 0)
        .unwrap_or(policy.max_delay_ms);
    watch.reconnect_attempt = watch.reconnect_attempt.saturating_add(1);
    watch.next_reconnect_at_ms = now_ms + i64::try_from(delay_ms).unwrap_or(i64::MAX);
}

fn reconnect_jitter_basis_points(host_id: &str) -> i16 {
    host_id
        .bytes()
        .fold(0_i16, |acc, byte| acc.wrapping_add(i16::from(byte)))
        % 2_501
}

pub(crate) fn detect_ssh_executable() -> String {
    for candidate in ssh_candidates() {
        if probe_ssh_executable(&candidate) {
            return candidate;
        }
    }
    "ssh".into()
}

pub(crate) fn detect_scp_executable() -> String {
    for candidate in scp_candidates() {
        if probe_scp_executable(&candidate) {
            return candidate;
        }
    }
    "scp".into()
}

fn ssh_candidates() -> Vec<String> {
    let mut candidates = vec!["ssh".into()];
    #[cfg(windows)]
    {
        candidates.push(r"C:\Windows\System32\OpenSSH\ssh.exe".into());
    }
    candidates
}

fn scp_candidates() -> Vec<String> {
    let mut candidates = vec!["scp".into()];
    #[cfg(windows)]
    {
        candidates.push(r"C:\Windows\System32\OpenSSH\scp.exe".into());
    }
    candidates
}

fn probe_ssh_executable(executable: &str) -> bool {
    hidden_command(executable)
        .arg("-V")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn probe_scp_executable(executable: &str) -> bool {
    hidden_command(executable)
        .arg("-V")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn backend_unavailable_message(ssh_present: bool, relay_present: bool) -> String {
    match (ssh_present, relay_present) {
        (false, false) => {
            "OpenSSH and the llm-notch-relay binary are unavailable in this environment.".into()
        }
        (false, true) => "OpenSSH is unavailable; install OpenSSH before starting remote relays."
            .into(),
        (true, false) => {
            "The llm-notch-relay binary is missing from this build. Build or bundle the relay sidecar before deploying."
                .into()
        }
        (true, true) => "SSH relay backend is unavailable.".into(),
    }
}

fn host_view(
    config: &RemoteHostConfig,
    backend_availability: RemoteAvailability,
    snapshot: Option<&notch_remote::RelaySessionSnapshot>,
    last_connected_at_ms: Option<i64>,
) -> RemoteHostView {
    let availability = if backend_availability == RemoteAvailability::Unavailable {
        RemoteAvailability::Unavailable
    } else {
        RemoteAvailability::Available
    };
    let (connection_state, message) = if let Some(snapshot) = snapshot {
        (
            connection_state_view(snapshot.state),
            snapshot.last_error.clone(),
        )
    } else {
        (RemoteConnectionState::Disconnected, None)
    };
    RemoteHostView {
        config: host_config_view(config),
        availability,
        connection_state,
        message,
        last_connected_at_ms,
    }
}

fn host_config_view(config: &RemoteHostConfig) -> RemoteHostConfigView {
    RemoteHostConfigView {
        id: config.id.clone(),
        destination: config.destination.clone(),
        port: config.port,
        identity_file: config
            .identity_file
            .as_ref()
            .map(|path| path.display().to_string()),
        host_key_policy: match config.host_key_policy {
            SshHostKeyPolicy::Strict => SshHostKeyPolicyView::Strict,
            SshHostKeyPolicy::AcceptNew => SshHostKeyPolicyView::AcceptNew,
        },
        connect_timeout_seconds: config.connect_timeout_seconds,
    }
}

fn connection_state_view(state: ConnectionState) -> RemoteConnectionState {
    match state {
        ConnectionState::Disconnected => RemoteConnectionState::Disconnected,
        ConnectionState::Connecting => RemoteConnectionState::Connecting,
        ConnectionState::Authenticating => RemoteConnectionState::Authenticating,
        ConnectionState::Streaming => RemoteConnectionState::Streaming,
        ConnectionState::Failed => RemoteConnectionState::Failed,
        ConnectionState::Backoff { attempt, delay_ms } => {
            RemoteConnectionState::Backoff { attempt, delay_ms }
        }
    }
}

fn deployment_step_view(step: DeploymentStep) -> RemoteDeploymentStepView {
    match step {
        DeploymentStep::ProbeTarget => RemoteDeploymentStepView::ProbeTarget,
        DeploymentStep::CreatePrivateDirectory { remote_directory } => {
            RemoteDeploymentStepView::CreatePrivateDirectory { remote_directory }
        }
        DeploymentStep::UploadTemporary { remote_path } => {
            RemoteDeploymentStepView::UploadTemporary { remote_path }
        }
        DeploymentStep::VerifySha256 { expected_sha256 } => {
            RemoteDeploymentStepView::VerifySha256 { expected_sha256 }
        }
        DeploymentStep::ActivateAtomically { remote_path } => {
            RemoteDeploymentStepView::ActivateAtomically { remote_path }
        }
        DeploymentStep::StartStdioRelay {
            remote_path,
            event_spool_dir,
        } => RemoteDeploymentStepView::StartStdioRelay {
            remote_path,
            event_spool_dir,
        },
    }
}

fn remote_target_view(target: RemoteTarget) -> RemoteTargetView {
    RemoteTargetView {
        os: match target.os {
            RemoteOs::Linux => RemoteOsView::Linux,
            RemoteOs::Macos => RemoteOsView::Macos,
            RemoteOs::Windows => RemoteOsView::Windows,
        },
        architecture: match target.architecture {
            RemoteArchitecture::X86_64 => RemoteArchitectureView::X86_64,
            RemoteArchitecture::Aarch64 => RemoteArchitectureView::Aarch64,
        },
    }
}

fn map_session_error(error: RelaySessionError) -> String {
    error.to_string()
}

fn map_relay_artifact_error(error: RelayArtifactError) -> String {
    match error {
        RelayArtifactError::UnsupportedTarget(target) => {
            format!("remote deploy does not support {target:?} targets over SSH")
        }
        RelayArtifactError::MissingArtifact {
            target,
            expected_path,
        } => format!(
            "no relay artifact for {target:?}; cross-compile with `npm run native:prepare-helper -- --target {}` and place the sidecar at {}",
            notch_remote::rust_triple_for_target(target).unwrap_or("unknown"),
            expected_path.display()
        ),
        RelayArtifactError::Unreadable { path, message } => {
            format!(
                "relay artifact is unreadable at {}: {message}",
                path.display()
            )
        }
    }
}

fn host_config_from_input(input: RemoteHostConfigInput) -> Result<RemoteHostConfig, String> {
    let identity_file = match input.identity_file {
        Some(path) if path.trim().is_empty() => None,
        Some(path) => Some(PathBuf::from(path)),
        None => None,
    };
    let config = RemoteHostConfig {
        id: input.id,
        destination: input.destination,
        port: input.port,
        identity_file,
        known_hosts_file: None,
        host_key_policy: match input.host_key_policy {
            SshHostKeyPolicyView::Strict => SshHostKeyPolicy::Strict,
            SshHostKeyPolicyView::AcceptNew => SshHostKeyPolicy::AcceptNew,
        },
        connect_timeout_seconds: input.connect_timeout_seconds,
    };
    config.validate().map_err(|error| error.to_string())?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use notch_remote::DirectRelayTransport;

    fn sample_host(id: &str) -> RemoteHostConfig {
        RemoteHostConfig {
            id: id.into(),
            destination: "dev@example.internal".into(),
            port: None,
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        }
    }

    fn registry_with_relay(relay_path: &Path) -> DesktopRemoteRegistry {
        DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            )
            .with_relay_binaries_dir(relay_binaries_dir_for_tests()),
        )
    }

    fn relay_binaries_dir_for_tests() -> PathBuf {
        std::env::var("CARGO_BIN_EXE_llm-notch-relay")
            .ok()
            .and_then(|relay_exe| PathBuf::from(relay_exe).parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"))
    }

    struct ProbeOnlyDeployTransport {
        target: RemoteTarget,
    }

    impl DeployTransport for ProbeOnlyDeployTransport {
        fn probe_target(
            &self,
            _host: &RemoteHostConfig,
        ) -> Result<RemoteTarget, notch_remote::DeployTransportError> {
            Ok(self.target)
        }

        fn run_remote(
            &self,
            _host: &RemoteHostConfig,
            _script: &str,
        ) -> Result<String, notch_remote::DeployTransportError> {
            Err(notch_remote::DeployTransportError::Protocol(
                "unexpected remote command".into(),
            ))
        }

        fn upload_file(
            &self,
            _host: &RemoteHostConfig,
            _local_path: &Path,
            _remote_path: &str,
        ) -> Result<(), notch_remote::DeployTransportError> {
            Err(notch_remote::DeployTransportError::Protocol(
                "unexpected upload".into(),
            ))
        }
    }

    fn registry_with_linux_relay_artifact(
        relay_path: &Path,
    ) -> (DesktopRemoteRegistry, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RemoteTarget {
            os: RemoteOs::Linux,
            architecture: RemoteArchitecture::X86_64,
        };
        let artifact_name =
            notch_remote::sidecar_filename_for_target(target).expect("linux triple");
        let artifact_path = dir.path().join(artifact_name);
        std::fs::copy(relay_path, &artifact_path).expect("copy relay sidecar");
        let registry = DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            )
            .with_relay_binaries_dir(dir.path().to_path_buf()),
        );
        (registry, dir)
    }

    #[test]
    fn backend_status_reuses_cached_executable_probes() {
        let registry = registry_with_relay(Path::new("/definitely/missing/llm-notch-relay"));
        assert!(registry.cached_ssh_probe().is_some());
        assert!(registry.cached_scp_probe().is_some());
        let status = registry.backend_status();
        assert_eq!(status.ssh_executable_present, registry.cached_ssh_probe());
    }

    #[test]
    fn empty_registry_reports_probe_results_without_fabricating_hosts() {
        let mut registry = registry_with_relay(Path::new("/definitely/missing/llm-notch-relay"));
        let status = registry.backend_status();
        assert_eq!(status.availability, RemoteAvailability::Unavailable);
        assert!(status.ssh_executable_present.is_some());
        assert_eq!(status.relay_binary_present, Some(false));
        assert!(registry.list_hosts().is_empty());
        assert!(status.message.as_deref().unwrap().contains("relay binary"));
    }

    #[test]
    fn lifecycle_actions_require_configured_host() {
        let mut registry = registry_with_relay(Path::new("/definitely/missing/llm-notch-relay"));
        assert!(registry.preview_deploy("dev-box").is_err());
        assert!(registry.execute_deploy("dev-box").is_err());
        assert!(registry.start_relay("dev-box").is_err());
        assert!(registry.stop_relay("dev-box").is_err());

        let status = registry.connection_status("dev-box");
        assert_eq!(status.availability, RemoteAvailability::Unavailable);
        assert_eq!(status.connection_state, RemoteConnectionState::Disconnected);
        assert!(
            status
                .message
                .as_deref()
                .unwrap()
                .contains("not configured")
        );
    }

    fn relay_binary_path() -> Option<PathBuf> {
        std::env::var("CARGO_BIN_EXE_llm-notch-relay")
            .ok()
            .map(PathBuf::from)
            .filter(|path| path.is_file())
    }

    #[test]
    fn preview_deploy_builds_plan_for_configured_host_when_relay_exists() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let (mut registry, _dir) = registry_with_linux_relay_artifact(&relay_path);
        registry.register_host(sample_host("dev-box")).unwrap();
        let status = registry.backend_status();
        if status.availability == RemoteAvailability::Unavailable {
            return;
        }

        let host = registry.hosts.get("dev-box").unwrap().clone();
        let transport = ProbeOnlyDeployTransport {
            target: RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            },
        };
        let plan = registry
            .build_deployment_plan_with_transport(&host, &transport)
            .expect("deploy plan");
        assert_eq!(plan.host_id, "dev-box");
        assert_eq!(plan.artifact.target.os, RemoteOs::Linux);
        assert_eq!(
            plan.artifact.target.architecture,
            RemoteArchitecture::X86_64
        );
        assert!(
            plan.steps
                .iter()
                .any(|step| matches!(step, DeploymentStep::ProbeTarget))
        );
        assert!(plan.steps.iter().any(|step| matches!(
            step,
            DeploymentStep::StartStdioRelay {
                event_spool_dir,
                ..
            } if event_spool_dir == DEFAULT_REMOTE_RUNTIME_DIRECTORY
        )));
    }

    #[test]
    fn persisted_hosts_roundtrip_through_sqlite_repository() {
        let repository = Arc::new(SqliteRepository::in_memory().expect("in-memory sqlite"));
        let relay_path = Path::new("/definitely/missing/llm-notch-relay");
        let mut registry = DesktopRemoteRegistry::with_config_and_repository(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            ),
            Some(Arc::clone(&repository)),
        );
        assert!(registry.list_hosts().is_empty());

        let input = RemoteHostConfigInput {
            id: "dev-box".into(),
            destination: "dev@example.internal".into(),
            port: Some(2222),
            identity_file: None,
            host_key_policy: SshHostKeyPolicyView::Strict,
            connect_timeout_seconds: 10,
        };
        let view = registry.upsert_host_input(input).expect("upsert host");
        assert_eq!(view.config.id, "dev-box");
        assert_eq!(view.connection_state, RemoteConnectionState::Disconnected);

        let mut reloaded = DesktopRemoteRegistry::with_config_and_repository(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            ),
            Some(repository),
        );
        let hosts = reloaded.list_hosts();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].config.id, "dev-box");
        assert_eq!(hosts[0].config.destination, "dev@example.internal");
        assert_eq!(hosts[0].config.port, Some(2222));
        assert_eq!(
            hosts[0].connection_state,
            RemoteConnectionState::Disconnected
        );
    }

    #[test]
    fn remove_host_deletes_persisted_entry() {
        let repository = Arc::new(SqliteRepository::in_memory().expect("in-memory sqlite"));
        let relay_path = Path::new("/definitely/missing/llm-notch-relay");
        let mut registry = DesktopRemoteRegistry::with_config_and_repository(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            ),
            Some(Arc::clone(&repository)),
        );
        registry
            .upsert_host(sample_host("dev-box"))
            .expect("upsert host");
        registry.remove_host("dev-box").expect("remove host");
        assert!(registry.list_hosts().is_empty());

        let mut reloaded = DesktopRemoteRegistry::with_config_and_repository(
            RemoteRegistryConfig::new(
                detect_ssh_executable(),
                detect_scp_executable(),
                relay_path.into(),
            ),
            Some(repository),
        );
        assert!(reloaded.list_hosts().is_empty());
    }

    #[test]
    fn host_input_validation_rejects_invalid_destination() {
        let mut registry = registry_with_relay(Path::new("/definitely/missing/llm-notch-relay"));
        let result = registry.upsert_host_input(RemoteHostConfigInput {
            id: "bad".into(),
            destination: "host;rm".into(),
            port: None,
            identity_file: None,
            host_key_policy: SshHostKeyPolicyView::Strict,
            connect_timeout_seconds: 10,
        });
        assert!(result.is_err());
        assert!(registry.list_hosts().is_empty());
    }

    #[test]
    fn poll_active_sessions_acknowledges_heartbeat_without_fabricating_session_events() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let mut registry = registry_with_relay(&relay_path);
        registry.register_host(sample_host("local-relay")).unwrap();
        let transport = Box::new(DirectRelayTransport::new(relay_path.display().to_string()));
        let session = RelaySession::new(sample_host("local-relay"), transport);
        registry.manager.register(session).unwrap();
        registry
            .manager
            .get_mut("local-relay")
            .unwrap()
            .start()
            .expect("relay handshake");

        let now_ms = 1_700_000_000_000;
        let poll = registry.poll_active_sessions(now_ms);
        assert!(
            poll.connection_updates
                .iter()
                .any(|update| update.connection_state == RemoteConnectionState::Streaming)
        );
        assert!(poll.session_events.is_empty());

        let second_pass = registry.poll_active_sessions(now_ms + 1_000);
        assert!(
            second_pass.connection_updates.is_empty(),
            "heartbeat poll should not emit status churn"
        );
        assert!(second_pass.session_events.is_empty());
    }

    #[test]
    fn poll_active_sessions_reports_disconnect_honestly() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let mut registry = registry_with_relay(&relay_path);
        registry.register_host(sample_host("local-relay")).unwrap();
        let transport = Box::new(DirectRelayTransport::new(relay_path.display().to_string()));
        let session = RelaySession::new(sample_host("local-relay"), transport);
        registry.manager.register(session).unwrap();
        registry
            .manager
            .get_mut("local-relay")
            .unwrap()
            .start()
            .expect("relay handshake");
        let _ = registry.poll_active_sessions(1_700_000_000_000);
        registry
            .manager
            .get_mut("local-relay")
            .unwrap()
            .stop()
            .expect("relay shutdown");

        let poll = registry.poll_active_sessions(1_700_000_001_000);
        assert!(
            poll.connection_updates
                .iter()
                .any(|update| update.connection_state == RemoteConnectionState::Disconnected)
        );
        assert!(poll.session_events.is_empty());
    }

    use notch_protocol::{AttentionKind, SessionEventKind};

    #[test]
    fn extract_relay_session_event_maps_session_event_payload() {
        let frame = RelayFrame {
            sequence: 3,
            payload: RelayPayload::SessionEvent {
                session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Tool finished on remote host".into(),
                occurred_at_ms: 1_700_000_000_123,
                kind: Some(SessionEventKind::Tool),
                tool_name: Some("run_command".into()),
                attention: None,
            },
        };
        assert_eq!(
            extract_relay_session_event("dev-box", &frame),
            Some(RelaySessionEventIngest {
                host_id: "dev-box".into(),
                external_session_id: "remote-session-1".into(),
                source: "codex".into(),
                summary: "Tool finished on remote host".into(),
                occurred_at_ms: 1_700_000_000_123,
                kind: Some(SessionEventKind::Tool),
                tool_name: Some("run_command".into()),
                attention: None,
            })
        );
        assert!(
            extract_relay_session_event(
                "dev-box",
                &RelayFrame {
                    sequence: 4,
                    payload: RelayPayload::Heartbeat,
                }
            )
            .is_none()
        );
    }

    #[test]
    fn extract_relay_session_event_preserves_legacy_frames_without_metadata() {
        let frame = RelayFrame {
            sequence: 5,
            payload: RelayPayload::SessionEvent {
                session_id: "remote-session-legacy".into(),
                source: "codex".into(),
                summary: "Legacy relay event".into(),
                occurred_at_ms: 1_700_000_000_200,
                kind: None,
                tool_name: None,
                attention: None,
            },
        };
        assert_eq!(
            extract_relay_session_event("dev-box", &frame),
            Some(RelaySessionEventIngest {
                host_id: "dev-box".into(),
                external_session_id: "remote-session-legacy".into(),
                source: "codex".into(),
                summary: "Legacy relay event".into(),
                occurred_at_ms: 1_700_000_000_200,
                kind: None,
                tool_name: None,
                attention: None,
            })
        );
    }

    #[test]
    fn extract_relay_session_event_maps_attention_metadata() {
        let frame = RelayFrame {
            sequence: 6,
            payload: RelayPayload::SessionEvent {
                session_id: "remote-session-2".into(),
                source: "cursor".into(),
                summary: "Approve shell command".into(),
                occurred_at_ms: 1_700_000_000_300,
                kind: Some(SessionEventKind::Attention),
                tool_name: None,
                attention: Some(AttentionKind::Permission),
            },
        };
        assert_eq!(
            extract_relay_session_event("dev-box", &frame).expect("event"),
            RelaySessionEventIngest {
                host_id: "dev-box".into(),
                external_session_id: "remote-session-2".into(),
                source: "cursor".into(),
                summary: "Approve shell command".into(),
                occurred_at_ms: 1_700_000_000_300,
                kind: Some(SessionEventKind::Attention),
                tool_name: None,
                attention: Some(AttentionKind::Permission),
            }
        );
    }

    #[test]
    fn direct_relay_session_reports_streaming_only_after_real_handshake() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let mut registry = registry_with_relay(&relay_path);
        registry.register_host(sample_host("local-relay")).unwrap();
        let transport = Box::new(DirectRelayTransport::new(relay_path.display().to_string()));
        let session = RelaySession::new(sample_host("local-relay"), transport);
        registry.manager.register(session).unwrap();

        let snapshot = registry.manager.get_mut("local-relay").unwrap().snapshot();
        assert_eq!(snapshot.state, ConnectionState::Disconnected);

        registry
            .manager
            .get_mut("local-relay")
            .unwrap()
            .start()
            .expect("relay handshake");
        let snapshot = registry.manager.get_mut("local-relay").unwrap().snapshot();
        assert_eq!(snapshot.state, ConnectionState::Streaming);
        assert!(snapshot.process_alive);
        assert_eq!(snapshot.connection_nonce.as_deref().map(str::len), Some(64));

        registry
            .manager
            .get_mut("local-relay")
            .unwrap()
            .stop()
            .expect("relay shutdown");
        let status = registry.connection_status("local-relay");
        assert_eq!(status.connection_state, RemoteConnectionState::Disconnected);
    }

    struct RegistryFakeDeployTransport {
        expected_hash: String,
    }

    impl DeployTransport for RegistryFakeDeployTransport {
        fn probe_target(
            &self,
            _host: &RemoteHostConfig,
        ) -> Result<RemoteTarget, notch_remote::DeployTransportError> {
            Ok(RemoteTarget {
                os: RemoteOs::Linux,
                architecture: RemoteArchitecture::X86_64,
            })
        }

        fn run_remote(
            &self,
            _host: &RemoteHostConfig,
            script: &str,
        ) -> Result<String, notch_remote::DeployTransportError> {
            if script.starts_with("mkdir -p") {
                return Ok(String::new());
            }
            if script.contains("sha256sum") || script.contains("shasum") {
                return Ok(self.expected_hash.clone());
            }
            if script.starts_with("test -f") {
                return Ok(String::new());
            }
            Err(notch_remote::DeployTransportError::Protocol(format!(
                "unexpected script: {script}"
            )))
        }

        fn upload_file(
            &self,
            _host: &RemoteHostConfig,
            _local_path: &Path,
            _remote_path: &str,
        ) -> Result<(), notch_remote::DeployTransportError> {
            Ok(())
        }
    }

    #[test]
    fn build_deployment_plan_for_target_selects_matching_sidecar() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RemoteTarget {
            os: RemoteOs::Linux,
            architecture: RemoteArchitecture::X86_64,
        };
        let artifact_path = dir.path().join("llm-notch-relay-x86_64-unknown-linux-gnu");
        std::fs::write(&artifact_path, b"relay-bytes").expect("write sidecar");
        let mut registry = DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new("ssh", "scp", Path::new("/missing/fallback").into())
                .with_relay_binaries_dir(dir.path().to_path_buf()),
        );
        registry.register_host(sample_host("dev-box")).unwrap();
        let host = registry.hosts.get("dev-box").unwrap().clone();
        let plan = registry
            .build_deployment_plan_for_target(&host, target)
            .expect("plan");
        assert_eq!(plan.artifact.target, target);
        assert_eq!(plan.artifact.local_path, artifact_path);
        assert_eq!(plan.artifact.byte_len, 11);
    }

    #[test]
    fn build_deployment_plan_with_transport_selects_darwin_arm64_sidecar() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = RemoteTarget {
            os: RemoteOs::Macos,
            architecture: RemoteArchitecture::Aarch64,
        };
        let artifact_path = dir.path().join("llm-notch-relay-aarch64-apple-darwin");
        std::fs::write(&artifact_path, b"darwin-relay").expect("write sidecar");
        let mut registry = DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new("ssh", "scp", Path::new("/missing/fallback").into())
                .with_relay_binaries_dir(dir.path().to_path_buf()),
        );
        registry.register_host(sample_host("mac-box")).unwrap();
        let host = registry.hosts.get("mac-box").unwrap().clone();
        let transport = ProbeOnlyDeployTransport { target };
        let plan = registry
            .build_deployment_plan_with_transport(&host, &transport)
            .expect("plan");
        assert_eq!(plan.artifact.target, target);
        assert_eq!(plan.artifact.local_path, artifact_path);
    }

    #[test]
    fn build_deployment_plan_for_target_fails_when_matching_artifact_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let wrong = dir.path().join("llm-notch-relay-aarch64-apple-darwin");
        std::fs::write(&wrong, b"wrong-arch").expect("write wrong sidecar");
        let mut registry = DesktopRemoteRegistry::with_config(
            RemoteRegistryConfig::new("ssh", "scp", Path::new("/missing/fallback").into())
                .with_relay_binaries_dir(dir.path().to_path_buf()),
        );
        registry.register_host(sample_host("dev-box")).unwrap();
        let host = registry.hosts.get("dev-box").unwrap().clone();
        let error = registry
            .build_deployment_plan_for_target(
                &host,
                RemoteTarget {
                    os: RemoteOs::Linux,
                    architecture: RemoteArchitecture::X86_64,
                },
            )
            .expect_err("missing linux artifact");
        assert!(error.contains("no relay artifact"));
        assert!(error.contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn execute_deploy_with_transport_reports_completed_activation_steps() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let (mut registry, _dir) = registry_with_linux_relay_artifact(&relay_path);
        let host = sample_host("dev-box");
        registry.register_host(host.clone()).unwrap();
        let target = RemoteTarget {
            os: RemoteOs::Linux,
            architecture: RemoteArchitecture::X86_64,
        };
        let plan = registry
            .build_deployment_plan_for_target(&host, target)
            .expect("plan");
        let expected_hash = plan
            .steps
            .iter()
            .find_map(|step| match step {
                DeploymentStep::VerifySha256 { expected_sha256 } => Some(expected_sha256.clone()),
                _ => None,
            })
            .expect("verify step");
        let transport = RegistryFakeDeployTransport { expected_hash };
        let result = registry
            .execute_deploy_with_transport(&host, &plan, &transport)
            .expect("deploy result");
        assert_eq!(result.host_id, "dev-box");
        assert!(
            result
                .completed_steps
                .iter()
                .any(|step| matches!(step, RemoteDeploymentStepView::ActivateAtomically { .. }))
        );
        assert_eq!(
            result.probed_target,
            Some(RemoteTargetView {
                os: RemoteOsView::Linux,
                architecture: RemoteArchitectureView::X86_64,
            })
        );
    }

    #[test]
    fn execute_deploy_requires_scp_when_backend_is_available() {
        let Some(relay_path) = relay_binary_path() else {
            return;
        };
        let mut registry = DesktopRemoteRegistry::with_config(RemoteRegistryConfig::new(
            detect_ssh_executable(),
            "/definitely/missing/scp",
            relay_path,
        ));
        registry.register_host(sample_host("dev-box")).unwrap();
        let status = registry.backend_status();
        if status.availability == RemoteAvailability::Unavailable {
            return;
        }
        let error = registry.execute_deploy("dev-box").expect_err("scp missing");
        assert!(error.contains("SCP is unavailable"));
    }
}
