use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use notch_core::{
    AcknowledgeAttentionCommand, AppCore, AttentionCommand, Clock, IngestCommand,
    IntegrationHealthCommand, LifecycleEventCommand, ProcessRootCommand, SessionEndCommand,
    SessionStartCommand, SessionUpdateCommand, SqliteRepository, ToolEventCommand,
};
use notch_ipc::{IngestServerConfig, NormalizedIngest, SecurityCapabilities, start_ingest_server};
use notch_metrics::MetricsEngine;
use notch_protocol::{
    AdapterCapabilities, AgentSession, AgentSource, AttentionCapability, AttentionKind,
    AttributionQuality, EventLevel, PublicSettings, STREAM_HEARTBEAT_INTERVAL_MS, SessionEventKind,
    SessionStatus,
};
use parking_lot::Mutex;
use tauri::{AppHandle, Wry};
use tauri::async_runtime::JoinHandle;
use crate::services::SharedTrayService;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use crate::stream::StreamHub;
use crate::services::AlertNotifier;

pub type DesktopCore = AppCore<SystemClock, SqliteRepository, StreamHub>;

#[derive(Debug, Clone)]
pub struct IpcRuntimeStatus {
    pub runtime_dir: PathBuf,
    pub socket_path: String,
    pub capabilities: SecurityCapabilities,
}

/// Shared host state accessed by both overlay and dashboard windows.
pub struct HostState {
    core: Arc<DesktopCore>,
    metrics: MetricsEngine,
    stream_hub: Arc<StreamHub>,
    ipc_runtime_dir: Option<PathBuf>,
    ipc_status: Mutex<Option<IpcRuntimeStatus>>,
    metrics_paused: AtomicBool,
    shutting_down: AtomicBool,
    shutdown_tx: watch::Sender<bool>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    alert_notifier: Arc<AlertNotifier>,
    tray_hooks: Mutex<Option<(AppHandle, SharedTrayService<Wry>)>>,
}

/// Production wall-clock implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

impl HostState {
    pub fn new(core: Arc<DesktopCore>, metrics: MetricsEngine, stream_hub: Arc<StreamHub>) -> Self {
        Self::with_runtime_dir(core, metrics, stream_hub, None)
    }

    pub fn with_alert_notifier(
        core: Arc<DesktopCore>,
        metrics: MetricsEngine,
        stream_hub: Arc<StreamHub>,
        alert_notifier: Arc<AlertNotifier>,
    ) -> Self {
        Self::with_runtime_dir_and_notifier(core, metrics, stream_hub, None, alert_notifier)
    }

    pub fn with_runtime_dir(
        core: Arc<DesktopCore>,
        metrics: MetricsEngine,
        stream_hub: Arc<StreamHub>,
        ipc_runtime_dir: Option<PathBuf>,
    ) -> Self {
        Self::with_runtime_dir_and_notifier(
            core,
            metrics,
            stream_hub,
            ipc_runtime_dir,
            Arc::new(AlertNotifier::new()),
        )
    }

