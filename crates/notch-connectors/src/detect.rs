use std::path::Path;

use notch_agent_catalog::AgentCatalog;
use notch_protocol::{AgentSource, ConnectorScope};
use serde::{Deserialize, Serialize};

use crate::adapter::AdapterRegistry;
use crate::error::ConnectorError;
use crate::executable::{any_executable, resolve_executable};
use crate::merge::file_managed_commands;
use crate::path_security::ScopeRoot;
use crate::process_scan::process_running_for_catalog;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedConnector {
    pub source: AgentSource,
    pub scope: ConnectorScope,
    pub display_path: String,
    pub config_present: bool,
    pub managed_entries_present: bool,
    pub executable_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    /// Honest evidence from OS process scan; does not imply a verified session.
    #[serde(default)]
    pub process_running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_process_name: Option<String>,
    #[serde(skip)]
    pub managed_commands: Vec<String>,
}

pub fn detect_all(
    registry: &AdapterRegistry,
    workspace_root: Option<&Path>,
) -> Result<Vec<DetectedConnector>, ConnectorError> {
    let mut found = Vec::new();
    for source in registry.supported_sources() {
        found.extend(detect_source(registry, source, workspace_root)?);
    }
    Ok(found)
}

pub fn detect_source(
    registry: &AdapterRegistry,
    source: AgentSource,
    workspace_root: Option<&Path>,
) -> Result<Vec<DetectedConnector>, ConnectorError> {
    let adapter = registry
        .get(source)
        .ok_or_else(|| ConnectorError::NotFound(format!("unsupported source: {source:?}")))?;

    let executable_present = executable_present_for(&adapter.catalog_id);
    let executable_path = executable_path_for(&adapter.catalog_id);
    let process_evidence = process_running_for_catalog(&adapter.catalog_id);

    let mut results = Vec::new();
    results.push(probe_scope(
        &adapter,
        ConnectorScope::User,
        &ScopeRoot::user_home()?,
        executable_present,
        executable_path.clone(),
        &process_evidence,
    )?);

    if let Some(workspace) = workspace_root {
        if let Ok(project_root) = ScopeRoot::project(workspace) {
            results.push(probe_scope(
                &adapter,
                ConnectorScope::Project,
                &project_root,
                executable_present,
                executable_path,
                &process_evidence,
            )?);
        }
    }

    Ok(results)
}

fn executable_present_for(catalog_id: &str) -> bool {
    let catalog = AgentCatalog::vibe_island_25();
    let Some(descriptor) = catalog.get(catalog_id) else {
        return false;
    };
    let names: Vec<&str> = descriptor
        .executable_names
        .iter()
        .map(String::as_str)
        .collect();
    any_executable(&names)
}

fn executable_path_for(catalog_id: &str) -> Option<String> {
    let catalog = AgentCatalog::vibe_island_25();
    let descriptor = catalog.get(catalog_id)?;
    let names: Vec<&str> = descriptor
        .executable_names
        .iter()
        .map(String::as_str)
        .collect();
    resolve_executable(&names).map(|path| path.display().to_string())
}

fn probe_scope(
    adapter: &crate::adapter::AdapterDescriptor,
    scope: ConnectorScope,
    root: &ScopeRoot,
    executable_present: bool,
    executable_path: Option<String>,
    process_evidence: &crate::process_scan::ProcessRunningEvidence,
) -> Result<DetectedConnector, ConnectorError> {
    let target = adapter.target_for(scope);
    let display_path = root.display_path(&target.relative_path);
    let canonical = root.resolve(&target.relative_path)?;
    let config_present = canonical.exists();
    let managed_commands = if config_present {
        file_managed_commands(&canonical)
    } else {
        Vec::new()
    };
    let managed_entries_present = !managed_commands.is_empty();

    Ok(DetectedConnector {
        source: adapter.source,
        scope,
        display_path,
        config_present,
        managed_entries_present,
        executable_present,
        executable_path,
        process_running: process_evidence.running,
        running_process_name: process_evidence.matched_name.clone(),
        managed_commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterRegistry;
    use std::path::PathBuf;

    #[test]
    fn detects_missing_user_config() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = AdapterRegistry::new(repo.clone(), repo.join("llm-notch-hook.exe"));
        let results = detect_source(&registry, AgentSource::Cursor, None).expect("detect");
        assert!(
            results
                .iter()
                .any(|entry| entry.scope == ConnectorScope::User)
        );
    }

    #[test]
    fn cursor_executable_is_detected_on_developer_windows_machines() {
        if std::env::var_os("CI").is_some()
            || std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
        {
            return;
        }
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = AdapterRegistry::new(repo.clone(), repo.join("llm-notch-hook.exe"));
        let results = detect_source(&registry, AgentSource::Cursor, None).expect("detect");
        let user = results
            .iter()
            .find(|entry| entry.scope == ConnectorScope::User)
            .expect("user scope");
        #[cfg(windows)]
        assert!(user.executable_present, "expected cursor on PATH");
    }
}
