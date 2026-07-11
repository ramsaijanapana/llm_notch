//! Generic third-party ingest protocol adapter for llm_notch.
//!
//! Onboarding is documentation plus example clients — nothing is installed automatically.

mod capabilities;
mod examples;
mod protocol;

pub use capabilities::{GenericClientCapabilities, capabilities, capabilities_with_ack};
pub use examples::{ExampleValidationError, validate_emit_example, validate_protocol_fixture_file};
pub use protocol::{
    GENERIC_PROTOCOL_VERSION, GenericCommandExample, GenericProtocolError, example_commands,
    validate_ingest_example, validate_protocol_fixture,
};
