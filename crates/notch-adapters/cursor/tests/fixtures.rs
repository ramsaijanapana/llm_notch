//! Vendor fixture and version capability integration tests.

use std::fs;
use std::path::PathBuf;

use notch_adapters_cursor::{
    capabilities, detect_version, normalize_event, CursorVersionProfile,
};
use notch_protocol::AgentSource;

fn fixture(rel: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../integrations/fixtures/cursor")
        .join(rel);
    let text = fs::read_to_string(path).expect("read fixture");
    serde_json::from_str(&text).expect("parse fixture")
}

#[test]
fn known_version_fixture_enables_template_capabilities() {
    let payload = fixture("versions/known-v1-input.json");
    let version = payload["cursor_version"].as_str();
    let profile = detect_version(version, Some(1));
    assert!(matches!(profile, CursorVersionProfile::Known { .. }));
    let caps = capabilities(&profile);
    assert_eq!(caps.source, AgentSource::Cursor);
    assert!(caps.events);
}

#[test]
fn unknown_version_fixture_is_observation_only() {
    let payload = fixture("versions/unknown-version-input.json");
    let version = payload["cursor_version"].as_str();
    let profile = detect_version(version, Some(1));
    assert!(matches!(profile, CursorVersionProfile::Unknown { .. }));
    let caps = capabilities(&profile);
    assert!(caps.events);
    assert!(!caps.respond_decisions);
    let paths = caps.response_paths();
    assert!(!paths.decisions);
}

#[test]
fn vendor_fixtures_normalize_without_sensitive_fields() {
    for (file, event) in [
        ("session-start-input.json", "sessionStart"),
        ("session-end-input.json", "sessionEnd"),
        ("pre-tool-use-input.json", "preToolUse"),
        ("post-tool-use-input.json", "postToolUse"),
        ("post-tool-use-failure-input.json", "postToolUseFailure"),
        ("stop-input.json", "stop"),
    ] {
        let payload = fixture(file);
        let normalized = normalize_event(event, &payload, 1_700_000_000_000).expect(file);
        let encoded = serde_json::to_string(&normalized.summary).expect("summary");
        assert!(!encoded.contains("cargo test"), "{file} leaked command body");
        assert!(!encoded.contains("npm install"), "{file} leaked command body");
        assert!(!encoded.contains("running 4 tests"), "{file} leaked tool output");
    }
}
