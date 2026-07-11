//! Host and per-agent process-tree metrics for llm_notch.
//!
//! See [`limitations`] for platform-specific caveats.
//!
//! Enable the `core` feature to implement [`notch_core::MetricsSource`] for
//! [`MetricsEngine`] once `notch-core` compiles in the workspace.
//!
//! # Example
//!
//! ```no_run
//! use notch_metrics::MetricsEngine;
//! use notch_protocol::{AttributionQuality, ProcessIdentity};
//!
//! let engine = MetricsEngine::new();
//! engine.register_session(
//!     "sess-1".into(),
//!     ProcessIdentity {
//!         pid: 4242,
//!         started_at_ms: 1_700_000_000_000,
//!     },
//!     AttributionQuality::Exact,
//!     1_700_000_000_000,
//! )
//! .expect("register");
//!
//! if let Some(frame) = engine.tick(1_700_000_001_000) {
//!     println!("host cpu={}%", frame.host.cpu_host_percent);
//! }
//! ```

pub mod aggregate;
pub mod bridge;
pub mod constants;
pub mod graph;
pub mod history;
pub mod limitations;
pub mod model;
pub mod sampler;
pub mod sysinfo_adapter;

use std::sync::Arc;

use notch_protocol::{AttributionQuality, MetricSample, MetricsFrame, ProcessIdentity};
use parking_lot::Mutex;
use thiserror::Error;

pub use constants::{
    ACTIVE_REFRESH_INTERVAL_MS, IDLE_REFRESH_INTERVAL_MS, MAX_ACTIVE_ROOTS,
    MAX_HISTORY_SAMPLES_PER_SESSION,
};
pub use sampler::{MetricsSampler, SamplerStats};

/// Errors raised by the metrics subsystem.
#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("metrics source unavailable")]
    Unavailable,
    #[error("too many active roots (max {max})")]
    TooManyRoots { max: usize },
    #[error("session not registered: {0}")]
    SessionNotRegistered(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type MetricsResult<T> = Result<T, MetricsError>;

impl PartialEq for MetricsError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unavailable, Self::Unavailable) => true,
            (Self::TooManyRoots { max: left }, Self::TooManyRoots { max: right }) => left == right,
            (Self::SessionNotRegistered(left), Self::SessionNotRegistered(right)) => left == right,
            _ => false,
        }
    }
}

/// Thread-safe metrics facade used by the host runtime.
#[derive(Debug, Clone)]
pub struct MetricsEngine {
    inner: Arc<Mutex<MetricsSampler>>,
}

impl Default for MetricsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsEngine {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MetricsSampler::new())),
        }
    }

    pub fn register_session(
        &self,
        session_id: String,
        root: ProcessIdentity,
        attribution: AttributionQuality,
        registered_at_ms: i64,
    ) -> MetricsResult<()> {
        self.inner
            .lock()
            .register_session(session_id, root, attribution, registered_at_ms)
    }

    pub fn resolve_process_identity(&self, pid: u32) -> Option<ProcessIdentity> {
        sysinfo_adapter::resolve_process_identity(pid)
    }

    pub fn unregister_session(&self, session_id: &str) {
        self.inner.lock().unregister_session(session_id);
    }

    pub fn registered_session_ids(&self) -> Vec<String> {
        self.inner.lock().registered_session_ids()
    }

    pub fn set_session_counts(&self, active_sessions: u32, attention_sessions: u32) {
        self.inner
            .lock()
            .set_session_counts(active_sessions, attention_sessions);
    }

    pub fn tick(&self, at_ms: i64) -> Option<MetricsFrame> {
        self.inner.lock().tick(at_ms)
    }

    pub fn refresh(&self, at_ms: i64) -> MetricsFrame {
        self.inner.lock().refresh(at_ms)
    }

    pub fn latest_frame(&self) -> Option<MetricsFrame> {
        self.inner.lock().latest_frame().cloned()
    }

    pub fn session_history(&self, session_id: &str) -> Vec<MetricSample> {
        self.inner.lock().session_history(session_id)
    }

    pub fn session_latest(&self, session_id: &str) -> Option<MetricSample> {
        self.inner.lock().session_latest(session_id)
    }

    pub fn clear_history(&self) {
        self.inner.lock().clear_history();
    }

    pub fn stats(&self) -> SamplerStats {
        self.inner.lock().stats()
    }

    pub fn refresh_interval_ms(&self) -> u64 {
        self.inner.lock().refresh_interval_ms()
    }
}

#[cfg(test)]
mod smoke {
    use super::*;

    /// Manual smoke test against the live process table.
    #[test]
    #[ignore = "manual: inspect real process metrics on a developer machine"]
    fn real_process_smoke_test() {
        let engine = MetricsEngine::new();
        let pid = std::process::id();
        let root = engine
            .resolve_process_identity(pid)
            .expect("current process identity");
        engine
            .register_session("self".into(), root, AttributionQuality::Heuristic, 0)
            .expect("register");

        let first = engine.refresh(1_000);
        let second = engine.refresh(2_000);
        assert!(first.host.visible_process_count > 0);
        assert!(second.agents.contains_key("self"));
        let sample = second.agents.get("self").expect("sample");
        assert!(sample.process_count >= 1);
        eprintln!("smoke sample: {sample:?}");
    }
}
