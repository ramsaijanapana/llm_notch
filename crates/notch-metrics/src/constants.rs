//! Sampling bounds and refresh intervals for the metrics engine.

/// One-second samples retained per registered session (15 minutes).
pub const MAX_HISTORY_SAMPLES_PER_SESSION: usize = 900;

/// Maximum concurrently tracked session process roots.
pub const MAX_ACTIVE_ROOTS: usize = 64;

/// Refresh interval while at least one session root is registered.
pub const ACTIVE_REFRESH_INTERVAL_MS: u64 = 1_000;

/// Refresh interval when no session roots are registered.
pub const IDLE_REFRESH_INTERVAL_MS: u64 = 5_000;

/// Maximum parent hops when walking a process tree.
pub const MAX_PARENT_WALK_DEPTH: usize = 256;
