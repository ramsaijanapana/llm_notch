//! Connector manager: detection, preview, safe merge, backup, atomic apply, health, repair, rollback.
//!
//! Apply accepts `plan_id` plus optional display paths that must already exist in the stored plan;
//! canonical file identities and filesystem paths stay backend-only.

mod adapter;
mod apply;
mod atomic;
mod detect;
mod diff;
mod error;
mod executable;
mod hash;
mod health;
mod journal;
mod lock;
mod manager;
mod merge;
mod path_security;
mod plan;
mod preview;
mod process_scan;
mod remove;
mod repair;
mod rollback;

pub use adapter::{AdapterDescriptor, AdapterRegistry, PlanOperation};
pub use detect::DetectedConnector;
pub use error::ConnectorError;
pub use manager::{ConnectorConfig, ConnectorManager, SharedConnectorManager};
pub use plan::StoredPlan;
