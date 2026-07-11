use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notch_protocol::{AgentSource, CONNECTOR_PLAN_TTL_MS, ConnectorScope};
use parking_lot::Mutex;
use uuid::Uuid;

use crate::adapter::PlanOperation;
use crate::error::ConnectorError;

#[derive(Debug, Clone)]
pub struct PlanFileSnapshot {
    pub canonical_path: PathBuf,
    pub display_path: String,
    pub baseline_sha256: String,
    pub baseline_text: String,
    pub merged_text: String,
    pub foreign_entries_preserved: Vec<String>,
    pub is_new_file: bool,
    pub backup_display_path: String,
}

#[derive(Debug, Clone)]
pub struct StoredPlan {
    pub plan_id: String,
    pub source: AgentSource,
    pub scope: ConnectorScope,
    pub operation: PlanOperation,
    pub expires_at_ms: i64,
    pub summary: String,
    pub files: Vec<PlanFileSnapshot>,
    pub rollback_backup_id: Option<String>,
}

pub struct PlanStore {
    plans: Mutex<HashMap<String, StoredPlan>>,
}

impl PlanStore {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, plan: StoredPlan) {
        self.plans.lock().insert(plan.plan_id.clone(), plan);
    }

    pub fn get_valid(&self, plan_id: &str, now_ms: i64) -> Result<StoredPlan, ConnectorError> {
        let plans = self.plans.lock();
        let plan = plans
            .get(plan_id)
            .cloned()
            .ok_or(ConnectorError::PlanNotFound)?;
        if now_ms >= plan.expires_at_ms {
            return Err(ConnectorError::PlanExpired);
        }
        Ok(plan)
    }

    pub fn remove(&self, plan_id: &str) {
        self.plans.lock().remove(plan_id);
    }
}

pub fn new_plan_id() -> String {
    format!("plan-{}", Uuid::new_v4().simple())
}

pub fn plan_expires_at(now_ms: i64) -> i64 {
    now_ms + i64::try_from(CONNECTOR_PLAN_TTL_MS).expect("CONNECTOR_PLAN_TTL_MS fits in i64")
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_plan_rejected() {
        let store = PlanStore::new();
        let plan = StoredPlan {
            plan_id: "plan-1".into(),
            source: AgentSource::Cursor,
            scope: ConnectorScope::User,
            operation: PlanOperation::Install,
            expires_at_ms: 100,
            summary: String::new(),
            files: Vec::new(),
            rollback_backup_id: None,
        };
        store.insert(plan);
        assert!(matches!(
            store.get_valid("plan-1", 101),
            Err(ConnectorError::PlanExpired)
        ));
    }
}
