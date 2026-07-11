use std::sync::Arc;

use notch_protocol::{
    AdapterCapabilities, AgentSession, AppSnapshot, AttentionCapability, AttentionKind,
    AttributionQuality, EventLevel, MetricsFrame, PROTOCOL_VERSION, PublicSettings, SessionEvent,
    SessionStatus, StreamFrame, StreamPayload,
};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::alerts::AlertEvaluator;
use crate::constants::{MAX_SESSIONS, METRIC_BUCKET_SECS, STALE_SESSION_MS};
use crate::domain::{
    AcknowledgeAttentionCommand, AttentionCommand, IngestCommand, IngestResult,
    IntegrationHealthCommand, LifecycleEventCommand, ProcessRootCommand, SessionEndCommand,
    SessionStartCommand, SessionUpdateCommand, ToolEventCommand, attention_kind, lifecycle_kind,
    status_kind, tool_kind, validate_event_summary, validate_external_session_id,
    validate_session_id, validate_session_label, validate_tool_name, validate_transition,
    validate_workspace_label,
};
use crate::error::{CoreError, CoreResult};
use crate::persistence::PurgeReport;
use crate::registry::SessionRegistry;
use crate::stream::StreamCoalescer;
use crate::traits::{Clock, SessionRepository, StreamSink};

struct AppCoreInner {
    registry: SessionRegistry,
    settings: PublicSettings,
    integrations: Vec<AdapterCapabilities>,
    stream_sequence: u64,
    coalescer: StreamCoalescer,
    alerts: AlertEvaluator,
    latest_metrics: Option<MetricsFrame>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotWithSequence {
    pub snapshot: AppSnapshot,
    pub sequence: u64,
    pub events: Vec<SessionEvent>,
}

/// Thread-safe application core orchestrating ingest, persistence, alerts, and streams.
pub struct AppCore<C: Clock, R: SessionRepository, S: StreamSink> {
    clock: C,
    repository: Arc<R>,
    stream: Arc<S>,
    inner: RwLock<AppCoreInner>,
}

impl<C: Clock, R: SessionRepository, S: StreamSink> AppCore<C, R, S> {
    pub fn new(
        clock: C,
        repository: Arc<R>,
        stream: Arc<S>,
        settings: PublicSettings,
    ) -> CoreResult<Self> {
        let mut registry = SessionRegistry::new();
        registry.restore(repository.load_sessions()?, repository.load_events()?)?;

        let integrations = repository.load_integrations()?;
        let persisted_settings = repository.load_settings()?.unwrap_or(settings);

        Ok(Self {
            clock,
            repository,
            stream,
            inner: RwLock::new(AppCoreInner {
                registry,
                settings: persisted_settings,
                integrations,
                stream_sequence: 0,
                coalescer: StreamCoalescer::new(),
                alerts: AlertEvaluator::new(),
                latest_metrics: None,
            }),
        })
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    pub fn settings(&self) -> PublicSettings {
        self.inner.read().settings.clone()
    }

    pub fn update_settings(&self, settings: PublicSettings) -> CoreResult<()> {
        self.repository.save_settings(&settings)?;
        let mut inner = self.inner.write();
        inner.settings = settings.clone();
        self.queue_payload(&mut inner, StreamPayload::SettingsChanged { settings });
        self.flush_pending(&mut inner);
        Ok(())
    }

    pub fn ingest(&self, command: IngestCommand) -> CoreResult<IngestResult> {
        let mut inner = self.inner.write();
        let result = self.handle_ingest(&mut inner, command)?;
        self.flush_pending(&mut inner);
        Ok(result)
    }

    pub fn record_metrics(&self, frame: MetricsFrame) -> CoreResult<()> {
        let now = self.clock.now_ms();
        let bucket = bucket_start(now);

        self.repository
            .record_metric_bucket("host", None, bucket, &host_sample(&frame))?;
        self.repository.record_metric_bucket(
            "aggregate",
            None,
            bucket,
            &aggregate_sample(&frame),
        )?;
        for (session_id, sample) in &frame.agents {
            self.repository
                .record_metric_bucket("agent", Some(session_id), bucket, sample)?;
        }

        let mut inner = self.inner.write();
        let session_ids = inner
            .registry
            .sessions()
            .map(|session| session.id.clone())
            .collect::<Vec<_>>();
        for session_id in session_ids {
            if let Some(session) = inner.registry.get_mut(&session_id) {
                let terminal = matches!(
                    session.status,
                    SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
                );
                if !terminal {
                    session.latest_metric = frame.agents.get(&session_id).cloned();
                } else {
                    session.latest_metric = None;
                }
                self.repository.upsert_session(session)?;
            }
        }

        inner.latest_metrics = Some(frame.clone());
        inner
            .alerts
            .evaluate_host_metrics(&frame.host, &aggregate_sample(&frame), now);
        self.queue_payload(&mut inner, StreamPayload::Metrics { metrics: frame });
        self.flush_pending(&mut inner);
        Ok(())
    }

    pub fn snapshot(&self) -> AppSnapshot {
        self.snapshot_with_sequence().snapshot
    }

    /// Captures snapshot state, stream sequence, and retained events under the
    /// same read lock so a bootstrap cursor can never advance past its data.
    pub fn snapshot_with_sequence(&self) -> SnapshotWithSequence {
        let inner = self.inner.read();
        let mut sessions: Vec<_> = inner.registry.sessions().cloned().collect();
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_event_at_ms));
        sessions.truncate(MAX_SESSIONS);

