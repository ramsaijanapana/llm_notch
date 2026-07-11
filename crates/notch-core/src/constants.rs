//! Core-owned limits and retention policy.

use std::time::Duration;

/// Maximum live sessions tracked in memory and snapshots.
pub const MAX_SESSIONS: usize = notch_protocol::MAX_SNAPSHOT_SESSIONS;

/// Maximum normalized events retained per session.
pub const MAX_EVENTS_PER_SESSION: usize = 10_000;

/// Maximum events included in an atomic renderer bootstrap.
pub const BOOTSTRAP_MAX_EVENTS: usize = 256;

/// Recent events reserved per active or unresolved session before filling the
/// remaining bootstrap budget with globally recent events.
pub const BOOTSTRAP_EVENTS_PER_ACTIVE_SESSION: usize = 1;

/// Maximum explicit per-session event page requested by the renderer.
pub const MAX_EVENT_PAGE_SIZE: usize = 100;

/// Metric aggregate bucket width.
pub const METRIC_BUCKET_SECS: i64 = 5;

/// Canonical non-null key for host metric buckets.
pub const HOST_METRIC_SESSION_KEY: &str = "__host__";

/// Raw metric bucket retention window.
pub const METRIC_RETENTION_MS: i64 = 24 * 60 * 60 * 1_000;

/// Session event retention window.
pub const EVENT_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1_000;

/// Sessions without events transition to stale after this interval.
pub const STALE_SESSION_MS: i64 = 30 * 60 * 1_000;

/// Soft database size target before aggressive pruning.
pub const PRUNE_TARGET_BYTES: u64 = 128 * 1024 * 1024;

/// Replay buffer for stream subscribers.
pub const STREAM_REPLAY_CAPACITY: usize = 1_024;

/// CPU host percent sustained above this for [`CPU_WARN_DURATION`] triggers an alert.
pub const CPU_WARN_THRESHOLD: f64 = 70.0;

/// CPU host percent sustained above this for [`CPU_CRITICAL_DURATION`] triggers an alert.
pub const CPU_CRITICAL_THRESHOLD: f64 = 90.0;

/// Duration CPU must remain above warn threshold.
pub const CPU_WARN_DURATION: Duration = Duration::from_secs(60);

/// Duration CPU must remain above critical threshold.
pub const CPU_CRITICAL_DURATION: Duration = Duration::from_secs(30);

/// Absolute RSS alert floor.
pub const RSS_ALERT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Fraction of host memory that may trigger RSS alerts.
pub const RSS_ALERT_HOST_FRACTION: f64 = 0.25;

/// Duration RSS must remain above threshold.
pub const RSS_ALERT_DURATION: Duration = Duration::from_secs(60);
