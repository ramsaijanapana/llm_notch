use notch_protocol::{
    AdapterCapabilities, AgentSession, MetricsFrame, PublicSettings, SessionEvent, StreamFrame,
};
use parking_lot::Mutex;

use crate::error::CoreResult;
use crate::persistence::{PersistedMetricHistory, PurgeReport, SessionEventPage, SqliteRepository};

/// Injectable clock for deterministic tests and replay.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

/// Durable and in-memory session persistence boundary.
pub trait SessionRepository: Send + Sync {
    fn load_sessions(&self) -> CoreResult<Vec<AgentSession>>;

    fn load_events(&self) -> CoreResult<Vec<SessionEvent>>;

    fn upsert_session(&self, session: &AgentSession) -> CoreResult<()>;

    fn remove_session(&self, session_id: &str) -> CoreResult<()>;

    fn append_event(&self, event: &SessionEvent) -> CoreResult<()>;

    fn load_session_event_page(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> CoreResult<SessionEventPage>;

    fn load_settings(&self) -> CoreResult<Option<PublicSettings>>;

    fn save_settings(&self, settings: &PublicSettings) -> CoreResult<()>;

    fn upsert_integration(
        &self,
        capabilities: &AdapterCapabilities,
        healthy: bool,
        message: Option<&str>,
        updated_at_ms: i64,
    ) -> CoreResult<()>;

    fn load_integrations(&self) -> CoreResult<Vec<AdapterCapabilities>>;

    fn record_metric_bucket(
        &self,
        scope: &str,
        session_id: Option<&str>,
        bucket_start_ms: i64,
        sample: &notch_protocol::MetricSample,
    ) -> CoreResult<()>;

    fn load_metric_history(
        &self,
        session_id: &str,
        limit: usize,
    ) -> CoreResult<Vec<notch_protocol::MetricSample>>;

    fn load_persisted_metric_history(
        &self,
        requested_start_ms: i64,
        requested_end_ms: i64,
        max_points_per_series: usize,
    ) -> CoreResult<PersistedMetricHistory>;

    fn purge_metric_history(&self) -> CoreResult<u64>;

    fn purge_session_events(&self) -> CoreResult<u64>;

    fn store_idempotency_key(&self, key: &str, created_at_ms: i64) -> CoreResult<bool>;

    fn prune(&self, now_ms: i64, metric_retention_ms: i64) -> CoreResult<PurgeReport>;
}

impl SessionRepository for SqliteRepository {
    fn load_sessions(&self) -> CoreResult<Vec<AgentSession>> {
        SqliteRepository::load_sessions(self)
    }

    fn load_events(&self) -> CoreResult<Vec<SessionEvent>> {
        SqliteRepository::load_events(self)
    }

    fn upsert_session(&self, session: &AgentSession) -> CoreResult<()> {
        SqliteRepository::upsert_session(self, session)
    }

    fn remove_session(&self, session_id: &str) -> CoreResult<()> {
        SqliteRepository::remove_session(self, session_id)
    }

    fn append_event(&self, event: &SessionEvent) -> CoreResult<()> {
        SqliteRepository::append_event(self, event)
    }

    fn load_session_event_page(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> CoreResult<SessionEventPage> {
        SqliteRepository::load_session_event_page(self, session_id, before_sequence, limit)
    }

    fn load_settings(&self) -> CoreResult<Option<PublicSettings>> {
        SqliteRepository::load_settings(self)
    }

    fn save_settings(&self, settings: &PublicSettings) -> CoreResult<()> {
        SqliteRepository::save_settings(self, settings)
    }

    fn upsert_integration(
        &self,
        capabilities: &AdapterCapabilities,
        healthy: bool,
        message: Option<&str>,
        updated_at_ms: i64,
    ) -> CoreResult<()> {
        SqliteRepository::upsert_integration(self, capabilities, healthy, message, updated_at_ms)
    }

    fn load_integrations(&self) -> CoreResult<Vec<AdapterCapabilities>> {
        SqliteRepository::load_integrations(self)
    }

    fn record_metric_bucket(
        &self,
        scope: &str,
        session_id: Option<&str>,
        bucket_start_ms: i64,
        sample: &notch_protocol::MetricSample,
    ) -> CoreResult<()> {
        SqliteRepository::record_metric_bucket(self, scope, session_id, bucket_start_ms, sample)
    }

    fn load_metric_history(
        &self,
        session_id: &str,
        limit: usize,
    ) -> CoreResult<Vec<notch_protocol::MetricSample>> {
        SqliteRepository::load_metric_history(self, session_id, limit)
    }

    fn purge_metric_history(&self) -> CoreResult<u64> {
        SqliteRepository::purge_metric_history(self)
    }

    fn purge_session_events(&self) -> CoreResult<u64> {
        SqliteRepository::purge_session_events(self)
    }

    fn load_persisted_metric_history(
        &self,
        requested_start_ms: i64,
        requested_end_ms: i64,
        max_points_per_series: usize,
    ) -> CoreResult<PersistedMetricHistory> {
        SqliteRepository::load_persisted_metric_history(
            self,
            requested_start_ms,
            requested_end_ms,
            max_points_per_series,
        )
    }

    fn store_idempotency_key(&self, key: &str, created_at_ms: i64) -> CoreResult<bool> {
        SqliteRepository::store_idempotency_key(self, key, created_at_ms)
    }

    fn prune(&self, now_ms: i64, metric_retention_ms: i64) -> CoreResult<PurgeReport> {
        SqliteRepository::prune_with_metric_retention(self, now_ms, metric_retention_ms)
    }
}

/// Destination for sequenced stream frames emitted by the core.
pub trait StreamSink: Send + Sync {
    fn emit(&self, frame: StreamFrame);

    /// Returns buffered frames strictly newer than `sequence`.
    ///
    /// Production sinks can override this to provide bounded replay. The
    /// default keeps simple sinks source-compatible without claiming replay.
    fn stream_since(&self, _sequence: u64) -> Vec<StreamFrame> {
        Vec::new()
    }

    /// Latest sequence accepted by this sink, or zero before the first frame.
    fn latest_sequence(&self) -> u64 {
        0
    }
}

/// Collects frames for tests and in-process subscribers.
#[derive(Debug, Default)]
pub struct VecStreamSink {
    frames: Mutex<Vec<StreamFrame>>,
}

impl VecStreamSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frames(&self) -> Vec<StreamFrame> {
        self.frames.lock().clone()
    }

    pub fn clear(&self) {
        self.frames.lock().clear();
    }
}

impl StreamSink for VecStreamSink {
    fn emit(&self, frame: StreamFrame) {
        self.frames.lock().push(frame);
    }

    fn stream_since(&self, sequence: u64) -> Vec<StreamFrame> {
        self.frames
            .lock()
            .iter()
            .filter(|frame| frame.sequence > sequence)
            .cloned()
            .collect()
    }

    fn latest_sequence(&self) -> u64 {
        self.frames
            .lock()
            .last()
            .map(|frame| frame.sequence)
            .unwrap_or(0)
    }
}

/// Host and per-session metric samples supplied by the metrics crate.
pub trait MetricsInput: Send + Sync {
    fn ingest_metrics(&self, core: &dyn MetricsCoreHandle, frame: MetricsFrame) -> CoreResult<()>;
}

/// Narrow callback surface used by [`MetricsInput`] implementations.
pub trait MetricsCoreHandle {
    fn record_metrics(&self, frame: MetricsFrame) -> CoreResult<()>;
}