        SnapshotWithSequence {
            snapshot: AppSnapshot {
                protocol_version: PROTOCOL_VERSION,
                captured_at_ms: self.clock.now_ms(),
                host: inner.latest_metrics.as_ref().map(|m| m.host.clone()),
                aggregate: inner.latest_metrics.as_ref().map(|m| m.aggregate.clone()),
                sessions,
                settings: inner.settings.clone(),
                adapters: inner.integrations.clone(),
            },
            sequence: inner.stream_sequence,
            events: inner.registry.bootstrap_events(),
        }
    }

    pub fn tick(&self) -> CoreResult<()> {
        let now = self.clock.now_ms();
        let mut inner = self.inner.write();
        self.mark_stale_sessions(&mut inner, now)?;
        self.flush_pending(&mut inner);
        Ok(())
    }

    pub fn purge_history(&self) -> CoreResult<PurgeReport> {
        let retention_ms =
            i64::from(self.settings().history_retention_hours).saturating_mul(60 * 60 * 1_000);
        let report = self.repository.prune(self.clock.now_ms(), retention_ms)?;
        if report.metric_buckets_deleted > 0 {
            let mut inner = self.inner.write();
            inner.registry.clear_latest_metrics();
            inner.latest_metrics = None;
        }
        Ok(report)
    }

    pub fn purge_metric_history(&self) -> CoreResult<u64> {
        let removed = self.repository.purge_metric_history()?;
        let mut inner = self.inner.write();
        inner.registry.clear_latest_metrics();
        inner.latest_metrics = None;
        Ok(removed)
    }

    pub fn metric_history(
        &self,
        session_id: &str,
        limit: usize,
    ) -> CoreResult<Vec<notch_protocol::MetricSample>> {
        validate_session_id(session_id)?;
        self.repository.load_metric_history(session_id, limit)
    }

    pub fn persisted_metric_history(
        &self,
        requested_start_ms: i64,
        requested_end_ms: i64,
        max_points_per_series: usize,
    ) -> CoreResult<crate::PersistedMetricHistory> {
        self.repository.load_persisted_metric_history(
            requested_start_ms,
            requested_end_ms,
            max_points_per_series,
        )
    }

    pub fn session_events(&self, session_id: &str) -> CoreResult<Vec<SessionEvent>> {
        validate_session_id(session_id)?;
        let inner = self.inner.read();
        if inner.registry.get(session_id).is_none() {
            return Err(CoreError::SessionNotFound(session_id.to_string()));
        }
        Ok(inner.registry.events_for(session_id).to_vec())
    }

    pub fn session_event_page(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> CoreResult<crate::SessionEventPage> {
        validate_session_id(session_id)?;
        if limit == 0 || limit > crate::MAX_EVENT_PAGE_SIZE {
            return Err(CoreError::Validation(format!(
                "event page limit must be 1..={}",
                crate::MAX_EVENT_PAGE_SIZE
            )));
        }
        if before_sequence == Some(0) {
            return Err(CoreError::Validation(
                "before_sequence must be greater than zero".into(),
            ));
        }
        if self.inner.read().registry.get(session_id).is_none() {
            return Err(CoreError::SessionNotFound(session_id.to_string()));
        }
        self.repository
            .load_session_event_page(session_id, before_sequence, limit)
    }

    pub fn remove_session(&self, session_id: &str) -> CoreResult<()> {
        validate_session_id(session_id)?;
        let mut inner = self.inner.write();
        if inner.registry.get(session_id).is_none() {
            return Err(CoreError::SessionNotFound(session_id.to_string()));
        }
        self.repository.remove_session(session_id)?;
        inner.registry.remove_session(session_id)?;
        self.queue_payload(
            &mut inner,
            StreamPayload::SessionRemove {
                session_id: session_id.to_string(),
            },
        );
        self.flush_pending(&mut inner);
        Ok(())
    }

    pub fn clear_process_root(&self, session_id: &str) -> CoreResult<()> {
        validate_session_id(session_id)?;
        let mut inner = self.inner.write();
        let session = inner
            .registry
            .get_mut(session_id)
            .ok_or_else(|| CoreError::SessionNotFound(session_id.to_string()))?;
        if session.process_root.is_none() {
            return Ok(());
        }
        session.process_root = None;
        session.latest_metric = None;
        self.repository.upsert_session(session)?;
        let updated = session.clone();
        self.queue_payload(
            &mut inner,
            StreamPayload::SessionUpsert { session: updated },
        );
        self.flush_pending(&mut inner);
        Ok(())
    }

    pub fn heartbeat(&self) {
        let mut inner = self.inner.write();
        self.queue_payload(&mut inner, StreamPayload::Heartbeat);
        self.flush_pending(&mut inner);
    }

    pub fn stream_since(&self, sequence: u64) -> Vec<StreamFrame> {
        self.stream.stream_since(sequence)
    }

    pub fn latest_stream_sequence(&self) -> u64 {
        self.stream.latest_sequence()
    }

    fn handle_ingest(
        &self,
        inner: &mut AppCoreInner,
        command: IngestCommand,
    ) -> CoreResult<IngestResult> {
        match command {
            IngestCommand::SessionStart(cmd) => self.session_start(inner, cmd),
            IngestCommand::SessionUpdate(cmd) => self.session_update(inner, cmd),
            IngestCommand::SessionEnd(cmd) => self.session_end(inner, cmd),
            IngestCommand::LifecycleEvent(cmd) => self.lifecycle_event(inner, cmd),
            IngestCommand::ToolEvent(cmd) => self.tool_event(inner, cmd),
            IngestCommand::Attention(cmd) => self.attention(inner, cmd),
            IngestCommand::RegisterProcessRoot(cmd) => self.register_process_root(inner, cmd),
            IngestCommand::IntegrationHealth(cmd) => self.integration_health(inner, cmd),
            IngestCommand::AcknowledgeAttention(cmd) => self.acknowledge_attention(inner, cmd),
        }
    }

    fn session_start(
        &self,
        inner: &mut AppCoreInner,
        cmd: SessionStartCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        validate_session_label(&cmd.label)?;
        if let Some(label) = &cmd.workspace_label {
            validate_workspace_label(label)?;
        }

        if let Some(key) = &cmd.idempotency_key {
            if inner.registry.check_idempotency(key)
                || self
                    .repository
                    .store_idempotency_key(key, cmd.occurred_at_ms)?
            {
                if let Some(session) = inner
                    .registry
                    .resolve_external(cmd.source, &cmd.external_session_id)
                {
                    return Ok(IngestResult {
                        session_id: Some(session.id.clone()),
                        event_id: None,
                        idempotent_replay: true,
                    });
                }
            }
            inner.registry.record_idempotency(key.clone());
        }

        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if !capabilities.events {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support events".into(),
            ));
        }

