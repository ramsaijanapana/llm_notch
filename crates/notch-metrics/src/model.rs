use notch_protocol::{AttributionQuality, ProcessIdentity};

/// Lightweight process snapshot used by pure graph/aggregation logic.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessNode {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub started_at_ms: i64,
    pub cpu_usage_percent: f64,
    pub rss_bytes: u64,
    pub read_bytes_total: u64,
    pub write_bytes_total: u64,
    pub read_bytes_delta: u64,
    pub write_bytes_delta: u64,
    pub io_available: bool,
}

/// Session root registration supplied by the host orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSession {
    pub session_id: String,
    pub root: ProcessIdentity,
    pub attribution: AttributionQuality,
    pub registered_at_ms: i64,
}

/// Result of validating a registered root against the live process snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootStatus {
    Valid,
    Missing,
    PidReused,
}

/// Whether a counter family can be reported on this refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterReadiness {
    WarmingUp,
    Ready,
}

impl CounterReadiness {
    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}
