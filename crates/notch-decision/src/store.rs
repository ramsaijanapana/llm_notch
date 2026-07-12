//! SQLite schema for decision audit trail.

use std::path::Path;

use std::sync::Mutex;

use notch_protocol::{
    DecisionDeliveryState, DecisionKind, DecisionRequest, DecisionResponse, DecisionResponseRecord,
    MAX_DECISION_SUMMARY_LEN, MIGRATION_REGISTRY_VERSION, MigrationLane, MigrationRecord,
    MigrationRegistry,
};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

pub const DECISIONS_MIGRATION_VERSION: u32 = 1;

pub const MIGRATION_003: &str = r#"
CREATE TABLE IF NOT EXISTS decision_audit (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    source TEXT NOT NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    has_actionable_payload INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER,
    delivery_state TEXT NOT NULL,
    response_json TEXT,
    responded_at_ms INTEGER,
    delivery_detail TEXT,
    vendor_event TEXT NOT NULL,
    external_session_id TEXT NOT NULL,
    connection_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decision_audit_pending
    ON decision_audit(delivery_state, created_at_ms DESC);
"#;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

pub struct DecisionStore {
    conn: Mutex<Connection>,
}

impl DecisionStore {
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let conn = Connection::open(path)?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> StoreResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(crate::migration::HOST_BOOTSTRAP_FOR_TESTS)?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("decision sqlite mutex poisoned")
    }

    fn init(conn: &Connection) -> StoreResult<()> {
        let version: Option<i32> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();
        if version.is_none() {
            return Err(StoreError::Other(
                "decision store requires host schema_version table".into(),
            ));
        }
        conn.execute_batch(MIGRATION_003)?;
        record_lane_migration(conn)?;
        Ok(())
    }

    pub fn upsert_active(
        &self,
        request: &DecisionRequest,
        vendor_event: &str,
        external_session_id: &str,
        connection_id: &str,
        delivery_state: DecisionDeliveryState,
    ) -> StoreResult<()> {
        let conn = self.connection();
        conn.execute(
            "INSERT INTO decision_audit (
                id, session_id, source, kind, summary, has_actionable_payload,
                created_at_ms, expires_at_ms, delivery_state, vendor_event,
                external_session_id, connection_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                delivery_state = excluded.delivery_state,
                response_json = COALESCE(decision_audit.response_json, excluded.response_json),
                responded_at_ms = COALESCE(decision_audit.responded_at_ms, excluded.responded_at_ms),
                delivery_detail = COALESCE(decision_audit.delivery_detail, excluded.delivery_detail)",
            params![
                request.id,
                request.session_id,
                format!("{:?}", request.source),
                format!("{:?}", request.kind),
                request.summary,
                i32::from(request.has_actionable_payload),
                request.created_at_ms,
                request.expires_at_ms,
                format!("{:?}", delivery_state),
                vendor_event,
                external_session_id,
                connection_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_delivery(
        &self,
        request_id: &str,
        delivery_state: DecisionDeliveryState,
        response: Option<&DecisionResponse>,
        responded_at_ms: Option<i64>,
        delivery_detail: Option<&str>,
    ) -> StoreResult<()> {
        let response_json = response.map(serde_json::to_string).transpose()?;
        let conn = self.connection();
        conn.execute(
            "UPDATE decision_audit SET
                delivery_state = ?2,
                response_json = COALESCE(?3, response_json),
                responded_at_ms = COALESCE(?4, responded_at_ms),
                delivery_detail = COALESCE(?5, delivery_detail)
             WHERE id = ?1",
            params![
                request_id,
                format!("{:?}", delivery_state),
                response_json,
                responded_at_ms,
                delivery_detail,
            ],
        )?;
        Ok(())
    }

    pub fn list_pending_requests(&self, now_ms: i64) -> StoreResult<Vec<DecisionRequest>> {
        let conn = self.connection();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source, kind, summary, has_actionable_payload,
                    created_at_ms, expires_at_ms
             FROM decision_audit
             WHERE delivery_state = 'Pending'
               AND (expires_at_ms IS NULL OR expires_at_ms > ?1)
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map([now_ms], |row| {
            let source: String = row.get(2)?;
            let kind: String = row.get(3)?;
            Ok(DecisionRequest {
                id: row.get(0)?,
                session_id: row.get(1)?,
                source: parse_agent_source(&source),
                kind: parse_decision_kind(&kind),
                summary: row.get(4)?,
                has_actionable_payload: row.get::<_, i32>(5)? != 0,
                created_at_ms: row.get(6)?,
                expires_at_ms: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn load_record(&self, request_id: &str) -> StoreResult<Option<DecisionResponseRecord>> {
        let conn = self.connection();
        let mut stmt = conn.prepare(
            "SELECT response_json, responded_at_ms, delivery_state, delivery_detail
             FROM decision_audit WHERE id = ?1",
        )?;
        let mut rows = stmt.query([request_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let response_json: Option<String> = row.get(0)?;
        let responded_at_ms: Option<i64> = row.get(1)?;
        let delivery_state: String = row.get(2)?;
        let delivery_detail: Option<String> = row.get(3)?;
        let (Some(response_json), Some(responded_at_ms)) = (response_json, responded_at_ms) else {
            return Ok(None);
        };
        Ok(Some(DecisionResponseRecord {
            request_id: request_id.into(),
            response: serde_json::from_str(&response_json)?,
            responded_at_ms,
            delivery_state: parse_delivery_state(&delivery_state),
            delivery_detail,
        }))
    }
}

fn record_lane_migration(conn: &Connection) -> StoreResult<()> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'migration_registry'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let mut registry: MigrationRegistry = existing
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_default();
    if registry.records.iter().any(|record| {
        record.lane == MigrationLane::Decisions && record.version == DECISIONS_MIGRATION_VERSION
    }) {
        return Ok(());
    }
    registry.records.push(MigrationRecord {
        lane: MigrationLane::Decisions,
        version: DECISIONS_MIGRATION_VERSION,
        applied_at_ms: chrono::Utc::now().timestamp_millis(),
        checksum: None,
    });
    registry.registry_version = MIGRATION_REGISTRY_VERSION;
    let encoded = serde_json::to_string(&registry)?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('migration_registry', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [encoded],
    )?;
    Ok(())
}

fn parse_agent_source(raw: &str) -> notch_protocol::AgentSource {
    match raw {
        "Cursor" => notch_protocol::AgentSource::Cursor,
        "ClaudeCode" => notch_protocol::AgentSource::ClaudeCode,
        "Codex" => notch_protocol::AgentSource::Codex,
        "Gemini" => notch_protocol::AgentSource::Gemini,
        "Generic" => notch_protocol::AgentSource::Generic,
        _ => notch_protocol::AgentSource::Unknown,
    }
}

fn parse_decision_kind(raw: &str) -> DecisionKind {
    match raw {
        "Approval" => DecisionKind::Approval,
        "Question" => DecisionKind::Question,
        _ => DecisionKind::Permission,
    }
}

fn parse_delivery_state(raw: &str) -> DecisionDeliveryState {
    match raw {
        "Delivered" => DecisionDeliveryState::Delivered,
        "EffectObserved" => DecisionDeliveryState::EffectObserved,
        "Expired" => DecisionDeliveryState::Expired,
        "Failed" => DecisionDeliveryState::Failed,
        _ => DecisionDeliveryState::Pending,
    }
}

pub fn truncate_summary(summary: &str) -> String {
    if summary.len() <= MAX_DECISION_SUMMARY_LEN {
        return summary.to_string();
    }
    summary.chars().take(MAX_DECISION_SUMMARY_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::AgentSource;

    fn bootstrap_host_schema(conn: &Connection) {
        conn.execute_batch(crate::migration::HOST_BOOTSTRAP_FOR_TESTS)
            .expect("bootstrap");
    }

    #[test]
    fn migration_records_decisions_lane() {
        let conn = Connection::open_in_memory().expect("memory");
        bootstrap_host_schema(&conn);
        DecisionStore::init(&conn).expect("init");
        let value: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'migration_registry'",
                [],
                |row| row.get(0),
            )
            .expect("registry");
        let registry: MigrationRegistry = serde_json::from_str(&value).expect("parse");
        assert!(registry.records.iter().any(|record| {
            record.lane == MigrationLane::Decisions && record.version == DECISIONS_MIGRATION_VERSION
        }));
    }

    #[test]
    fn pending_list_excludes_expired_rows() {
        let store = DecisionStore::in_memory().expect("store");
        let request = DecisionRequest {
            id: "dec-1".into(),
            session_id: "sess-1".into(),
            source: AgentSource::ClaudeCode,
            kind: DecisionKind::Permission,
            summary: "Allow?".into(),
            has_actionable_payload: true,
            created_at_ms: 1,
            expires_at_ms: Some(100),
        };
        store
            .upsert_active(
                &request,
                "PermissionRequest",
                "ext-1",
                "conn-1",
                DecisionDeliveryState::Pending,
            )
            .expect("insert");
        assert_eq!(store.list_pending_requests(50).expect("list").len(), 1);
        assert!(store.list_pending_requests(200).expect("list").is_empty());
    }
}