        let status = if cmd.status == SessionStatus::Starting {
            SessionStatus::Starting
        } else {
            validate_transition(SessionStatus::Starting, cmd.status)?;
            cmd.status
        };

        if let Some(existing) = inner
            .registry
            .resolve_external(cmd.source, &cmd.external_session_id)
        {
            return Ok(IngestResult {
                session_id: Some(existing.id.clone()),
                event_id: None,
                idempotent_replay: true,
            });
        }

        if let Some(evicted_id) = inner.registry.capacity_eviction_candidate()? {
            self.repository.remove_session(&evicted_id)?;
            inner.registry.remove_session(&evicted_id)?;
            self.queue_payload(
                inner,
                StreamPayload::SessionRemove {
                    session_id: evicted_id,
                },
            );
        }

        let (mut session, replay) =
            inner
                .registry
                .insert_new_session(cmd.source, &cmd.external_session_id, |id| AgentSession {
                    id,
                    source: cmd.source,
                    external_session_id: cmd.external_session_id.clone(),
                    label: cmd.label.clone(),
                    workspace_label: cmd.workspace_label.clone(),
                    status,
                    attention: AttentionKind::None,
                    started_at_ms: cmd.occurred_at_ms,
                    last_event_at_ms: cmd.occurred_at_ms,
                    ended_at_ms: None,
                    process_root: None,
                    latest_metric: None,
                })?;

        if replay {
            return Ok(IngestResult {
                session_id: Some(session.id.clone()),
                event_id: None,
                idempotent_replay: true,
            });
        }

        self.repository.upsert_session(&session)?;
        let event = self.append_event(
            inner,
            &session.id,
            lifecycle_kind(),
            EventLevel::Info,
            "Session started",
            None,
            cmd.occurred_at_ms,
        )?;
        session.last_event_at_ms = cmd.occurred_at_ms;
        inner.registry.upsert_session(session.clone())?;
        self.repository.upsert_session(&session)?;
        self.queue_payload(inner, StreamPayload::SessionUpsert { session });

        Ok(IngestResult {
            session_id: Some(event.session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn session_update(
        &self,
        inner: &mut AppCoreInner,
        cmd: SessionUpdateCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if !capabilities.events {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support events".into(),
            ));
        }

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        let session = inner
            .registry
            .get_mut(&session_id)
            .expect("resolved session");

        if let Some(label) = &cmd.label {
            validate_session_label(label)?;
            session.label = label.clone();
        }
        if let Some(workspace) = &cmd.workspace_label {
            validate_workspace_label(workspace)?;
            session.workspace_label = Some(workspace.clone());
        }
        if let Some(status) = cmd.status {
            validate_transition(session.status, status)?;
            session.status = status;
        }
        session.last_event_at_ms = cmd.occurred_at_ms;

        self.repository.upsert_session(session)?;
        let updated = session.clone();
        let event = self.append_event(
            inner,
            &session_id,
            status_kind(),
            EventLevel::Info,
            "Session updated",
            None,
            cmd.occurred_at_ms,
        )?;
        self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn session_end(
        &self,
        inner: &mut AppCoreInner,
        cmd: SessionEndCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if !capabilities.events {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support events".into(),
            ));
        }

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        let session = inner
            .registry
            .get_mut(&session_id)
            .expect("resolved session");

        if !matches!(cmd.status, SessionStatus::Completed | SessionStatus::Failed) {
            return Err(CoreError::Validation(
                "session end status must be completed or failed".into(),
            ));
        }
        validate_transition(session.status, cmd.status)?;
        session.status = cmd.status;
        session.ended_at_ms = Some(cmd.occurred_at_ms);
        session.last_event_at_ms = cmd.occurred_at_ms;
        session.attention = AttentionKind::None;
        session.latest_metric = None;

        self.repository.upsert_session(session)?;
        let updated = session.clone();
        let summary = cmd.summary.unwrap_or_else(|| "Session ended".into());
        validate_event_summary(&summary)?;
        let event = self.append_event(
            inner,
            &session_id,
            lifecycle_kind(),
            EventLevel::Info,
            &summary,
            None,
            cmd.occurred_at_ms,
        )?;
        self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn lifecycle_event(
        &self,
        inner: &mut AppCoreInner,
        cmd: LifecycleEventCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        validate_event_summary(&cmd.summary)?;
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if !capabilities.events {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support events".into(),
            ));
        }

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        if let Some(session) = inner.registry.get_mut(&session_id) {
            session.last_event_at_ms = cmd.occurred_at_ms;
            self.repository.upsert_session(session)?;
        }

