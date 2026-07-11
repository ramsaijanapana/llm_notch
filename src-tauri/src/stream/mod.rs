//! Sequenced stream delivery for overlay and dashboard windows.
//!
//! Sequence numbers come exclusively from `notch_core::AppCore`; the hub is
//! the single replay and live-delivery implementation in the desktop host.

mod hub;

pub use hub::{PublishError, StreamHub, SubscribeError};
