//! Migration registry extension points for parallel implementation lanes.
//!
//! Lanes register applied migrations here; this crate defines numbering only.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Registry format version; bump when record shape changes.
pub const MIGRATION_REGISTRY_VERSION: u16 = 1;

/// Lane identifiers for future migrations. Values are stable wire numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum MigrationLane {
    Connectors = 1,
    Decisions = 2,
    Metrics = 3,
    Platform = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct MigrationRecord {
    pub lane: MigrationLane,
    pub version: u32,
    #[ts(type = "number")]
    pub applied_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct MigrationRegistry {
    pub registry_version: u16,
    pub records: Vec<MigrationRecord>,
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self {
            registry_version: MIGRATION_REGISTRY_VERSION,
            records: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_lane_numbers_are_stable() {
        assert_eq!(MigrationLane::Connectors as u32, 1);
        assert_eq!(MigrationLane::Decisions as u32, 2);
    }

    #[test]
    fn migration_registry_round_trips() {
        let registry = MigrationRegistry {
            registry_version: MIGRATION_REGISTRY_VERSION,
            records: vec![MigrationRecord {
                lane: MigrationLane::Connectors,
                version: 1,
                applied_at_ms: 1_700_000_000_000,
                checksum: Some("sha256:abc".into()),
            }],
        };
        let value = serde_json::to_value(&registry).expect("serialize");
        assert_eq!(value["records"][0]["lane"], "connectors");
        let decoded: MigrationRegistry = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, registry);
    }
}
