//! Validate shipped generic emit examples and protocol fixtures.

use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::protocol::{GenericProtocolError, validate_protocol_fixture};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExampleValidationError {
    #[error("example script missing required `--source generic`")]
    MissingGenericSource,
    #[error("example script missing `--event`")]
    MissingEvent,
    #[error("protocol fixture invalid: {0}")]
    Protocol(#[from] GenericProtocolError),
    #[error("read fixture `{0}`: {1}")]
    Io(String, String),
}

/// Validate a generic emit example shell/ps1 script contains required flags.
pub fn validate_emit_example(script: &str) -> Result<(), ExampleValidationError> {
    if !script.contains("--source generic") && !script.contains("--source") {
        return Err(ExampleValidationError::MissingGenericSource);
    }
    if !script.contains("--source generic") {
        return Err(ExampleValidationError::MissingGenericSource);
    }
    if !script.contains("--event") {
        return Err(ExampleValidationError::MissingEvent);
    }
    Ok(())
}

/// Load and validate one protocol fixture under `integrations/fixtures/protocol`.
pub fn validate_protocol_fixture_file(name: &str) -> Result<(), ExampleValidationError> {
    let path = protocol_fixture_path(name);
    let raw = std::fs::read_to_string(&path)
        .map_err(|err| ExampleValidationError::Io(path.display().to_string(), err.to_string()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| ExampleValidationError::Io(path.display().to_string(), err.to_string()))?;
    validate_protocol_fixture(&value).map_err(ExampleValidationError::Protocol)
}

fn protocol_fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("integrations/fixtures/protocol")
        .join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_examples_script_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("integrations/generic/emit-examples.sh");
        let script = std::fs::read_to_string(&path).expect("read shell examples");
        validate_emit_example(&script).expect("shell example");
    }

    #[test]
    fn emit_examples_ps1_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("integrations/generic/emit-examples.ps1");
        let script = std::fs::read_to_string(&path).expect("read ps1 examples");
        validate_emit_example(&script).expect("ps1 example");
    }

    #[test]
    fn protocol_command_fixtures_are_version_one() {
        for name in [
            "session-start.json",
            "session-event.json",
            "session-status.json",
            "session-end.json",
            "attention.json",
            "capabilities.json",
        ] {
            validate_protocol_fixture_file(name).expect(name);
        }
    }

    #[test]
    fn protocol_ingest_payload_fixtures_validate() {
        let path = protocol_fixture_path("process-root.json");
        let raw = std::fs::read_to_string(&path).expect("read");
        let value: Value = serde_json::from_str(&raw).expect("json");
        let payload: notch_ipc::IngestPayload =
            serde_json::from_value(value).expect("ingest payload");
        crate::protocol::validate_ingest_example(&payload).expect("process-root");
    }

    #[test]
    fn rejects_unsupported_protocol_version() {
        let value = serde_json::json!({"protocolVersion": 99});
        assert!(matches!(
            crate::protocol::validate_protocol_fixture(&value),
            Err(GenericProtocolError::UnsupportedVersion(99))
        ));
    }
}
