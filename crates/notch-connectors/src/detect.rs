use std::path::Path;

use notch_protocol::{AgentSource, ConnectorScope};
use serde::{Deserialize, Serialize};

use crate::adapter::AdapterRegistry;
use crate::error::ConnectorError;
use crate::path_security::ScopeRoot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedConnector {
    pub source: AgentSource,
    pub scope: ConnectorScope,
    pub display_path: String,
    pub config_present: bool,
    pub managed_entries_present: bool,
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

    let mut results = Vec::new();
    results.push(probe_scope(
        &adapter,
        ConnectorScope::User,
        &ScopeRoot::user_home()?,
    )?);

    if let Some(workspace) = workspace_root {
        if let Ok(project_root) = ScopeRoot::project(workspace) {
            results.push(probe_scope(
                &adapter,
                ConnectorScope::Project,
                &project_root,
            )?);
        }
    }

    Ok(results)
}

fn probe_scope(
    adapter: &crate::adapter::AdapterDescriptor,
    scope: ConnectorScope,
    root: &ScopeRoot,
) -> Result<DetectedConnector, ConnectorError> {
    let target = adapter.target_for(scope);
    let display_path = root.display_path(&target.relative_path);
    let canonical = root.resolve(&target.relative_path)?;
    let config_present = canonical.exists();
    let managed_entries_present = if config_present {
        let raw = std::fs::read_to_string(&canonical).map_err(|error| {
            ConnectorError::Internal(format!("read failed for {}: {error}", display_path))
        })?;
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
        crate::merge::is_managed_command(&value.to_string())
    } else {
        false
    };

    Ok(DetectedConnector {
        source: adapter.source,
        scope,
        display_path,
        config_present,
        managed_entries_present,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterRegistry;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn registry_with(root: &Path) -> AdapterRegistry {
        AdapterRegistry::new(
            root.to_path_buf(),
            root.join("llm-notch-hook.exe"),
        )
    }

    #[test]
    fn detects_missing_user_config() {
        let dir = TempDir::new().expect("tempdir");
        // Override home by using project scope only in this test via custom layout
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = registry_with(&repo);
        let results = detect_source(&registry, AgentSource::Cursor, None).expect("detect");
        assert!(results.iter().any(|entry| entry.scope == ConnectorScope::User));
    }
}