    pub fn with_runtime_dir_and_notifier(
        core: Arc<DesktopCore>,
        metrics: MetricsEngine,
        stream_hub: Arc<StreamHub>,
        ipc_runtime_dir: Option<PathBuf>,
        alert_notifier: Arc<AlertNotifier>,
    ) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        let state = Self {
            core,
            metrics,
            stream_hub,
            ipc_runtime_dir,
            ipc_status: Mutex::new(None),
            metrics_paused: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            shutdown_tx,
            tasks: Mutex::new(Vec::new()),
            alert_notifier,
            tray_hooks: Mutex::new(None),
        };
        if let Err(error) = state.reconcile_metric_roots() {
            warn!(%error, "failed to reconcile restored process roots");
        }
        state
    }

    pub fn stream_hub(&self) -> &Arc<StreamHub> {
        &self.stream_hub
    }

    pub fn snapshot(&self) -> notch_protocol::AppSnapshot {
        self.core.snapshot()
    }

    pub fn snapshot_with_sequence(&self) -> notch_core::SnapshotWithSequence {
        self.core.snapshot_with_sequence()
    }

    pub fn settings(&self) -> PublicSettings {
        self.core.settings()
    }

    pub fn update_settings(&self, settings: PublicSettings) -> Result<PublicSettings, String> {
        self.core
            .update_settings(settings.clone())
            .map_err(|error| error.to_string())?;
        Ok(settings)
    }

    pub fn acknowledge_attention(&self, session_id: String) -> Result<(), String> {
        self.core
            .ingest(IngestCommand::AcknowledgeAttention(
                AcknowledgeAttentionCommand {
                    session_id,
                    occurred_at_ms: SystemClock.now_ms(),
                },
            ))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn persisted_metric_history(
        &self,
        requested_start_ms: i64,
        requested_end_ms: i64,
        max_points_per_series: usize,
    ) -> Result<notch_core::PersistedMetricHistory, String> {
        self.core
            .persisted_metric_history(requested_start_ms, requested_end_ms, max_points_per_series)
            .map_err(|error| error.to_string())
    }

    pub fn session_event_page(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> Result<notch_core::SessionEventPage, String> {
        self.core
            .session_event_page(session_id, before_sequence, limit)
            .map_err(|error| error.to_string())
    }

    pub fn purge_metric_history(&self) -> Result<u64, String> {
        self.metrics.clear_history();
        self.core
            .purge_metric_history()
            .map_err(|error| error.to_string())
    }

    pub fn purge_scoped(
        &self,
        scope: notch_protocol::PurgeScope,
        app: &AppHandle,
    ) -> Result<notch_protocol::PurgeResult, String> {
        let mut result = self
            .core
            .purge_scoped(scope.clone())
            .map_err(|error| error.to_string())?;
        if scope.history {
            self.metrics.clear_history();
        }
        if scope.connector_journal {
            let (_applies, backups) = crate::commands::integration::purge_connector_data(
                app,
                scope.include_backups,
            )
            .map_err(|error| error.to_string())?;
            result.backups_removed = u64::from(backups);
        }
        Ok(result)
    }

    pub fn active_alerts(&self) -> Vec<notch_core::ActiveAlert> {
        self.core.active_alerts()
    }

    pub fn alert_notifier(&self) -> Arc<AlertNotifier> {
        Arc::clone(&self.alert_notifier)
    }

    pub fn attach_tray_hooks(&self, app: AppHandle, tray: SharedTrayService<Wry>) {
        *self.tray_hooks.lock() = Some((app, tray));
    }

    pub fn sync_presentation(&self) {
        let hooks = self.tray_hooks.lock().clone();
        let Some((app, tray)) = hooks else {
            return;
        };
        self.sync_presentation_with(&app, &tray);
    }

    pub fn sync_presentation_with(&self, app: &AppHandle, tray: &SharedTrayService<Wry>) {
        let island_visible = app
            .get_webview_window("overlay")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        if let Err(error) = crate::synchronize_tray_model(
            app,
            self,
            tray,
            island_visible,
            &self.alert_notifier,
        ) {
            warn!(%error, "observability tray sync failed");
        }
    }

    pub fn set_metrics_paused(&self, paused: bool) {
        self.metrics_paused.store(paused, Ordering::Release);
    }

    pub fn metrics_paused(&self) -> bool {
        self.metrics_paused.load(Ordering::Acquire)
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    pub fn start_background(self: &Arc<Self>) {
        let ipc_state = Arc::clone(self);
        let ipc_task = tauri::async_runtime::spawn(async move {
            ipc_state.run_ipc().await;
        });

        let metrics_state = Arc::clone(self);
        let metrics_task = tauri::async_runtime::spawn(async move {
            metrics_state.run_metrics().await;
        });

        self.tasks.lock().extend([ipc_task, metrics_task]);
    }

    pub fn begin_shutdown(&self) {
        if !self.shutting_down.swap(true, Ordering::AcqRel) {
            let _ = self.shutdown_tx.send(true);
            info!("desktop host shutdown requested");
        }
    }

    pub async fn shutdown(&self) {
        self.begin_shutdown();
        let tasks = std::mem::take(&mut *self.tasks.lock());
        for task in tasks {
            match tokio::time::timeout(Duration::from_secs(3), task).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => warn!(%error, "background task join failed"),
                Err(_) => warn!("background task did not stop before timeout"),
            }
        }
    }

    fn reconcile_metric_roots(&self) -> Result<(), String> {
        let snapshot = self.snapshot();
        let registered = self.metrics.registered_session_ids();
        let mut valid_sources = Vec::<(AgentSource, AttributionQuality)>::new();
        let mut valid_session_ids = Vec::<String>::new();

        for session in &snapshot.sessions {
            let active = !matches!(
                session.status,
                SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
            );
            let Some(root) = &session.process_root else {
                continue;
            };
            if !active {
                self.metrics.unregister_session(&session.id);
                self.core
                    .clear_process_root(&session.id)
                    .map_err(|error| error.to_string())?;
                continue;
            }

            let Some(observed) = self.metrics.resolve_process_identity(root.pid) else {
                self.metrics.unregister_session(&session.id);
                self.core
                    .clear_process_root(&session.id)
                    .map_err(|error| error.to_string())?;
                continue;
            };
            if !notch_metrics::graph::start_times_match(root.started_at_ms, observed.started_at_ms)
            {
                self.metrics.unregister_session(&session.id);
                self.core
                    .clear_process_root(&session.id)
                    .map_err(|error| error.to_string())?;
                continue;
            }

            let attribution = attribution_for_source(session.source);
            if attribution == AttributionQuality::Unknown {
                continue;
            }
            valid_session_ids.push(session.id.clone());
            if !valid_sources
                .iter()
                .any(|(source, _)| *source == session.source)
            {
                valid_sources.push((session.source, attribution));
            }
            if !registered.iter().any(|id| id == &session.id) {
                self.metrics
                    .register_session(
                        session.id.clone(),
                        observed,
                        attribution,
                        session.started_at_ms,
                    )
                    .map_err(|error| error.to_string())?;
            }
        }

        for session_id in registered {
            if !valid_session_ids.iter().any(|valid| valid == &session_id) {
                self.metrics.unregister_session(&session_id);
            }
        }
        self.reconcile_adapter_attribution(&valid_sources)
    }

    fn reconcile_adapter_attribution(
        &self,
        valid_sources: &[(AgentSource, AttributionQuality)],
    ) -> Result<(), String> {
        let snapshot = self.snapshot();
        for capabilities in builtin_adapter_capabilities() {
            let attribution = valid_sources
                .iter()
                .find(|(source, _)| *source == capabilities.source)
                .map(|(_, attribution)| *attribution)
                .unwrap_or(AttributionQuality::Unknown);
            self.set_adapter_attribution_with_snapshot(
                capabilities.source,
                attribution,
                &snapshot.adapters,
            )?;
        }
        Ok(())
    }

    fn set_adapter_attribution(
        &self,
        source: AgentSource,
        attribution: AttributionQuality,
    ) -> Result<(), String> {
        let adapters = self.snapshot().adapters;
        self.set_adapter_attribution_with_snapshot(source, attribution, &adapters)
    }

    fn set_adapter_attribution_with_snapshot(
        &self,
        source: AgentSource,
        attribution: AttributionQuality,
        current: &[AdapterCapabilities],
    ) -> Result<(), String> {
        let mut capabilities = builtin_adapter_capabilities()
            .into_iter()
            .find(|capabilities| capabilities.source == source)
            .ok_or_else(|| "unsupported adapter source".to_string())?;
        capabilities.process_attribution = attribution;
        if current
            .iter()
            .find(|existing| existing.source == source)
            .is_some_and(|existing| existing == &capabilities)
        {
            return Ok(());
        }
        self.core
            .ingest(IngestCommand::IntegrationHealth(IntegrationHealthCommand {
                capabilities,
                healthy: false,
                message: Some("Template available; live connector health not yet verified".into()),
                occurred_at_ms: SystemClock.now_ms(),
            }))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn run_ipc(self: Arc<Self>) {
        let mut shutdown = self.shutdown_tx.subscribe();
        let mut server = match start_ingest_server(IngestServerConfig {
            runtime_dir: self.ipc_runtime_dir.clone(),
        })
        .await
        {
            Ok(server) => server,
            Err(error) => {
                error!(%error, "authenticated local ingest server failed to start");
                return;
            }
        };

        let status = IpcRuntimeStatus {
            runtime_dir: server.runtime_dir().to_path_buf(),
            socket_path: server.descriptor().socket_path.clone(),
            capabilities: server.capabilities().clone(),
        };
        info!(
            runtime_dir = %status.runtime_dir.display(),
            socket_path = %status.socket_path,
            peer_check = ?status.capabilities.peer_check,
            "authenticated local ingest server started"
        );
        *self.ipc_status.lock() = Some(status);

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                pending = server.recv() => {
                    let Some(pending) = pending else {
                        break;
                    };
                    let (ingest, completion) = pending.into_parts();
                    let state = Arc::clone(&self);
                    let result = match tauri::async_runtime::spawn_blocking(move || state.ingest_normalized(ingest)).await {
                        Ok(result) => result,
                        Err(error) => Err(format!("normalized ingest task failed: {error}")),
                    };
                    if let Err(error) = &result {
                        warn!(%error, "normalized ingest rejected");
                    }
                    let _ = completion.send(result);
                }
            }
        }

        *self.ipc_status.lock() = None;
        if let Err(error) = server.shutdown().await {
            warn!(%error, "ingest server shutdown failed");
        } else {
            info!("authenticated local ingest server stopped");
        }
    }

    async fn run_metrics(self: Arc<Self>) {
        let mut shutdown = self.shutdown_tx.subscribe();
        let mut timer = tokio::time::interval(Duration::from_millis(250));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_sample_ms = 0_i64;
        let mut last_heartbeat_ms = 0_i64;
        let mut last_prune_ms = 0_i64;

        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                _ = timer.tick() => {
                    let now_ms = SystemClock.now_ms();
                    let sampling_interval = self.settings().sampling_interval_ms.max(250) as i64;
                    let should_sample = !self.metrics_paused()
                        && now_ms.saturating_sub(last_sample_ms) >= sampling_interval;
                    if should_sample {
                        last_sample_ms = now_ms;
                        let state = Arc::clone(&self);
                        match tauri::async_runtime::spawn_blocking(move || state.sample_metrics(now_ms)).await {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => warn!(%error, "metrics frame rejected by core"),
                            Err(error) => warn!(%error, "metrics sampling task failed"),
                        }
                    }

                    if now_ms.saturating_sub(last_heartbeat_ms)
                        >= STREAM_HEARTBEAT_INTERVAL_MS as i64
                    {
                        last_heartbeat_ms = now_ms;
                        let should_prune =
                            now_ms.saturating_sub(last_prune_ms) >= 60 * 60 * 1_000;
                        if should_prune {
                            last_prune_ms = now_ms;
                        }
                        let state = Arc::clone(&self);
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = state.core.tick() {
                                warn!(%error, "core maintenance tick failed");
                            }
                            if let Err(error) = state.reconcile_metric_roots() {
                                warn!(%error, "process-root reconciliation failed");
                            }
                            if should_prune {
                                match state.core.purge_history() {
                                    Ok(report) => {
                                        if report.metric_buckets_deleted > 0 {
                                            state.metrics.clear_history();
                                        }
                                        debug!(
                                            events_deleted = report.events_deleted,
                                            metric_buckets_deleted = report.metric_buckets_deleted,
                                            "history retention pass completed"
                                        )
                                    }
                                    Err(error) => {
                                        warn!(%error, "history retention pass failed");
                                    }
                                }
                            }
                            if state.metrics_paused() {
                                state.core.heartbeat();
                            }
                        })
                        .await;
                    }
                }
            }
        }
        debug!("metrics sampler stopped");
    }

    fn sample_metrics(&self, at_ms: i64) -> Result<(), String> {
        let snapshot = self.snapshot();
        let active_sessions = snapshot
            .sessions
            .iter()
            .filter(|session| {
                !matches!(
                    session.status,
                    SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
                )
            })
            .count() as u32;
        let attention_sessions = snapshot
            .sessions
            .iter()
            .filter(|session| session.attention != AttentionKind::None)
            .count() as u32;
        self.metrics
            .set_session_counts(active_sessions, attention_sessions);
        let Some(frame) = self.metrics.tick(at_ms) else {
            return Ok(());
        };
        let agent_samples = frame.agents.len();
        self.core
            .record_metrics(frame)
            .map_err(|error| error.to_string())?;
        self.sync_presentation();
        debug!(at_ms, agent_samples, "metrics frame recorded");
        Ok(())
    }

    fn ingest_normalized(&self, ingest: NormalizedIngest) -> Result<(), String> {
        match ingest {
            NormalizedIngest::SessionUpsert(session) => self.ingest_session_upsert(session),
            NormalizedIngest::SessionRemove {
                session_id,
                source,
                external_session_id,
            } => {
                let resolved = self
                    .resolve_session(&session_id, source, external_session_id.as_deref())
                    .ok_or_else(|| "session not found for remove".to_string())?;
                self.core
                    .remove_session(&resolved.id)
                    .map_err(|error| error.to_string())?;
                self.metrics.unregister_session(&resolved.id);
                Ok(())
            }
            NormalizedIngest::SessionEvent {
                event,
                source,
                external_session_id,
                attention,
            } => {
                let session = self
                    .resolve_session(&event.session_id, source, external_session_id.as_deref())
                    .ok_or_else(|| "session not found for event".to_string())?;
                let command = if event.kind == SessionEventKind::Attention {
                    attention.map(|attention| {
                        IngestCommand::Attention(AttentionCommand {
                            source,
                            external_session_id: session.external_session_id.clone(),
                            attention,
                            level: event.level,
                            summary: event.summary.clone(),
                            occurred_at_ms: event.occurred_at_ms,
                        })
                    })
                } else {
                    None
                }
                .unwrap_or_else(|| match event.kind {
                    SessionEventKind::Tool => IngestCommand::ToolEvent(ToolEventCommand {
                        source,
                        external_session_id: session.external_session_id.clone(),
                        level: event.level,
                        summary: event.summary,
                        tool_name: event.tool_name,
                        occurred_at_ms: event.occurred_at_ms,
                    }),
                    _ => IngestCommand::LifecycleEvent(LifecycleEventCommand {
                        source,
                        external_session_id: session.external_session_id.clone(),
                        level: event.level,
                        summary: event.summary,
                        occurred_at_ms: event.occurred_at_ms,
                    }),
                });
                self.core
                    .ingest(command)
                    .map(|_| ())
                    .map_err(|error| error.to_string())?;
                self.record_connector_traffic(source, event.occurred_at_ms);
                Ok(())
            }
        }
    }

    fn record_connector_traffic(&self, source: AgentSource, at_ms: i64) {
        if source == AgentSource::Unknown {
            return;
        }
        crate::commands::integration::record_connector_traffic(source, at_ms);
    }

    fn ingest_session_upsert(&self, incoming: AgentSession) -> Result<(), String> {
        let existing = self.snapshot().sessions.into_iter().find(|session| {
            session.source == incoming.source
                && session.external_session_id == incoming.external_session_id
        });
        let source = incoming.source;
        let external_session_id = incoming.external_session_id.clone();
        let occurred_at_ms = incoming.last_event_at_ms;
        let terminal = matches!(
            incoming.status,
            SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
        );
        let validated_root = if let Some(root) =
            incoming.process_root.as_ref().filter(|_| !terminal)
        {
            let observed = self
                .metrics
                .resolve_process_identity(root.pid)
                .ok_or_else(|| "process root is not running".to_string())?;
            if !notch_metrics::graph::start_times_match(root.started_at_ms, observed.started_at_ms)
            {
                return Err("process root start time does not match the live process".into());
            }
            let attribution = attribution_for_source(source);
            (attribution != AttributionQuality::Unknown).then_some((observed, attribution))
        } else {
            None
        };

        let session_id = if let Some(existing) = &existing {
            existing.id.clone()
        } else {
            let start_status = match incoming.status {
                SessionStatus::Starting | SessionStatus::Running | SessionStatus::Failed => {
                    incoming.status
                }
                _ => SessionStatus::Running,
            };
            let result = self
                .core
                .ingest(IngestCommand::SessionStart(SessionStartCommand {
                    idempotency_key: None,
                    source,
                    external_session_id: external_session_id.clone(),
                    label: incoming.label.clone(),
                    workspace_label: incoming.workspace_label.clone(),
                    status: start_status,
                    occurred_at_ms: incoming.started_at_ms,
                }))
                .map_err(|error| error.to_string())?;
            result
                .session_id
                .ok_or_else(|| "core did not return a session id".to_string())?
        };

        if let Some(existing) = &existing {
            let terminal = matches!(
                incoming.status,
                SessionStatus::Completed | SessionStatus::Failed
            );
            if !terminal {
                self.core
                    .ingest(IngestCommand::SessionUpdate(SessionUpdateCommand {
                        source,
                        external_session_id: external_session_id.clone(),
                        status: Some(incoming.status),
                        label: Some(incoming.label.clone()),
                        workspace_label: incoming.workspace_label.clone(),
                        occurred_at_ms,
                    }))
                    .map_err(|error| error.to_string())?;
            } else if !matches!(
                existing.status,
                SessionStatus::Completed | SessionStatus::Failed
            ) {
                if existing.status == SessionStatus::Starting {
                    self.core
                        .ingest(IngestCommand::SessionUpdate(SessionUpdateCommand {
                            source,
                            external_session_id: external_session_id.clone(),
                            status: Some(SessionStatus::Running),
                            label: None,
                            workspace_label: None,
                            occurred_at_ms,
                        }))
                        .map_err(|error| error.to_string())?;
                }
                self.core
                    .ingest(IngestCommand::SessionEnd(SessionEndCommand {
                        source,
                        external_session_id: external_session_id.clone(),
                        status: incoming.status,
                        occurred_at_ms,
                        summary: Some("Session ended".into()),
                    }))
                    .map_err(|error| error.to_string())?;
                self.metrics.unregister_session(&session_id);
            }
        } else if matches!(
            incoming.status,
            SessionStatus::Completed | SessionStatus::Failed
        ) {
            self.core
                .ingest(IngestCommand::SessionEnd(SessionEndCommand {
                    source,
                    external_session_id: external_session_id.clone(),
                    status: incoming.status,
                    occurred_at_ms,
                    summary: Some("Session ended".into()),
                }))
                .map_err(|error| error.to_string())?;
        } else if matches!(
            incoming.status,
            SessionStatus::Waiting | SessionStatus::Paused | SessionStatus::Stale
        ) {
            self.core
                .ingest(IngestCommand::SessionUpdate(SessionUpdateCommand {
                    source,
                    external_session_id: external_session_id.clone(),
                    status: Some(incoming.status),
                    label: None,
                    workspace_label: None,
                    occurred_at_ms,
                }))
                .map_err(|error| error.to_string())?;
        }

        if let Some((observed, attribution)) = validated_root {
            self.set_adapter_attribution(source, attribution)?;
            self.core
                .ingest(IngestCommand::RegisterProcessRoot(ProcessRootCommand {
                    source,
                    external_session_id: external_session_id.clone(),
                    process_root: observed.clone(),
                    occurred_at_ms,
                }))
                .map_err(|error| error.to_string())?;
            self.metrics
                .register_session(session_id.clone(), observed, attribution, occurred_at_ms)
                .map_err(|error| error.to_string())?;
        }

        if incoming.attention != AttentionKind::None {
            self.core
                .ingest(IngestCommand::Attention(AttentionCommand {
                    source,
                    external_session_id,
                    attention: incoming.attention,
                    level: if incoming.attention == AttentionKind::Error {
                        EventLevel::Error
                    } else {
                        EventLevel::Warning
                    },
                    summary: "Attention observed by local integration".into(),
                    occurred_at_ms,
                }))
                .map_err(|error| error.to_string())?;
        }

        self.record_connector_traffic(source, occurred_at_ms);
        self.reconcile_metric_roots()
    }

    fn resolve_session(
        &self,
        session_id: &str,
        source: AgentSource,
        external_session_id: Option<&str>,
    ) -> Option<AgentSession> {
        self.snapshot().sessions.into_iter().find(|session| {
            session.id == session_id
                || (session.source == source
                    && (session.external_session_id == session_id
                        || external_session_id
                            .map(|external| session.external_session_id == external)
                            .unwrap_or(false)))
        })
    }
}

