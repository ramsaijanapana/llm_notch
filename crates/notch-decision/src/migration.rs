//! Test-only host schema bootstrap shared with integration tests.

pub const HOST_BOOTSTRAP_FOR_TESTS: &str = r#"
CREATE TABLE schema_version (version INTEGER NOT NULL);
INSERT INTO schema_version (version) VALUES (2);
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;
