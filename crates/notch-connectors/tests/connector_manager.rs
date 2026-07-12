use std::path::PathBuf;

use notch_connectors::{ConnectorConfig, ConnectorManager};
use notch_protocol::{AgentSource, ConnectorFileOutcome, ConnectorScope};
use tempfile::TempDir;

fn integrations_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations")
}

fn test_config(dir: &TempDir) -> ConnectorConfig {
    ConnectorConfig {
        integrations_root: integrations_root(),
        app_data_dir: dir.path().to_path_buf(),
        helper_path: dir.path().join("llm-notch-hook.exe"),
        workspace_root: Some(std::fs::canonicalize(dir.path()).expect("canonicalize")),
        user_scope_root: Some(std::fs::canonicalize(dir.path()).expect("canonicalize")),
    }
}

#[test]
fn apply_rejects_selected_paths_not_in_plan() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".cursor/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    std::fs::write(&hooks, r#"{"version":1,"hooks":{}}"#).expect("write");

    let manager = ConnectorManager::new(test_config(&dir)).expect("manager");
    let preview = manager
        .preview_install(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");
    let err = manager
        .apply(
            &preview.plan_id,
            Some(&["~/.cursor/hooks.json".into(), "/etc/passwd".into()]),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        notch_connectors::ConnectorError::InvalidRequest(_)
    ));
}

#[test]
fn merge_preserves_foreign_entries() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".cursor/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    let baseline = integrations_root().join("fixtures/connectors/cursor-user-baseline.json");
    std::fs::copy(baseline, &hooks).expect("seed");

    let manager = ConnectorManager::new(test_config(&dir)).expect("manager");
    let preview = manager
        .preview_install(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");
    assert!(!preview.files[0].foreign_entries_preserved.is_empty());
    assert!(!preview.files[0].diff_text.is_empty());

    let result = manager.apply(&preview.plan_id, None).expect("apply");
    assert_eq!(
        result.file_results[0].outcome,
        ConnectorFileOutcome::Applied
    );

    let merged: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hooks).expect("read")).expect("json");
    assert!(merged["hooks"]["beforeShellExecution"].is_array());
    assert!(merged["hooks"]["sessionStart"].is_array());
}

#[test]
fn idempotent_reinstall_skips_backup() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".cursor/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    std::fs::write(&hooks, r#"{"version":1,"hooks":{}}"#).expect("write");

    let manager = ConnectorManager::new(test_config(&dir)).expect("manager");
    let first = manager
        .preview_install(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");
    manager.apply(&first.plan_id, None).expect("apply");

    let preview = manager
        .preview_install(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");
    assert!(preview.files[0].diff_text.is_empty());

    let result = manager.apply(&preview.plan_id, None).expect("apply");
    assert_eq!(
        result.file_results[0].outcome,
        ConnectorFileOutcome::Skipped
    );
    assert!(preview.backup_display_hint.is_none());
}

#[test]
fn lock_contention_on_concurrent_apply() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".cursor/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    std::fs::write(&hooks, r#"{"version":1,"hooks":{}}"#).expect("write");

    let manager = ConnectorManager::new(test_config(&dir)).expect("manager");
    let preview = manager
        .preview_install(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");

    let lock_path = hooks.with_file_name("hooks.json.llm-notch.lock");
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .expect("hold lock");
    let apply_result = manager.apply(&preview.plan_id, None);
    drop(lock);
    let _ = std::fs::remove_file(lock_path);
    assert!(apply_result.is_err());
}

#[test]
fn remove_plan_strips_managed_entries_only() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".cursor/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    let merged = integrations_root().join("fixtures/connectors/cursor-user-merged.json");
    std::fs::copy(merged, &hooks).expect("seed");

    let manager = ConnectorManager::new(test_config(&dir)).expect("manager");
    let preview = manager
        .preview_remove(AgentSource::Cursor, ConnectorScope::User)
        .expect("preview");
    assert!(!preview.files[0].diff_text.is_empty());
    manager.apply(&preview.plan_id, None).expect("apply");

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hooks).expect("read")).expect("json");
    assert!(after["hooks"]["beforeShellExecution"].is_array());
    assert!(after["hooks"].get("sessionStart").is_none());
}

#[test]
fn repair_replaces_drifted_codex_commands() {
    let dir = TempDir::new().expect("tempdir");
    let hooks = dir.path().join(".codex/hooks.json");
    std::fs::create_dir_all(hooks.parent().unwrap()).expect("mkdir");
    let drifted = r#"{
      "hooks": {
        "SessionStart": [{
          "matcher": "startup|resume",
          "hooks": [{
            "type": "command",
            "command": "sh C:\\old\\llm-notch-hook.exe --source codex --vendor-event SessionStart",
            "timeout": 2
          }]
        }],
        "PreToolUse": [{
          "matcher": "Bash",
          "hooks": [{
            "type": "command",
            "command": "/usr/bin/python3 policy.py"
          }]
        }]
      }
    }"#;
    std::fs::write(&hooks, drifted).expect("write");

    let helper = dir.path().join("llm-notch-hook.exe");
    std::fs::write(&helper, b"").expect("touch helper");
    let mut config = test_config(&dir);
    config.helper_path = helper.clone();

    let manager = ConnectorManager::new(config).expect("manager");
    let preview = manager
        .preview_repair(AgentSource::Codex, ConnectorScope::User)
        .expect("preview");
    assert!(!preview.files[0].diff_text.is_empty());
    manager.apply(&preview.plan_id, None).expect("apply");

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hooks).expect("read")).expect("json");
    let command = after["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .expect("command");
    assert!(!command.starts_with("sh "));
    assert!(command.contains(" hook --source codex --vendor-event SessionStart"));
    assert!(command.contains("llm-notch-hook.exe"));
    let pre_groups = after["hooks"]["PreToolUse"].as_array().expect("groups");
    assert_eq!(pre_groups.len(), 2);
    assert!(
        pre_groups.iter().any(|group| {
            group["hooks"][0]["command"].as_str() == Some("/usr/bin/python3 policy.py")
        }),
        "foreign PreToolUse hook must be preserved"
    );
}