pub fn builtin_adapter_capabilities() -> Vec<AdapterCapabilities> {
    vec![
        AdapterCapabilities::template(AgentSource::Cursor),
        AdapterCapabilities::template(AgentSource::ClaudeCode),
        AdapterCapabilities::template(AgentSource::Codex),
        AdapterCapabilities::template(AgentSource::Generic),
    ]
}

pub fn register_builtin_adapters(core: &DesktopCore) -> Result<(), String> {
    let now_ms = SystemClock.now_ms();
    for capabilities in builtin_adapter_capabilities() {
        core.ingest(IngestCommand::IntegrationHealth(IntegrationHealthCommand {
            capabilities,
            healthy: false,
            message: Some("Template available; live connector health not yet verified".into()),
            occurred_at_ms: now_ms,
        }))
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn attribution_for_source(source: AgentSource) -> AttributionQuality {
    match source {
        AgentSource::Cursor => AttributionQuality::Shared,
        AgentSource::ClaudeCode | AgentSource::Codex => AttributionQuality::Heuristic,
        AgentSource::Generic => AttributionQuality::Exact,
        AgentSource::Unknown => AttributionQuality::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> HostState {
        let stream = Arc::new(StreamHub::default());
        let repository = Arc::new(SqliteRepository::in_memory().unwrap());
        let core = Arc::new(
            AppCore::new(
                SystemClock,
                repository,
                Arc::clone(&stream),
                PublicSettings {
                    overlay_enabled: true,
                    autostart_enabled: false,
                    reduced_motion: false,
                    sampling_interval_ms: 1_000,
                    selected_display: None,
                    show_over_fullscreen: false,
                    history_retention_hours: 24,
                    alert_sound_enabled: false,
                },
            )
            .unwrap(),
        );
        register_builtin_adapters(&core).unwrap();
        HostState::new(core, MetricsEngine::new(), stream)
    }

    fn generic_session(
        external_session_id: &str,
        status: SessionStatus,
        at_ms: i64,
        process_root: Option<notch_protocol::ProcessIdentity>,
    ) -> AgentSession {
        AgentSession {
            id: "ignored-wire-id".into(),
            source: AgentSource::Generic,
            external_session_id: external_session_id.into(),
            label: "generic".into(),
            workspace_label: None,
            status,
            attention: AttentionKind::None,
            started_at_ms: at_ms,
            last_event_at_ms: at_ms,
            ended_at_ms: None,
            process_root,
            latest_metric: None,
        }
    }

    #[test]
    fn terminal_session_unregisters_and_clears_process_root() {
        let state = test_state();
        let root = state
            .metrics
            .resolve_process_identity(std::process::id())
            .unwrap();
        state
            .ingest_session_upsert(generic_session(
                "lifecycle",
                SessionStatus::Running,
                SystemClock.now_ms(),
                Some(root),
            ))
            .unwrap();
        assert_eq!(state.metrics.registered_session_ids().len(), 1);

        state
            .ingest_session_upsert(generic_session(
                "lifecycle",
                SessionStatus::Completed,
                SystemClock.now_ms(),
                None,
            ))
            .unwrap();
        assert!(state.metrics.registered_session_ids().is_empty());
        let session = state
            .snapshot()
            .sessions
            .into_iter()
            .find(|session| session.external_session_id == "lifecycle")
            .unwrap();
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(session.process_root.is_none());
    }

    #[test]
    fn stale_maintenance_unregisters_process_root() {
        let state = test_state();
        let root = state
            .metrics
            .resolve_process_identity(std::process::id())
            .unwrap();
        let old = SystemClock.now_ms() - notch_core::STALE_SESSION_MS - 1;
        state
            .ingest_session_upsert(generic_session(
                "stale-root",
                SessionStatus::Running,
                old,
                Some(root),
            ))
            .unwrap();
        assert_eq!(state.metrics.registered_session_ids().len(), 1);

        state.core.tick().unwrap();
        state.reconcile_metric_roots().unwrap();
        assert!(state.metrics.registered_session_ids().is_empty());
        let session = state
            .snapshot()
            .sessions
            .into_iter()
            .find(|session| session.external_session_id == "stale-root")
            .unwrap();
        assert_eq!(session.status, SessionStatus::Stale);
        assert!(session.process_root.is_none());
    }

    #[test]
    fn mismatched_process_identity_is_rejected_without_attribution_claim() {
        let state = test_state();
        let mut root = state
            .metrics
            .resolve_process_identity(std::process::id())
            .unwrap();
        root.started_at_ms += 10_000;
        let error = state
            .ingest_session_upsert(generic_session(
                "bad-root",
                SessionStatus::Running,
                SystemClock.now_ms(),
                Some(root),
            ))
            .unwrap_err();
        assert!(error.contains("start time"));
        assert!(state.metrics.registered_session_ids().is_empty());
        assert!(
            state
                .snapshot()
                .sessions
                .iter()
                .all(|session| session.external_session_id != "bad-root")
        );
        let generic = state
            .snapshot()
            .adapters
            .into_iter()
            .find(|adapter| adapter.source == AgentSource::Generic)
            .unwrap();
        assert_eq!(generic.process_attribution, AttributionQuality::Unknown);
    }

    #[test]
    fn metrics_pause_state_is_authoritative_across_unrelated_actions() {
        let state = test_state();
        assert!(!state.metrics_paused());
        state.set_metrics_paused(true);
        assert!(state.metrics_paused());
        let _ = state.snapshot();
        assert!(state.metrics_paused());
        state.set_metrics_paused(false);
        assert!(!state.metrics_paused());
    }
}
