//! History and connector data purge scope contracts.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Purge scope for privacy controls. Backups are kept by default; `include_backups`
/// is an explicit opt-in. "Delete all data" must handle active connectors first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct PurgeScope {
    #[serde(default)]
    pub history: bool,
    #[serde(default)]
    pub session_events: bool,
    #[serde(default)]
    pub connector_journal: bool,
    /// Explicit opt-in to delete connector backup files.
    #[serde(default)]
    pub include_backups: bool,
}

impl Default for PurgeScope {
    fn default() -> Self {
        Self {
            history: true,
            session_events: true,
            connector_journal: false,
            include_backups: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct PurgeResult {
    #[ts(type = "number")]
    pub history_rows_removed: u64,
    #[ts(type = "number")]
    pub events_removed: u64,
    #[ts(type = "number")]
    pub backups_removed: u64,
    pub active_connectors_disconnected: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_scope_defaults_keep_backups() {
        let scope = PurgeScope::default();
        assert!(scope.history);
        assert!(!scope.include_backups);
    }

    #[test]
    fn purge_scope_round_trips_include_backups_opt_in() {
        let scope = PurgeScope {
            history: true,
            session_events: true,
            connector_journal: true,
            include_backups: true,
        };
        let value = serde_json::to_value(&scope).expect("serialize");
        assert_eq!(value["includeBackups"], true);
        let decoded: PurgeScope = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, scope);
    }
}