        let event = self.append_event(
            inner,
            &session_id,
            lifecycle_kind(),
            cmd.level,
            &cmd.summary,
            None,
            cmd.occurred_at_ms,
        )?;

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn tool_event(
        &self,
        inner: &mut AppCoreInner,
        cmd: ToolEventCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        validate_event_summary(&cmd.summary)?;
        if let Some(tool) = &cmd.tool_name {
            validate_tool_name(tool)?;
        }
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if !capabilities.events {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support events".into(),
            ));
        }

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        if let Some(session) = inner.registry.get_mut(&session_id) {
            session.last_event_at_ms = cmd.occurred_at_ms;
            self.repository.upsert_session(session)?;
        }

        let event = self.append_event(
            inner,
            &session_id,
            tool_kind(),
            cmd.level,
            &cmd.summary,
            cmd.tool_name.as_deref(),
            cmd.occurred_at_ms,
        )?;

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn attention(
        &self,
        inner: &mut AppCoreInner,
        cmd: AttentionCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        validate_event_summary(&cmd.summary)?;
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        self.ensure_attention_capability(&capabilities, cmd.attention)?;

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        let session = inner
            .registry
            .get_mut(&session_id)
            .expect("resolved session");
        session.attention = cmd.attention;
        session.last_event_at_ms = cmd.occurred_at_ms;
        self.repository.upsert_session(session)?;
        let updated = session.clone();

        inner
            .alerts
            .evaluate_attention(&session_id, cmd.attention, cmd.occurred_at_ms);

        let event = self.append_event(
            inner,
            &session_id,
            attention_kind(),
            cmd.level,
            &cmd.summary,
            None,
            cmd.occurred_at_ms,
        )?;
        self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn register_process_root(
        &self,
        inner: &mut AppCoreInner,
        cmd: ProcessRootCommand,
    ) -> CoreResult<IngestResult> {
        validate_external_session_id(&cmd.external_session_id)?;
        let capabilities = self.capabilities_for(inner, cmd.source)?;
        if capabilities.process_attribution == AttributionQuality::Unknown {
            return Err(CoreError::UnsupportedCapability(
                "integration does not support process attribution".into(),
            ));
        }

        let session_id = self.resolve_session_id(inner, cmd.source, &cmd.external_session_id)?;
        let session = inner
            .registry
            .get_mut(&session_id)
            .expect("resolved session");
        session.process_root = Some(cmd.process_root.clone());
        session.last_event_at_ms = cmd.occurred_at_ms;
        self.repository.upsert_session(session)?;
        let updated = session.clone();

        let event = self.append_event(
            inner,
            &session_id,
            lifecycle_kind(),
            EventLevel::Info,
            "Process root registered",
            None,
            cmd.occurred_at_ms,
        )?;
        self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });

        Ok(IngestResult {
            session_id: Some(session_id),
            event_id: Some(event.id),
            idempotent_replay: false,
        })
    }

    fn integration_health(
        &self,
        inner: &mut AppCoreInner,
        cmd: IntegrationHealthCommand,
    ) -> CoreResult<IngestResult> {
        self.repository.upsert_integration(
            &cmd.capabilities,
            cmd.healthy,
            cmd.message.as_deref(),
            cmd.occurred_at_ms,
        )?;
        if let Some(existing) = inner
            .integrations
            .iter_mut()
            .find(|c| c.source == cmd.capabilities.source)
        {
            *existing = cmd.capabilities.clone();
        } else {
            inner.integrations.push(cmd.capabilities.clone());
        }
        self.queue_payload(
            inner,
            StreamPayload::IntegrationChanged {
                integration: cmd.capabilities,
            },
        );
        Ok(IngestResult::empty())
    }

    fn acknowledge_attention(
        &self,
        inner: &mut AppCoreInner,
        cmd: AcknowledgeAttentionCommand,
    ) -> CoreResult<IngestResult> {
        validate_session_id(&cmd.session_id)?;
        let session = inner
            .registry
            .get_mut(&cmd.session_id)
            .ok_or_else(|| CoreError::SessionNotFound(cmd.session_id.clone()))?;
        session.attention = AttentionKind::None;
        session.last_event_at_ms = cmd.occurred_at_ms;
        self.repository.upsert_session(session)?;
        inner.alerts.clear_local_acknowledgement(&cmd.session_id);
        let updated = session.clone();
        self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });
        Ok(IngestResult {
            session_id: Some(cmd.session_id),
            event_id: None,
            idempotent_replay: false,
        })
    }

    fn append_event(
        &self,
        inner: &mut AppCoreInner,
        session_id: &str,
        kind: notch_protocol::SessionEventKind,
        level: EventLevel,
        summary: &str,
        tool_name: Option<&str>,
        occurred_at_ms: i64,
    ) -> CoreResult<SessionEvent> {
        let sequence = inner.registry.next_event_sequence(session_id);
        let event = SessionEvent {
            id: Uuid::new_v4(),
            session_id: session_id.to_string(),
            sequence,
            occurred_at_ms,
            kind,
            level,
            summary: summary.to_string(),
            tool_name: tool_name.map(str::to_string),
        };
        inner.registry.append_event(event.clone())?;
        self.repository.append_event(&event)?;
        self.queue_payload(
            inner,
            StreamPayload::SessionEvent {
                event: event.clone(),
            },
        );
        Ok(event)
    }

    fn resolve_session_id(
        &self,
        inner: &AppCoreInner,
        source: notch_protocol::AgentSource,
        external_session_id: &str,
    ) -> CoreResult<String> {
        inner
            .registry
            .resolve_external(source, external_session_id)
            .map(|s| s.id.clone())
            .ok_or_else(|| CoreError::SessionNotFound(format!("{source:?}:{external_session_id}")))
    }

    fn capabilities_for(
        &self,
        inner: &AppCoreInner,
        source: notch_protocol::AgentSource,
    ) -> CoreResult<AdapterCapabilities> {
        inner
            .integrations
            .iter()
            .find(|c| c.source == source)
            .cloned()
            .ok_or_else(|| {
                CoreError::UnsupportedCapability(format!(
                    "no integration registered for {source:?}"
                ))
            })
    }

    fn ensure_attention_capability(
        &self,
        capabilities: &AdapterCapabilities,
        attention: AttentionKind,
    ) -> CoreResult<()> {
        match capabilities.attention {
            AttentionCapability::Full => Ok(()),
            AttentionCapability::Partial if attention != AttentionKind::None => Ok(()),
            AttentionCapability::None if attention == AttentionKind::None => Ok(()),
            _ => Err(CoreError::UnsupportedCapability(
                "integration does not support attention reporting".into(),
            )),
        }
    }

    fn mark_stale_sessions(&self, inner: &mut AppCoreInner, now_ms: i64) -> CoreResult<()> {
        let stale_ids: Vec<_> = inner
            .registry
            .sessions()
            .filter(|session| {
                !matches!(
                    session.status,
                    SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Stale
                ) && now_ms.saturating_sub(session.last_event_at_ms) >= STALE_SESSION_MS
            })
            .map(|session| session.id.clone())
            .collect();

        for session_id in stale_ids {
            let session = inner.registry.get_mut(&session_id).expect("stale session");
            if validate_transition(session.status, SessionStatus::Stale).is_ok() {
                session.status = SessionStatus::Stale;
                session.last_event_at_ms = now_ms;
                session.latest_metric = None;
                self.repository.upsert_session(session)?;
                let updated = session.clone();
                self.queue_payload(inner, StreamPayload::SessionUpsert { session: updated });
            }
        }
        Ok(())
    }

    fn queue_payload(&self, inner: &mut AppCoreInner, payload: StreamPayload) {
        inner.coalescer.push(payload);
    }

    fn flush_pending(&self, inner: &mut AppCoreInner) {
        if inner.coalescer.is_empty() {
            return;
        }
        let now = self.clock.now_ms();
        for payload in inner.coalescer.drain() {
            inner.stream_sequence += 1;
            let frame = StreamFrame {
                sequence: inner.stream_sequence,
                emitted_at_ms: now,
                payload,
            };
            self.stream.emit(frame);
        }
    }
}

