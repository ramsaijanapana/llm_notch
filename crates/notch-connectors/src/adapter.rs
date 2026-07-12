use std::path::{Path, PathBuf};

use notch_protocol::{AgentSource, ConnectorScope, ExternalTrustAction, ExternalTrustActionKind};
use serde_json::Value;

use crate::merge::{
    merge_antigravity_named_hooks, merge_claude_settings, merge_hooks_json,
    remove_antigravity_named_hooks, remove_claude_settings, remove_hooks_json,
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
    AntigravityNamedHooks,
}

#[derive(Debug, Clone)]
pub struct AdapterDescriptor {
    pub catalog_id: &'static str,
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
            ConfigFormat::AntigravityNamedHooks => {
                merge_antigravity_named_hooks(baseline, template)
            }
        }
    }

    pub fn remove_managed(&self, baseline: &Value) -> (Value, Vec<String>) {
        match self.user_target.format {
            ConfigFormat::HooksJson => remove_hooks_json(baseline),
            ConfigFormat::ClaudeSettings => remove_claude_settings(baseline),
            ConfigFormat::AntigravityNamedHooks => remove_antigravity_named_hooks(baseline),
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
            AgentSource::Gemini => Some(self.gemini()),
            AgentSource::Qwen => Some(self.qwen()),
            AgentSource::AntigravityCli => Some(self.antigravity_cli()),
            AgentSource::CopilotCli => Some(self.copilot_cli()),
            AgentSource::Generic | AgentSource::Unknown => None,
        }
    }

    pub fn supported_sources(&self) -> Vec<AgentSource> {
        vec![
            AgentSource::Cursor,
            AgentSource::ClaudeCode,
            AgentSource::Codex,
            AgentSource::Gemini,
            AgentSource::Qwen,
            AgentSource::AntigravityCli,
            AgentSource::CopilotCli,
        ]
    }

    /// Catalog-backed adapters, including agents that do not yet have a distinct `AgentSource`.
    pub fn catalog_supported_ids(&self) -> Vec<&'static str> {
        vec![
            "cursor",
            "claude-code",
            "codex",
            "gemini-cli",
            "qwen",
            "antigravity-cli",
            "copilot",
        ]
    }

    pub fn get_by_catalog_id(&self, catalog_id: &str) -> Option<AdapterDescriptor> {
        match catalog_id {
            "cursor" => Some(self.cursor()),
            "claude-code" => Some(self.claude_code()),
            "codex" => Some(self.codex()),
            "gemini-cli" => Some(self.gemini()),
            "qwen" => Some(self.qwen()),
            "antigravity-cli" => Some(self.antigravity_cli()),
            "copilot" => Some(self.copilot_cli()),
            _ => None,
        }
    }

    fn cursor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            catalog_id: "cursor",
            source: AgentSource::Cursor,
            template_path: self.integrations_root.join("cursor/hooks.json.template"),
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
            catalog_id: "claude-code",
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
            catalog_id: "codex",
            source: AgentSource::Codex,
            template_path: self.integrations_root.join("codex/hooks.json.template"),
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

    fn gemini(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            catalog_id: "gemini-cli",
            source: AgentSource::Gemini,
            template_path: self
                .integrations_root
                .join("gemini/settings.hooks.template.json"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".gemini/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".gemini/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            external_trust_actions: Vec::new(),
        }
    }

    fn qwen(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            catalog_id: "qwen",
            source: AgentSource::Qwen,
            template_path: self
                .integrations_root
                .join("qwen/settings.hooks.template.json"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".qwen/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".qwen/settings.json"),
                format: ConfigFormat::ClaudeSettings,
            },
            external_trust_actions: Vec::new(),
        }
    }

    fn antigravity_cli(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            catalog_id: "antigravity-cli",
            source: AgentSource::AntigravityCli,
            template_path: self
                .integrations_root
                .join("antigravity-cli/hooks.json.template"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".gemini/antigravity-cli/hooks.json"),
                format: ConfigFormat::AntigravityNamedHooks,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".agents/hooks.json"),
                format: ConfigFormat::AntigravityNamedHooks,
            },
            external_trust_actions: Vec::new(),
        }
    }

    fn copilot_cli(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            catalog_id: "copilot",
            source: AgentSource::CopilotCli,
            template_path: self.integrations_root.join("copilot/hooks.json.template"),
            user_target: TargetFile {
                relative_path: PathBuf::from(".copilot/hooks/llm-notch.json"),
                format: ConfigFormat::HooksJson,
            },
            project_target: TargetFile {
                relative_path: PathBuf::from(".github/hooks/llm-notch.json"),
                format: ConfigFormat::HooksJson,
            },
            external_trust_actions: Vec::new(),
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
        return materialize_wrapper_command(command, &quoted_helper);
    }
    if command.contains(WRAPPER_ABSOLUTE_PLACEHOLDER) {
        return materialize_wrapper_command(command, &quoted_helper);
    }

    if let Some(idx) = command.find("--source") {
        let suffix = command[idx..].trim();
        return format!("{quoted_helper} hook {suffix}");
    }

    if command.contains("llm-notch-hook-wrapper") {
        format!("{quoted_helper}")
    } else {
        helper_path.display().to_string()
    }
}

