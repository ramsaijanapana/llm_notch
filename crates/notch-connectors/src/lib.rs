//! Connector manager: detection, preview, safe merge, backup, atomic apply, health, repair, rollback.
//!
//! Renderer commands accept only `plan_id`; canonical file identities stay backend-only.

mod adapter;
mod apply;
mod atomic;
mod detect;
mod diff;
mod error;
mod hash;
mod health;
mod journal;
mod lock;
mod manager;
mod merge;
mod path_security;
mod plan;
mod preview;
mod remove;
mod repair;
mod rollback;

pub use adapter::{AdapterDescriptor, AdapterRegistry, PlanOperation};
pub use error::ConnectorError;
pub use detect::DetectedConnector;
pub use manager::{ConnectorConfig, ConnectorManager, SharedConnectorManager};
pub use plan::StoredPlan;
