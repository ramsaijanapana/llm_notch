use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Identifies which agent runtime produced a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum AgentSource {
    Cursor,
    ClaudeCode,
    Codex,
    Generic,
    Unknown,
}

/// High-level lifecycle state for an agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SessionStatus {
    Starting,
    Running,
    Waiting,
    Paused,
    Completed,
    Failed,
    Stale,
}

/// What kind of user attention a session currently requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum AttentionKind {
    None,
    Approval,
    Question,
    Permission,
    Error,
}

/// Confidence in process/session attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum AttributionQuality {
    Exact,
    Shared,
    Heuristic,
    Unknown,
}

/// Availability of a sampled metric family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum MetricAvailability {
    Available,
    WarmingUp,
    Unavailable,
}

/// Scope and reliability of process I/O counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum IoQuality {
    Disk,
    AllIo,
    Partial,
    Unavailable,
}

/// Kind of timeline event within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SessionEventKind {
    Lifecycle,
    Tool,
    Attention,
    Status,
}

/// Severity assigned to a normalized session event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum EventLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// Fidelity of attention reporting offered by an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum AttentionCapability {
    Full,
    Partial,
    None,
}

/// Granularity of context-open support advertised by an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ContextOpenTier {
    None,
    AppActivate,
    WindowFocus,
    ExactPane,
}

impl Default for ContextOpenTier {
    fn default() -> Self {
        Self::None
    }
}

/// Stable identity for an OS process associated with a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct ProcessIdentity {
    /// Operating-system process identifier.
    pub pid: u32,
    /// Process creation time; paired with PID to survive PID reuse.
    #[ts(type = "number")]
    pub started_at_ms: i64,
}

/// Quality metadata carried beside every per-agent and aggregate sample.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct MetricQuality {
    pub attribution: AttributionQuality,
    pub cpu: MetricAvailability,
    pub io: IoQuality,
    /// Bounded diagnostic explanation when a metric family is degraded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// Resource sample for a single attributed session process tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct MetricSample {
    #[ts(type = "number")]
    pub at_ms: i64,
    /// 100% equals one logical core and may exceed 100%.
    pub cpu_core_percent: f64,
    /// Host-normalized CPU percentage in the range 0..=100.
    pub cpu_host_percent: f64,
    #[ts(type = "number")]
    pub rss_bytes: u64,
    #[ts(type = "number")]
    pub runtime_ms: u64,
    pub process_count: u32,
    #[ts(type = "number")]
    pub read_bytes_per_sec: u64,
    #[ts(type = "number")]
    pub write_bytes_per_sec: u64,
    pub quality: MetricQuality,
}

/// System-wide resource sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct HostMetricSample {
    #[ts(type = "number")]
    pub at_ms: i64,
    pub cpu_host_percent: f64,
    #[ts(type = "number")]
    pub used_memory_bytes: u64,
    #[ts(type = "number")]
    pub total_memory_bytes: u64,
    pub visible_process_count: u32,
    #[ts(type = "number")]
    pub disk_read_bytes_per_sec: u64,
    #[ts(type = "number")]
    pub disk_write_bytes_per_sec: u64,
}

/// Deduplicated resource totals across all attributed agent process trees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AgentAggregate {
    #[ts(type = "number")]
    pub at_ms: i64,
    pub cpu_core_percent: f64,
    pub cpu_host_percent: f64,
    #[ts(type = "number")]
    pub rss_bytes: u64,
    #[ts(type = "number")]
    pub runtime_ms: u64,
    pub process_count: u32,
    #[ts(type = "number")]
    pub read_bytes_per_sec: u64,
    #[ts(type = "number")]
    pub write_bytes_per_sec: u64,
    pub quality: MetricQuality,
    pub active_sessions: u32,
    pub attention_sessions: u32,
}

/// Canonical agent session record exchanged between host and UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AgentSession {
    /// Stable session id (bounded by [`crate::MAX_SESSION_ID_LEN`]).
    pub id: String,
    pub source: AgentSource,
    /// Source-owned session identifier, namespaced by `source`.
    pub external_session_id: String,
    /// Privacy-preserving display label.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub workspace_label: Option<String>,
    pub status: SessionStatus,
    pub attention: AttentionKind,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    #[ts(type = "number")]
    pub last_event_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub ended_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub process_root: Option<ProcessIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub latest_metric: Option<MetricSample>,
}

/// Timeline event belonging to a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct SessionEvent {
    pub id: Uuid,
    pub session_id: String,
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub occurred_at_ms: i64,
    pub kind: SessionEventKind,
    pub level: EventLevel,
    /// Redacted event summary (bounded by [`crate::MAX_EVENT_SUMMARY_LEN`]).
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tool_name: Option<String>,
}