fn materialize_wrapper_command(command: &str, quoted_helper: &str) -> String {
    let mut materialized = command
        .replace(WRAPPER_ABSOLUTE_PLACEHOLDER, quoted_helper)
        .replace(WRAPPER_PLACEHOLDER, quoted_helper);
    if cfg!(windows) {
        materialized = strip_windows_sh_prefix(&materialized);
    }
    if !materialized.contains(" hook ") && materialized.contains("--source") {
        if let Some(idx) = materialized.find("--source") {
            let suffix = materialized[idx..].trim();
            return format!("{quoted_helper} hook {suffix}");
        }
    }
    materialized
}

/// Codex templates prefix wrapper paths with `sh`; Windows runs the helper directly.
fn strip_windows_sh_prefix(command: &str) -> String {
    let trimmed = command.trim_start();
    if let Some(rest) = trimmed.strip_prefix("sh ") {
        return rest.trim_start().to_string();
    }
    command.to_string()
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
    fn registry_loads_gemini_settings_template() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry =
            AdapterRegistry::new(root.clone(), root.join("target/fake/llm-notch-hook.exe"));
        let adapter = registry.get(AgentSource::Gemini).expect("gemini");
        let template = adapter.load_template().expect("template");

        assert_eq!(
            adapter.user_target.relative_path,
            PathBuf::from(".gemini/settings.json")
        );
        assert!(template["hooks"]["SessionStart"].is_array());
        assert!(template["hooks"]["BeforeTool"].is_array());
        assert!(template["hooks"]["Notification"].is_array());
        assert!(template["hooks"]["SessionEnd"].is_array());
        assert!(template["hooks"].get("AfterAgent").is_none());
    }

    #[test]
    fn registry_loads_qwen_settings_template() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry =
            AdapterRegistry::new(root.clone(), root.join("target/fake/llm-notch-hook.exe"));
        let adapter = registry
            .get_by_catalog_id("qwen")
            .expect("qwen catalog adapter");
        let template = adapter.load_template().expect("template");

        assert_eq!(
            adapter.user_target.relative_path,
            PathBuf::from(".qwen/settings.json")
        );
        assert_eq!(adapter.catalog_id, "qwen");
        assert_eq!(adapter.source, AgentSource::Qwen);
        assert!(template["hooks"]["SessionStart"].is_array());
        assert!(template["hooks"]["PreToolUse"].is_array());
        assert!(template["hooks"]["PermissionRequest"].is_array());
    }

    #[test]
    fn registry_loads_antigravity_named_hook_template() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry =
            AdapterRegistry::new(root.clone(), root.join("target/fake/llm-notch-hook.exe"));
        let adapter = registry
            .get_by_catalog_id("antigravity-cli")
            .expect("antigravity catalog adapter");
        let template = adapter.load_template().expect("template");

        assert_eq!(
            adapter.project_target.relative_path,
            PathBuf::from(".agents/hooks.json")
        );
        assert_eq!(adapter.source, AgentSource::AntigravityCli);
        assert!(template["llm-notch"]["PreToolUse"].is_array());
        assert!(template["llm-notch"]["Stop"].is_array());
    }

    #[test]
    fn registry_loads_copilot_hook_template() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let registry =
            AdapterRegistry::new(root.clone(), root.join("target/fake/llm-notch-hook.exe"));
        let adapter = registry
            .get_by_catalog_id("copilot")
            .expect("copilot catalog adapter");
        let template = adapter.load_template().expect("template");

        assert_eq!(
            adapter.user_target.relative_path,
            PathBuf::from(".copilot/hooks/llm-notch.json")
        );
        assert_eq!(adapter.catalog_id, "copilot");
        assert_eq!(adapter.source, AgentSource::CopilotCli);
        assert_eq!(template["version"], 1);
        assert!(template["hooks"]["sessionStart"].is_array());
        assert!(template["hooks"]["preToolUse"].is_array());
        assert!(template["hooks"]["permissionRequest"].is_array());
        assert!(template["hooks"]["agentStop"].is_array());
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

    #[test]
    fn materialize_codex_template_uses_hook_subcommand_without_sh() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations");
        let helper = PathBuf::from(r"C:\Program Files\llm_notch\llm-notch-hook.exe");
        let registry = AdapterRegistry::new(root.clone(), helper.clone());
        let adapter = registry.get(AgentSource::Codex).expect("codex");
        let materialized =
            materialize_template(&adapter.load_template().expect("template"), &helper);
        let command = materialized["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .expect("command");
        assert!(
            !command.starts_with("sh "),
            "Windows Codex commands must not use sh: {command}"
        );
        assert!(
            command.contains(" hook --source codex --vendor-event SessionStart"),
            "expected hook subcommand: {command}"
        );
        assert!(
            command.contains("llm-notch-hook.exe"),
            "expected helper path in command: {command}"
        );
    }

    #[test]
    fn strip_windows_sh_prefix_removes_leading_sh() {
        assert_eq!(
            strip_windows_sh_prefix("sh \"C:\\helper.exe\" --source codex"),
            "\"C:\\helper.exe\" --source codex"
        );
    }
}
