//! Protocol version and bounded wire limits.
//!
//! Length constants express intent for validation layers; enforcement is deferred
//! to ingest/transport agents.

/// Current protocol version. Bump on breaking wire changes.
pub const PROTOCOL_VERSION: u16 = 1;

/// Maximum serialized stream frame size in bytes.
pub const MAX_STREAM_FRAME_BYTES: usize = 65_536;

/// Maximum UTF-8 byte length for session identifiers.
pub const MAX_SESSION_ID_LEN: usize = 64;

/// Maximum UTF-8 byte length for source-owned session identifiers.
pub const MAX_EXTERNAL_SESSION_ID_LEN: usize = 256;

/// Maximum UTF-8 byte length for privacy-preserving session labels.
pub const MAX_SESSION_LABEL_LEN: usize = 256;

/// Maximum UTF-8 byte length for optional workspace labels.
pub const MAX_WORKSPACE_LABEL_LEN: usize = 256;

/// Maximum UTF-8 byte length for normalized, redacted event summaries.
pub const MAX_EVENT_SUMMARY_LEN: usize = 512;

/// Maximum UTF-8 byte length for normalized tool names.
pub const MAX_TOOL_NAME_LEN: usize = 128;

/// Maximum UTF-8 byte length for metric-quality explanations.
pub const MAX_METRIC_REASON_LEN: usize = 512;

/// Maximum UTF-8 byte length for resynchronization explanations.
pub const MAX_RESYNC_REASON_LEN: usize = 512;

/// Recommended heartbeat interval for live streams (milliseconds).
pub const STREAM_HEARTBEAT_INTERVAL_MS: u64 = 5_000;

/// Maximum concurrent sessions represented in a snapshot.
pub const MAX_SNAPSHOT_SESSIONS: usize = 128;

/// Maximum UTF-8 byte length for connector plan identifiers.
pub const MAX_PLAN_ID_LEN: usize = 128;

/// Maximum UTF-8 byte length for display-only connector path labels.
pub const MAX_CONNECTOR_DISPLAY_PATH_LEN: usize = 512;

/// Maximum UTF-8 byte length for connector diff preview text.
pub const MAX_CONNECTOR_DIFF_LEN: usize = 65_536;

/// Maximum files represented in one connector plan preview.
pub const MAX_CONNECTOR_PLAN_FILES: usize = 16;

/// Default connector plan preview TTL (milliseconds).
pub const CONNECTOR_PLAN_TTL_MS: u64 = 300_000;

/// Maximum UTF-8 byte length for redacted decision summaries.
pub const MAX_DECISION_SUMMARY_LEN: usize = 512;

/// Maximum UTF-8 byte length for free-text decision answers when enabled.
pub const MAX_DECISION_ANSWER_LEN: usize = 4_096;
