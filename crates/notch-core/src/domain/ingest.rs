use notch_protocol::{
    AdapterCapabilities, AgentSource, AttentionKind, EventLevel, ProcessIdentity, SessionEventKind,
    SessionStatus,
};
use uuid::Uuid;

/// Normalized ingest commands accepted by the application core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestCommand {
    SessionStart(SessionStartCommand),
    SessionUpdate(SessionUpdateCommand),
    SessionEnd(SessionEndCommand),
    LifecycleEvent(LifecycleEventCommand),
    ToolEvent(ToolEventCommand),
    Attention(AttentionCommand),
    RegisterProcessRoot(ProcessRootCommand),
    IntegrationHealth(IntegrationHealthCommand),
    AcknowledgeAttention(AcknowledgeAttentionCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStartCommand {
    pub idempotency_key: Option<String>,
    pub source: AgentSource,
    pub external_session_id: String,
    pub label: String,
    pub workspace_label: Option<String>,
    pub status: SessionStatus,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUpdateCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub status: Option<SessionStatus>,
    pub label: Option<String>,
    pub workspace_label: Option<String>,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEndCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub status: SessionStatus,
    pub occurred_at_ms: i64,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEventCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub level: EventLevel,
    pub summary: String,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolEventCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub level: EventLevel,
    pub summary: String,
    pub tool_name: Option<String>,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub attention: AttentionKind,
    pub level: EventLevel,
    pub summary: String,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRootCommand {
    pub source: AgentSource,
    pub external_session_id: String,
    pub process_root: ProcessIdentity,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationHealthCommand {
    pub capabilities: AdapterCapabilities,
    pub healthy: bool,
    pub message: Option<String>,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgeAttentionCommand {
    pub session_id: String,
    pub occurred_at_ms: i64,
}

/// Outcome of a handled ingest command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestResult {
    pub session_id: Option<String>,
    pub event_id: Option<Uuid>,
    pub idempotent_replay: bool,
}

impl IngestResult {
    pub fn empty() -> Self {
        Self {
            session_id: None,
            event_id: None,
            idempotent_replay: false,
        }
    }
}

/// Maps ingest commands to normalized session event kinds.
pub fn lifecycle_kind() -> SessionEventKind {
    SessionEventKind::Lifecycle
}

pub fn tool_kind() -> SessionEventKind {
    SessionEventKind::Tool
}

pub fn attention_kind() -> SessionEventKind {
    SessionEventKind::Attention
}

pub fn status_kind() -> SessionEventKind {
    SessionEventKind::Status
}