impl<C: Clock, R: SessionRepository, S: StreamSink> crate::traits::MetricsCoreHandle
    for AppCore<C, R, S>
{
    fn record_metrics(&self, frame: MetricsFrame) -> CoreResult<()> {
        self.record_metrics(frame)
    }
}

fn bucket_start(at_ms: i64) -> i64 {
    at_ms - (at_ms.rem_euclid(METRIC_BUCKET_SECS * 1_000))
}

fn aggregate_sample(frame: &MetricsFrame) -> notch_protocol::MetricSample {
    let aggregate = &frame.aggregate;
    notch_protocol::MetricSample {
        at_ms: aggregate.at_ms,
        cpu_core_percent: aggregate.cpu_core_percent,
        cpu_host_percent: aggregate.cpu_host_percent,
        rss_bytes: aggregate.rss_bytes,
        runtime_ms: aggregate.runtime_ms,
        process_count: aggregate.process_count,
        read_bytes_per_sec: aggregate.read_bytes_per_sec,
        write_bytes_per_sec: aggregate.write_bytes_per_sec,
        quality: aggregate.quality.clone(),
    }
}

fn host_sample(frame: &MetricsFrame) -> notch_protocol::MetricSample {
    notch_protocol::MetricSample {
        at_ms: frame.host.at_ms,
        cpu_core_percent: frame.host.cpu_host_percent,
        cpu_host_percent: frame.host.cpu_host_percent,
        rss_bytes: frame.host.used_memory_bytes,
        runtime_ms: 0,
        process_count: frame.host.visible_process_count,
        read_bytes_per_sec: frame.host.disk_read_bytes_per_sec,
        write_bytes_per_sec: frame.host.disk_write_bytes_per_sec,
        quality: notch_protocol::MetricQuality {
            attribution: AttributionQuality::Unknown,
            cpu: notch_protocol::MetricAvailability::Available,
            io: notch_protocol::IoQuality::Disk,
            reason: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_EVENTS_PER_SESSION;
    use crate::domain::{
        IngestCommand, IntegrationHealthCommand, SessionStartCommand, ToolEventCommand,
    };
    use crate::persistence::SqliteRepository;
    use crate::registry::SessionRegistry;
    use crate::traits::VecStreamSink;
    use notch_protocol::{
        AgentAggregate, AgentSource, AttributionQuality, HostMetricSample, IoQuality,
        MetricAvailability, MetricQuality, MetricSample, SessionEventKind,
    };
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Barrier, Mutex};
    use std::time::Duration;

    #[derive(Debug, Clone)]
    struct ManualClock(i64);

    impl Clock for ManualClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    fn default_settings() -> PublicSettings {
        PublicSettings {
            overlay_enabled: true,
            autostart_enabled: false,
            reduced_motion: false,
            sampling_interval_ms: 1_000,
            selected_display: None,
            show_over_fullscreen: false,
            history_retention_hours: 24,
        }
    }

    fn register_cursor<R: SessionRepository, S: StreamSink>(inner: &AppCore<ManualClock, R, S>) {
        let mut capabilities = AdapterCapabilities::template(AgentSource::Cursor);
        capabilities.attention = AttentionCapability::Partial;
        capabilities.context_open = true;
        capabilities.process_attribution = AttributionQuality::Shared;
        inner
            .ingest(IngestCommand::IntegrationHealth(IntegrationHealthCommand {
                capabilities,
                healthy: true,
                message: None,
                occurred_at_ms: 0,
            }))
            .unwrap();
    }

    fn test_core() -> AppCore<ManualClock, SqliteRepository, VecStreamSink> {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(ManualClock(1_000), repo, stream, default_settings()).unwrap();
        register_cursor(&core);
        core
    }

    #[test]
    fn rejects_invalid_status_transition() {
        let core = test_core();
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "ext".into(),
            label: "label".into(),
            workspace_label: None,
            status: SessionStatus::Starting,
            occurred_at_ms: 1,
        }))
        .unwrap();

        let err = core
            .ingest(IngestCommand::SessionUpdate(
                crate::domain::SessionUpdateCommand {
                    source: AgentSource::Cursor,
                    external_session_id: "ext".into(),
                    status: Some(SessionStatus::Completed),
                    label: None,
                    workspace_label: None,
                    occurred_at_ms: 2,
                },
            ))
            .unwrap_err();
        assert!(matches!(err, CoreError::InvalidTransition { .. }));
    }

    #[test]
    fn session_start_is_idempotent_by_external_key() {
        let core = test_core();
        let first = core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: Some("req-1".into()),
            source: AgentSource::Cursor,
            external_session_id: "ext".into(),
            label: "label".into(),
            workspace_label: None,
            status: SessionStatus::Starting,
            occurred_at_ms: 1,
        }));
        let second = core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: Some("req-1".into()),
            source: AgentSource::Cursor,
            external_session_id: "ext".into(),
            label: "label".into(),
            workspace_label: None,
            status: SessionStatus::Starting,
            occurred_at_ms: 2,
        }));
        assert!(!first.unwrap().idempotent_replay);
        assert!(second.unwrap().idempotent_replay);
    }

    #[test]
    fn rejects_events_without_capability() {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(ManualClock(0), repo, stream, default_settings()).unwrap();
        let mut capabilities = AdapterCapabilities::template(AgentSource::Codex);
        capabilities.events = false;
        core.ingest(IngestCommand::IntegrationHealth(IntegrationHealthCommand {
            capabilities,
            healthy: true,
            message: None,
            occurred_at_ms: 0,
        }))
        .unwrap();

        let err = core
            .ingest(IngestCommand::SessionStart(SessionStartCommand {
                idempotency_key: None,
                source: AgentSource::Codex,
                external_session_id: "x".into(),
                label: "l".into(),
                workspace_label: None,
                status: SessionStatus::Starting,
                occurred_at_ms: 1,
            }))
            .unwrap_err();
        assert!(matches!(err, CoreError::UnsupportedCapability(_)));
    }

    #[test]
    fn bounded_sessions_enforced() {
        let core = test_core();
        for i in 0..MAX_SESSIONS {
            core.ingest(IngestCommand::SessionStart(SessionStartCommand {
                idempotency_key: None,
                source: AgentSource::Cursor,
                external_session_id: format!("ext-{i}"),
                label: format!("label-{i}"),
                workspace_label: None,
                status: SessionStatus::Running,
                occurred_at_ms: i as i64,
            }))
            .unwrap();
            core.ingest(IngestCommand::SessionEnd(
                crate::domain::SessionEndCommand {
                    source: AgentSource::Cursor,
                    external_session_id: format!("ext-{i}"),
                    status: SessionStatus::Completed,
                    occurred_at_ms: i as i64 + 1,
                    summary: None,
                },
            ))
            .unwrap();
        }

        for i in 0..MAX_SESSIONS {
            core.ingest(IngestCommand::SessionStart(SessionStartCommand {
                idempotency_key: None,
                source: AgentSource::Cursor,
                external_session_id: format!("new-{i}"),
                label: "new".into(),
                workspace_label: None,
                status: SessionStatus::Starting,
                occurred_at_ms: 10_000 + i as i64,
            }))
            .unwrap();
        }

        let captured = core.snapshot_with_sequence();
        assert_eq!(captured.snapshot.sessions.len(), MAX_SESSIONS);
        assert!(captured.events.len() <= crate::BOOTSTRAP_MAX_EVENTS);
        let serialized =
            serde_json::to_vec(&(captured.snapshot, captured.sequence, captured.events)).unwrap();
        assert!(
            serialized.len() < 512 * 1024,
            "max-load bootstrap should remain bounded, got {} bytes",
            serialized.len()
        );
    }

    #[test]
    fn restart_restore_preserves_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core.db");
        let session_id;
        {
            let repo = Arc::new(SqliteRepository::open(&path).unwrap());
            let stream = Arc::new(VecStreamSink::new());
            let core = AppCore::new(ManualClock(1), repo, stream, default_settings()).unwrap();
            register_cursor(&core);
            let result = core
                .ingest(IngestCommand::SessionStart(SessionStartCommand {
                    idempotency_key: None,
                    source: AgentSource::Cursor,
                    external_session_id: "restore".into(),
                    label: "restore".into(),
                    workspace_label: None,
                    status: SessionStatus::Running,
                    occurred_at_ms: 5,
                }))
                .unwrap();
            session_id = result.session_id.unwrap();
        }

        let repo = Arc::new(SqliteRepository::open(&path).unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(ManualClock(2), repo, stream, default_settings()).unwrap();
        assert!(core.snapshot().sessions.iter().any(|s| s.id == session_id));
    }

    #[test]
    fn stream_frames_are_sequenced() {
        let core = test_core();
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "seq".into(),
            label: "seq".into(),
            workspace_label: None,
            status: SessionStatus::Starting,
            occurred_at_ms: 1,
        }))
        .unwrap();
        let frames = core.stream_since(0);
        assert!(!frames.is_empty());
        assert_eq!(frames[0].sequence, 1);
        if frames.len() > 1 {
            assert!(frames[1].sequence > frames[0].sequence);
        }
    }

    #[test]
    fn tool_event_respects_bounds() {
        let core = test_core();
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "tool".into(),
            label: "tool".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 1,
        }))
        .unwrap();

        let long = "x".repeat(notch_protocol::MAX_TOOL_NAME_LEN + 1);
        let err = core
            .ingest(IngestCommand::ToolEvent(ToolEventCommand {
                source: AgentSource::Cursor,
                external_session_id: "tool".into(),
                level: EventLevel::Info,
                summary: "tool".into(),
                tool_name: Some(long),
                occurred_at_ms: 2,
            }))
            .unwrap_err();
        assert!(matches!(err, CoreError::Validation(_)));
    }

    #[test]
    fn rejects_process_root_without_attribution() {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(ManualClock(0), repo, stream, default_settings()).unwrap();
        core.ingest(IngestCommand::IntegrationHealth(IntegrationHealthCommand {
            capabilities: AdapterCapabilities::template(AgentSource::Cursor),
            healthy: true,
            message: None,
            occurred_at_ms: 0,
        }))
        .unwrap();

        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "proc".into(),
            label: "proc".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 1,
        }))
        .unwrap();

        let err = core
            .ingest(IngestCommand::RegisterProcessRoot(
                crate::domain::ProcessRootCommand {
                    source: AgentSource::Cursor,
                    external_session_id: "proc".into(),
                    process_root: notch_protocol::ProcessIdentity {
                        pid: 1,
                        started_at_ms: 1,
                    },
                    occurred_at_ms: 2,
                },
            ))
            .unwrap_err();
        assert!(matches!(err, CoreError::UnsupportedCapability(_)));
    }

    #[test]
    fn stale_sessions_are_marked_on_tick() {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(
            ManualClock(STALE_SESSION_MS + 100),
            repo,
            stream,
            default_settings(),
        )
        .unwrap();
        register_cursor(&core);
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "stale".into(),
            label: "stale".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 0,
        }))
        .unwrap();

        core.tick().unwrap();
        let session = &core.snapshot().sessions[0];
        assert_eq!(session.status, SessionStatus::Stale);
    }

    #[test]
    fn event_count_is_bounded_per_session() {
        let mut registry = SessionRegistry::new();
        let session = AgentSession {
            id: "s1".into(),
            source: AgentSource::Cursor,
            external_session_id: "e1".into(),
            label: "l".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 0,
            last_event_at_ms: 0,
            ended_at_ms: None,
            process_root: None,
            latest_metric: None,
        };
        registry.upsert_session(session).unwrap();

        for seq in 1..=(MAX_EVENTS_PER_SESSION as u64 + 10) {
            registry
                .append_event(SessionEvent {
                    id: Uuid::new_v4(),
                    session_id: "s1".into(),
                    sequence: seq,
                    occurred_at_ms: seq as i64,
                    kind: SessionEventKind::Lifecycle,
                    level: EventLevel::Info,
                    summary: "e".into(),
                    tool_name: None,
                })
                .unwrap();
        }

        assert_eq!(registry.events_for("s1").len(), MAX_EVENTS_PER_SESSION);
        assert_eq!(
            registry.events_for("s1")[0].sequence,
            11,
            "oldest events should be pruned"
        );
    }

    #[test]
    fn metrics_input_updates_snapshot() {
        let core = test_core();
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "metric".into(),
            label: "metric".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 1,
        }))
        .unwrap();

        let session_id = core.snapshot().sessions[0].id.clone();
        let quality = MetricQuality {
            attribution: AttributionQuality::Exact,
            cpu: MetricAvailability::Available,
            io: IoQuality::Unavailable,
            reason: None,
        };
        let sample = MetricSample {
            at_ms: 10,
            cpu_core_percent: 10.0,
            cpu_host_percent: 5.0,
            rss_bytes: 100,
            runtime_ms: 1000,
            process_count: 1,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            quality: quality.clone(),
        };
        let mut agents = BTreeMap::new();
        agents.insert(session_id, sample);
        core.record_metrics(MetricsFrame {
            host: HostMetricSample {
                at_ms: 10,
                cpu_host_percent: 5.0,
                used_memory_bytes: 1,
                total_memory_bytes: 2,
                visible_process_count: 1,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
            },
            aggregate: AgentAggregate {
                at_ms: 10,
                cpu_core_percent: 10.0,
                cpu_host_percent: 5.0,
                rss_bytes: 100,
                runtime_ms: 1000,
                process_count: 1,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality,
                active_sessions: 1,
                attention_sessions: 0,
            },
            agents,
        })
        .unwrap();

        let snapshot = core.snapshot();
        assert!(snapshot.host.is_some());
        assert!(snapshot.aggregate.is_some());
        assert!(snapshot.sessions[0].latest_metric.is_some());
        let history = core.persisted_metric_history(0, 10_000, 100).unwrap();
        assert_eq!(history.host.points.len(), 1);
        assert_eq!(history.aggregate.points.len(), 1);
        assert_eq!(
            history
                .agents
                .values()
                .map(|series| series.points.len())
                .sum::<usize>(),
            1
        );
        core.record_metrics(MetricsFrame {
            host: snapshot.host.unwrap(),
            aggregate: snapshot.aggregate.unwrap(),
            agents: BTreeMap::new(),
        })
        .unwrap();
        assert!(
            core.snapshot().sessions[0].latest_metric.is_none(),
            "an authoritative frame without the session must clear stale latest metrics"
        );
    }

    struct BlockingStreamSink {
        frames: Mutex<Vec<StreamFrame>>,
        armed: AtomicBool,
        entered: Barrier,
        release: Barrier,
    }

    impl BlockingStreamSink {
        fn new() -> Self {
            Self {
                frames: Mutex::new(Vec::new()),
                armed: AtomicBool::new(false),
                entered: Barrier::new(2),
                release: Barrier::new(2),
            }
        }

        fn arm(&self) {
            self.armed.store(true, Ordering::Release);
        }
    }

    impl StreamSink for BlockingStreamSink {
        fn emit(&self, frame: StreamFrame) {
            if self.armed.swap(false, Ordering::AcqRel) {
                self.entered.wait();
                self.release.wait();
            }
            self.frames.lock().unwrap().push(frame);
        }

        fn latest_sequence(&self) -> u64 {
            self.frames
                .lock()
                .unwrap()
                .last()
                .map(|frame| frame.sequence)
                .unwrap_or(0)
        }
    }

    #[test]
    fn snapshot_and_sequence_are_atomic_during_ingest() {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(BlockingStreamSink::new());
        let core = Arc::new(
            AppCore::new(
                ManualClock(10),
                Arc::clone(&repo),
                Arc::clone(&stream),
                default_settings(),
            )
            .unwrap(),
        );
        register_cursor(&core);
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "race".into(),
            label: "before".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 1,
        }))
        .unwrap();

        stream.arm();
        let ingest_core = Arc::clone(&core);
        let ingest = std::thread::spawn(move || {
            ingest_core
                .ingest(IngestCommand::SessionUpdate(SessionUpdateCommand {
                    source: AgentSource::Cursor,
                    external_session_id: "race".into(),
                    status: Some(SessionStatus::Running),
                    label: Some("after".into()),
                    workspace_label: None,
                    occurred_at_ms: 2,
                }))
                .unwrap();
        });
        stream.entered.wait();

        let (snapshot_started_tx, snapshot_started_rx) = std::sync::mpsc::channel();
        let (snapshot_tx, snapshot_rx) = std::sync::mpsc::channel();
        let snapshot_core = Arc::clone(&core);
        let snapshot_thread = std::thread::spawn(move || {
            snapshot_started_tx.send(()).unwrap();
            snapshot_tx
                .send(snapshot_core.snapshot_with_sequence())
                .unwrap();
        });
        snapshot_started_rx.recv().unwrap();
        assert!(
            snapshot_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "snapshot must wait for the in-flight write lock"
        );

        stream.release.wait();
        ingest.join().unwrap();
        let captured = snapshot_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        snapshot_thread.join().unwrap();
        assert_eq!(captured.snapshot.sessions[0].label, "after");
        assert_eq!(captured.sequence, stream.latest_sequence());
        assert!(
            captured
                .events
                .iter()
                .any(|event| event.summary == "Session updated")
        );
    }

    #[test]
    fn capacity_eviction_deletes_session_and_events_durably() {
        let repo = Arc::new(SqliteRepository::in_memory().unwrap());
        let stream = Arc::new(VecStreamSink::new());
        let core = AppCore::new(
            ManualClock(1_000),
            Arc::clone(&repo),
            stream,
            default_settings(),
        )
        .unwrap();
        register_cursor(&core);

        for index in 0..MAX_SESSIONS {
            core.ingest(IngestCommand::SessionStart(SessionStartCommand {
                idempotency_key: None,
                source: AgentSource::Cursor,
                external_session_id: format!("evict-{index}"),
                label: format!("session-{index}"),
                workspace_label: None,
                status: SessionStatus::Running,
                occurred_at_ms: index as i64,
            }))
            .unwrap();
            core.ingest(IngestCommand::SessionEnd(SessionEndCommand {
                source: AgentSource::Cursor,
                external_session_id: format!("evict-{index}"),
                status: SessionStatus::Completed,
                occurred_at_ms: index as i64 + 1,
                summary: None,
            }))
            .unwrap();
        }

        let evicted_id = crate::domain::session_id_for(AgentSource::Cursor, "evict-0");
        core.ingest(IngestCommand::SessionStart(SessionStartCommand {
            idempotency_key: None,
            source: AgentSource::Cursor,
            external_session_id: "replacement".into(),
            label: "replacement".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            occurred_at_ms: 10_000,
        }))
        .unwrap();

        let conn = repo.connection();
        let session_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                [&evicted_id],
                |row| row.get(0),
            )
            .unwrap();
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
                [&evicted_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_count, 0);
        assert_eq!(event_count, 0);
    }

    #[test]
    fn metric_purge_clears_in_memory_snapshot_metrics() {
        let core = test_core();
        let quality = MetricQuality {
            attribution: AttributionQuality::Unknown,
            cpu: MetricAvailability::Available,
            io: IoQuality::Unavailable,
            reason: None,
        };
        core.record_metrics(MetricsFrame {
            host: HostMetricSample {
                at_ms: 10,
                cpu_host_percent: 1.0,
                used_memory_bytes: 1,
                total_memory_bytes: 2,
                visible_process_count: 1,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
            },
            aggregate: AgentAggregate {
                at_ms: 10,
                cpu_core_percent: 0.0,
                cpu_host_percent: 1.0,
                rss_bytes: 0,
                runtime_ms: 0,
                process_count: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality,
                active_sessions: 0,
                attention_sessions: 0,
            },
            agents: BTreeMap::new(),
        })
        .unwrap();
        assert!(core.snapshot().host.is_some());
        core.purge_metric_history().unwrap();
        let snapshot = core.snapshot();
        assert!(snapshot.host.is_none());
        assert!(snapshot.aggregate.is_none());
    }
}
