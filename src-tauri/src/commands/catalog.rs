use notch_agent_catalog::{AgentCatalog, IntegrationDescriptor};

/// Returns catalog metadata only. Catalog presence is not an implementation claim;
/// callers must inspect each descriptor's maturity and capability evidence.
#[tauri::command]
pub fn list_agent_catalog() -> Vec<IntegrationDescriptor> {
    AgentCatalog::vibe_island_25().integrations().to_vec()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use notch_agent_catalog::IntegrationMaturity;

    use super::*;

    #[test]
    fn command_exposes_25_entries_and_only_verified_current_adapters() {
        let entries = list_agent_catalog();
        assert_eq!(entries.len(), 25);

        let verified: HashSet<_> = entries
            .iter()
            .filter(|entry| entry.maturity == IntegrationMaturity::VerifiedCurrent)
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(
            verified,
            HashSet::from([
                "antigravity-cli",
                "claude-code",
                "codex",
                "copilot",
                "cursor",
                "gemini-cli",
                "qwen"
            ])
        );
    }
}
