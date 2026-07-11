//! Interactive decision broker: capability-gated permission/approval flows with
//! honest delivery states and fail-open hook timeouts.

pub mod adapter;
pub mod broker;
pub mod migration;
pub mod store;
pub mod types;

pub use broker::DecisionBroker;
pub use types::{DecisionReplyPayload, DecisionWaitPayload, PendingDecisionWait};
