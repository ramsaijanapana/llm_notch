use std::path::{Path, PathBuf};

use notch_protocol::{AgentSource, ConnectorScope, ExternalTrustAction, ExternalTrustActionKind};
use serde_json::Value;

use crate::merge::{
    merge_claude_settings, merge_hooks_json, remove_claude_settings, remove_hooks_json,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanOperation {
    Install,
    Remove,
    Repair,
    Rollback,
}

#[derive(Debug, Clone)]
pub struct TargetFile {
    pub relative_path: PathBuf,
    pub format: ConfigFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    HooksJson,
    ClaudeSettings,
}

#[derive(Debug, Clone)]
pub struct AdapterDescriptor {
    pub source: AgentSource,
    pub template_path: PathBuf,
    pub user_target: TargetFile,
    pub project_target: TargetFile,
    pub external_trust_actions: Vec<ExternalTrustAction>,
}

impl AdapterDescriptor {
    pub fn target_for(&self, scope: ConnectorScope) -> &TargetFile {
        match scope {
            ConnectorScope::User => &self.user_target,
            ConnectorScope::Project => &self.project_target,
        }
    }

    pub fn load_template(&self) -> Result<Value, std::io::Error> {
        let raw = std::fs::read_to_string(&self.template_path)?;
        let mut value: Value = serde_json::from_str(&raw)?;
        strip_template_comments(&mut value);
        Ok(value)
    }

    pub fn merge(&self, baseline: &Value, template: &Value) -> (Value, Vec<String>) {
        match self.user_target.format {
            ConfigFormat::HooksJson => merge_hooks_json(baseline, template),
            ConfigFormat::ClaudeSettings => merge_claude_settings(baseline, template),
        }
    }

    pub fn remove_managed(&self, baseline: &Value) -> (Value, Vec<String>) {
        match self.user_target.format {
            ConfigFormat::HooksJson => remove_hooks_json(baseline),
            ConfigFormat::ClaudeSettings => remove_claude_settings(baseline),
        }
    }
}

fn strip_template_comments(value: &mut Value) {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("_comment");
    }
}

pub struct AdapterRegistry {
    integrations_root: PathBuf,
    helper_path: PathBuf,
}

impl AdapterRegistry {
    pub fn new(integrations_root: PathBuf, helper_path: PathBuf) -> Self {
        Self {
            integrations_root,
            helper_path,
        }
    }

    pub fn helper_path(&self) -> &Path {
        &self.helper_path
    }

    pub fn get(&self, source: AgentSource) -> Option<AdapterDescriptor> {
        match source {
            AgentSource::Cursor => Some(self.cursor()),
            AgentSource::ClaudeCode => Some(self.claude_code()),
            AgentSource::Codex => Some(self.codex()),
            AgentSource::Generic | AgentSource::Unknown => None,
        }
    }

    pub fn supported_sources(&self) -> Vec<AgentSource> {
        vec![
            AgentSource::Cursor,
            AgentSource::ClaudeCode,
            AgentSource::Codex,
        ]
    }

    fn cursor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            source: AgentSource::Cursor,
            template_path: self
                .integrations_root
                .join("cursor/hooks.json.template"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".cursor/hooks.json"),
                format: ConfigFormat::HooksJson,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".cursor/hooks.json"),
                format: ConfigFormat::HooksJson,
            },
            external_trust_actions: Vec::new(),
        }
    }

    fn claude_code(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            source: AgentSource::ClaudeCode,
            template_path: self
                .integrations_root
                .join("claude-code/settings.hooks.template.json"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".claude/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".claude/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            external_trust_actions: Vec::new(),
        }
    }

    fn codex(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            source: AgentSource::Codex,
            template_path: self
                .integrations_root
                .join("codex/hooks.json.template"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".codex/hooks.json"),
                format: ConfigFormat::HooksJson,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".codex/hooks.json"),
                format: ConfigFormat::HooksJson,
            },
            external_trust_actions: vec![ExternalTrustAction {
                kind: ExternalTrustActionKind::CodexHooksReview,
                instructions:
                    "Run /hooks in Codex and approve the llm_notch hooks to finish setup.".into(),
            }],
        }
    }
}

const HELPER_PLACEHOLDER: &str = "{{LLM_NOTCH_HELPER}}";
const WRAPPER_PLACEHOLDER: &str = "{{LLM_NOTCH_WRAPPER}}";
const WRAPPER_ABSOLUTE_PLACEHOLDER: &str = "{{LLM_NOTCH_WRAPPER_ABSOLUTE_PATH}}";

/// Rewrite template hook commands to use the absolute bundled helper path.
pub fn materialize_template(template: &Value, helper_path: &Path) -> Value {
    let helper = helper_path.to_string_lossy();
    let mut value = template.clone();
    rewrite_commands(&mut value, helper_path, &helper);
    value
}

fn rewrite_commands(value: &mut Value, helper_path: &Path, helper: &str) {
    match value {
        Value::Object(map) => {
            for (_, child) in map.iter_mut() {
                rewrite_commands(child, helper_path, helper);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                rewrite_commands(item, helper_path, helper);
            }
        }
        Value::String(text)
            if text.contains("llm-notch-hook")
                || text.contains(HELPER_PLACEHOLDER)
                || text.contains(WRAPPER_PLACEHOLDER)
                || text.contains(WRAPPER_ABSOLUTE_PLACEHOLDER) =>
        {
            *text = rewrite_command(text, helper_path, helper);
        }
        _ => {}
    }
}

fn rewrite_command(command: &str, helper_path: &Path, helper: &str) -> String {
    let quoted_helper = if helper.contains(' ') {
        format!("\"{helper}\"")
    } else {
        helper.to_string()
    };

    if command.contains(HELPER_PLACEHOLDER) {
        return command.replace(HELPER_PLACEHOLDER, &quoted_helper);
    }
    if command.contains(WRAPPER_PLACEHOLDER) {
        return command.replace(WRAPPER_PLACEHOLDER, &quoted_helper);
    }
    if command.contains(WRAPPER_ABSOLUTE_PLACEHOLDER) {
        return command.replace(WRAPPER_ABSOLUTE_PLACEHOLDER, &quoted_helper);
    }

    if let Some(idx) = command.find("--source") {
        let suffix = command[idx..].trim();
        if cfg!(windows) {
            format!("{quoted_helper} {suffix}")
        } else {
            format!("{quoted_helper} {suffix}")
        }
    } else if command.contains("llm-notch-hook-wrapper") {
        format!("{quoted_helper}")
    } else {
        helper_path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn registry_loads_cursor_template() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry =
            AdapterRegistry::new(root.clone(), root.join("target/fake/llm-notch-hook.exe"));
        let adapter = registry.get(AgentSource::Cursor).expect("cursor");
        let template = adapter.load_template().expect("template");
        assert!(template.get("hooks").is_some());
    }

    #[test]
    fn materialize_replaces_wrapper_with_helper_path() {
        let template = json!({
            "hooks": {
                "sessionStart": [{
                    "command": "sh integrations/wrappers/llm-notch-hook-wrapper.sh --source cursor --vendor-event sessionStart"
                }]
            }
        });
        let dir = TempDir::new().expect("tempdir");
        let helper = dir.path().join("llm-notch-hook.exe");
        let materialized = materialize_template(&template, &helper);
        let command = materialized["hooks"]["sessionStart"][0]["command"]
            .as_str()
            .expect("command");
        assert!(command.contains("llm-notch-hook.exe"));
        assert!(command.contains("--source cursor"));
    }
}
