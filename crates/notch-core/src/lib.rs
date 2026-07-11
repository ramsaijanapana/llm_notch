//! # notch-core
//!
//! Tauri-independent application core: ingest orchestration, bounded session
//! registry, SQLite persistence, sustained alerts, and sequenced stream output.

mod alerts;
mod app_core;
mod constants;
mod domain;
mod error;
mod persistence;
mod registry;
mod stream;
mod traits;

pub use alerts::{ActiveAlert, AlertEvaluator, AlertKind, resource_alerts_from_active};
pub use app_core::{AppCore, SnapshotWithSequence};
pub use constants::*;
pub use domain::*;
pub use error::{CoreError, CoreResult};
pub use persistence::{
    PersistedMetricHistory, PersistedMetricSeries, PurgeReport, SessionEventPage, SqliteRepository,
};
pub use registry::SessionRegistry;
pub use stream::{StreamCoalescer, StreamReplayBuffer};
pub use traits::{
    Clock, MetricsCoreHandle, MetricsInput, SessionRepository, StreamSink, VecStreamSink,
};