/// Observation paths supported by an adapter (Sol capability matrix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AdapterObservationPaths {
    pub lifecycle_events: bool,
    pub tool_events: bool,
    pub attention_events: bool,
}

/// Response paths supported by an adapter (Sol capability matrix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AdapterResponsePaths {
    pub decisions: bool,
    pub questions: bool,
    pub context_open_tier: ContextOpenTier,
}

/// Capability flags advertised by an adapter integration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AdapterCapabilities {
    pub source: AgentSource,
    pub events: bool,
    pub attention: AttentionCapability,
    pub decision_response: bool,
    pub context_open: bool,
    pub process_attribution: AttributionQuality,
    /// Additive v2 field; supersedes coarse `context_open` when non-`none`.
    #[serde(default)]
    pub context_open_tier: ContextOpenTier,
    #[serde(default)]
    pub observe_lifecycle: bool,
    #[serde(default)]
    pub observe_tools: bool,
    #[serde(default)]
    pub respond_decisions: bool,
    #[serde(default)]
    pub respond_questions: bool,
    #[serde(default = "default_fail_open_hooks")]
    pub fail_open_hooks: bool,
    #[serde(default)]
    pub requires_external_trust: bool,
}

fn default_fail_open_hooks() -> bool {
    true
}

impl AdapterCapabilities {
    /// Shipped template defaults aligned with `docs/integrations/capability-matrix.md`.
    pub fn template(source: AgentSource) -> Self {
        let (attention, requires_external_trust) = match source {
            AgentSource::ClaudeCode => (AttentionCapability::Partial, false),
            AgentSource::Codex => (AttentionCapability::None, true),
            AgentSource::Generic => (AttentionCapability::Full, false),
            AgentSource::Cursor | AgentSource::Unknown => (AttentionCapability::None, false),
        };

        Self {
            source,
            events: source != AgentSource::Unknown,
            attention,
            decision_response: false,
            context_open: false,
            process_attribution: AttributionQuality::Unknown,
            context_open_tier: ContextOpenTier::None,
            observe_lifecycle: source != AgentSource::Unknown,
            observe_tools: source != AgentSource::Unknown,
            respond_decisions: false,
            respond_questions: false,
            fail_open_hooks: true,
            requires_external_trust,
        }
    }

    /// Derives the Sol matrix observation/response split from wire flags.
    pub fn observation_paths(&self) -> AdapterObservationPaths {
        AdapterObservationPaths {
            lifecycle_events: self.observe_lifecycle || self.events,
            tool_events: self.observe_tools || self.events,
            attention_events: self.attention != AttentionCapability::None,
        }
    }

    pub fn response_paths(&self) -> AdapterResponsePaths {
        AdapterResponsePaths {
            decisions: self.respond_decisions || self.decision_response,
            questions: self.respond_questions,
            context_open_tier: if self.context_open_tier != ContextOpenTier::None {
                self.context_open_tier
            } else if self.context_open {
                ContextOpenTier::AppActivate
            } else {
                ContextOpenTier::None
            },
        }
    }
}

/// User-visible settings safe to expose to overlay and dashboard surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct PublicSettings {
    pub overlay_enabled: bool,
    pub autostart_enabled: bool,
    pub reduced_motion: bool,
    #[ts(type = "number")]
    pub sampling_interval_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub selected_display: Option<String>,
    pub show_over_fullscreen: bool,
    pub history_retention_hours: u32,
}

/// One live metrics update delivered to renderer subscribers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct MetricsFrame {
    pub host: HostMetricSample,
    pub aggregate: AgentAggregate,
    pub agents: BTreeMap<String, MetricSample>,
}

/// Point-in-time host snapshot delivered to renderers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct AppSnapshot {
    pub protocol_version: u16,
    #[ts(type = "number")]
    pub captured_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub host: Option<HostMetricSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub aggregate: Option<AgentAggregate>,
    pub sessions: Vec<AgentSession>,
    pub settings: PublicSettings,
    pub adapters: Vec<AdapterCapabilities>,
}

/// Individual stream payload variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase", tag = "type")]
pub enum StreamPayload {
    Snapshot {
        snapshot: AppSnapshot,
    },
    SessionUpsert {
        session: AgentSession,
    },
    SessionRemove {
        #[serde(rename = "sessionId")]
        #[ts(rename = "sessionId")]
        session_id: String,
    },
    SessionEvent {
        event: SessionEvent,
    },
    Metrics {
        metrics: MetricsFrame,
    },
    SettingsChanged {
        settings: PublicSettings,
    },
    IntegrationChanged {
        integration: AdapterCapabilities,
    },
    Heartbeat,
    ResyncRequired {
        /// Bounded explanation suitable for diagnostics, not display of secrets.
        reason: String,
    },
}

/// Framed stream envelope with monotonic sequencing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct StreamFrame {
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub emitted_at_ms: i64,
    pub payload: StreamPayload,
}
