use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use notch_protocol::{
    AdapterCapabilities, AgentSession, AgentSource, AttentionKind, AttributionQuality, EventLevel,
    IoQuality, MetricAvailability, MetricQuality, MetricSample, ProcessIdentity, PublicSettings,
    SessionEvent, SessionEventKind, SessionStatus,
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::constants::{
    EVENT_RETENTION_MS, HOST_METRIC_SESSION_KEY, MAX_EVENTS_PER_SESSION, MAX_SESSIONS,
    METRIC_RETENTION_MS, PRUNE_TARGET_BYTES,
};
use crate::error::{CoreError, CoreResult};
use crate::persistence::migrations::apply_migrations;

fn db_err(err: impl Into<anyhow::Error>) -> CoreError {
    CoreError::Other(err.into())
}

/// Report from a retention or purge pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PurgeReport {
    pub events_deleted: u64,
    pub metric_buckets_deleted: u64,
    pub bytes_after: u64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PersistedMetricHistory {
    pub requested_start_ms: i64,
    pub requested_end_ms: i64,
    pub host: PersistedMetricSeries,
    pub aggregate: PersistedMetricSeries,
    pub agents: BTreeMap<String, PersistedMetricSeries>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PersistedMetricSeries {
    pub points: Vec<MetricSample>,
    pub actual_first_ms: Option<i64>,
    pub actual_last_ms: Option<i64>,
    pub total_points: u64,
    pub downsampled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionEventPage {
    pub events: Vec<SessionEvent>,
    pub next_before_sequence: Option<u64>,
    pub has_more: bool,
}

/// SQLite-backed durable store for sessions, events, settings, and metric history.
pub struct SqliteRepository {
    conn: Mutex<Connection>,
    path: std::path::PathBuf,
}

impl SqliteRepository {
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(&path).map_err(db_err)?;
        apply_migrations(&conn).map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path,
        })
    }

    pub fn in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        apply_migrations(&conn).map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: std::path::PathBuf::from(":memory:"),
        })
    }

    pub fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("sqlite mutex poisoned")
    }

    pub fn load_sessions(&self) -> CoreResult<Vec<AgentSession>> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, source, external_session_id, label, workspace_label, status, attention,
                        started_at_ms, last_event_at_ms, ended_at_ms,
                        process_root_pid, process_root_started_at_ms, latest_metric_json
                 FROM sessions
                 ORDER BY
                    CASE
                      WHEN status NOT IN ('Completed', 'Failed', 'Stale') OR attention != 'None'
                      THEN 0 ELSE 1
                    END,
                    last_event_at_ms DESC
                 LIMIT ?1",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map([MAX_SESSIONS as i64], |row| {
                let source: String = row.get(1)?;
                let status: String = row.get(5)?;
                let attention: String = row.get(6)?;
                let latest_metric_json: Option<String> = row.get(12)?;
                Ok(AgentSession {
                    id: row.get(0)?,
                    source: parse_source(&source),
                    external_session_id: row.get(2)?,
                    label: row.get(3)?,
                    workspace_label: row.get(4)?,
                    status: parse_status(&status),
                    attention: parse_attention(&attention),
                    started_at_ms: row.get(7)?,
                    last_event_at_ms: row.get(8)?,
                    ended_at_ms: row.get(9)?,
                    process_root: match (
                        row.get::<_, Option<u32>>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                    ) {
                        (Some(pid), Some(started_at_ms)) => {
                            Some(ProcessIdentity { pid, started_at_ms })
                        }
                        _ => None,
                    },
                    latest_metric: latest_metric_json
                        .and_then(|json| serde_json::from_str(&json).ok()),
                })
            })
            .map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn load_events(&self) -> CoreResult<Vec<SessionEvent>> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare(
                "WITH selected_sessions AS (
                    SELECT id
                    FROM sessions
                    ORDER BY
                        CASE
                          WHEN status NOT IN ('Completed', 'Failed', 'Stale') OR attention != 'None'
                          THEN 0 ELSE 1
                        END,
                        last_event_at_ms DESC
                    LIMIT ?1
                 ),
                 ranked_events AS (
                    SELECT
                        events.id,
                        events.session_id,
                        events.sequence,
                        events.occurred_at_ms,
                        events.kind,
                        events.level,
                        events.summary,
                        events.tool_name,
                        ROW_NUMBER() OVER (
                            PARTITION BY events.session_id
                            ORDER BY events.sequence DESC
                        ) AS event_rank
                    FROM session_events events
                    JOIN selected_sessions selected ON selected.id = events.session_id
                 )
                 SELECT id, session_id, sequence, occurred_at_ms, kind, level, summary, tool_name
                 FROM ranked_events
                 WHERE event_rank <= ?2
                 ORDER BY session_id, sequence",
            )
            .map_err(db_err)?;

        let rows = stmt
            .query_map(
                params![MAX_SESSIONS as i64, MAX_EVENTS_PER_SESSION as i64],
                |row| {
                    let id: String = row.get(0)?;
                    let kind: String = row.get(4)?;
                    let level: String = row.get(5)?;
                    Ok(SessionEvent {
                        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::nil()),
                        session_id: row.get(1)?,
                        sequence: row.get::<_, i64>(2)? as u64,
                        occurred_at_ms: row.get(3)?,
                        kind: parse_event_kind(&kind),
                        level: parse_event_level(&level),
                        summary: row.get(6)?,
                        tool_name: row.get(7)?,
                    })
                },
            )
            .map_err(db_err)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    pub fn upsert_session(&self, session: &AgentSession) -> CoreResult<()> {
        let latest_metric_json = session
            .latest_metric
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(db_err)?;

        let conn = self.connection();
        conn.execute(
            "INSERT INTO sessions (
                    id, source, external_session_id, label, workspace_label, status, attention,
                    started_at_ms, last_event_at_ms, ended_at_ms,
                    process_root_pid, process_root_started_at_ms, latest_metric_json
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
                ON CONFLICT(id) DO UPDATE SET
                    label=excluded.label,
                    workspace_label=excluded.workspace_label,
                    status=excluded.status,
                    attention=excluded.attention,
                    last_event_at_ms=excluded.last_event_at_ms,
                    ended_at_ms=excluded.ended_at_ms,
                    process_root_pid=excluded.process_root_pid,
                    process_root_started_at_ms=excluded.process_root_started_at_ms,
                    latest_metric_json=excluded.latest_metric_json",
            params![
                session.id,
                format!("{:?}", session.source),
                session.external_session_id,
                session.label,
                session.workspace_label,
                format!("{:?}", session.status),
                format!("{:?}", session.attention),
                session.started_at_ms,
                session.last_event_at_ms,
                session.ended_at_ms,
                session.process_root.as_ref().map(|p| p.pid),
                session.process_root.as_ref().map(|p| p.started_at_ms),
                latest_metric_json,
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn remove_session(&self, session_id: &str) -> CoreResult<()> {
        let conn = self.connection();
        conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn append_event(&self, event: &SessionEvent) -> CoreResult<()> {
        {
            let conn = self.connection();
            conn.execute(
                "INSERT INTO session_events
                    (id, session_id, sequence, occurred_at_ms, kind, level, summary, tool_name)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    event.id.to_string(),
                    event.session_id,
                    event.sequence as i64,
                    event.occurred_at_ms,
                    format!("{:?}", event.kind),
                    format!("{:?}", event.level),
                    event.summary,
                    event.tool_name,
                ],
            )
            .map_err(db_err)?;
        }

        self.trim_session_events(&event.session_id)?;
        Ok(())
    }

    pub fn load_session_event_page(
        &self,
        session_id: &str,
        before_sequence: Option<u64>,
        limit: usize,
    ) -> CoreResult<SessionEventPage> {
        let conn = self.connection();
        let mut statement = conn
            .prepare(
                "SELECT id, session_id, sequence, occurred_at_ms, kind, level, summary, tool_name
                 FROM session_events
                 WHERE session_id = ?1
                   AND (?2 IS NULL OR sequence < ?2)
                 ORDER BY sequence DESC
                 LIMIT ?3",
            )
            .map_err(db_err)?;
        let rows = statement
            .query_map(
                params![
                    session_id,
                    before_sequence.map(|sequence| sequence as i64),
                    limit.saturating_add(1) as i64
                ],
                |row| {
                    let id: String = row.get(0)?;
                    let kind: String = row.get(4)?;
                    let level: String = row.get(5)?;
                    Ok(SessionEvent {
                        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::nil()),
                        session_id: row.get(1)?,
                        sequence: row.get::<_, i64>(2)? as u64,
                        occurred_at_ms: row.get(3)?,
                        kind: parse_event_kind(&kind),
                        level: parse_event_level(&level),
                        summary: row.get(6)?,
                        tool_name: row.get(7)?,
                    })
                },
            )
            .map_err(db_err)?;
        let mut events = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
        let has_more = events.len() > limit;
        if has_more {
            events.pop();
        }
        events.reverse();
        Ok(SessionEventPage {
            next_before_sequence: has_more
                .then(|| events.first().map(|event| event.sequence))
                .flatten(),
            events,
            has_more,
        })
    }

    pub fn load_settings(&self) -> CoreResult<Option<PublicSettings>> {
        let conn = self.connection();
        let json: Option<String> = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key = 'public'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_err)?;
        json.map(|value| serde_json::from_str(&value).map_err(db_err))
            .transpose()
    }

    pub fn save_settings(&self, settings: &PublicSettings) -> CoreResult<()> {
        let json = serde_json::to_string(settings).map_err(db_err)?;
        let conn = self.connection();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('public', ?1)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            [json],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn upsert_integration(
        &self,
        capabilities: &AdapterCapabilities,
        healthy: bool,
        message: Option<&str>,
        updated_at_ms: i64,
    ) -> CoreResult<()> {
        let json = serde_json::to_string(capabilities).map_err(db_err)?;
        let conn = self.connection();
        conn
            .execute(
                "INSERT INTO integration_health (source, capabilities_json, healthy, message, updated_at_ms)
                 VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(source) DO UPDATE SET
                    capabilities_json=excluded.capabilities_json,
                    healthy=excluded.healthy,
                    message=excluded.message,
                    updated_at_ms=excluded.updated_at_ms",
                params![
                    format!("{:?}", capabilities.source),
                    json,
                    i32::from(healthy),
                    message,
                    updated_at_ms,
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn load_integrations(&self) -> CoreResult<Vec<AdapterCapabilities>> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare("SELECT capabilities_json FROM integration_health")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(db_err)?;

        rows.map(|row| {
            let json = row.map_err(db_err)?;
            serde_json::from_str(&json).map_err(db_err)
        })
        .collect()
    }

    pub fn record_metric_bucket(
        &self,
        scope: &str,
        session_id: Option<&str>,
        bucket_start_ms: i64,
        sample: &MetricSample,
    ) -> CoreResult<()> {
        let session_id = session_id.unwrap_or(HOST_METRIC_SESSION_KEY);
        let conn = self.connection();
        conn.execute(
            "INSERT INTO metric_buckets (
                    scope, session_id, bucket_start_ms, cpu_host_percent, cpu_core_percent,
                    rss_bytes, runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                 ON CONFLICT(scope, session_id, bucket_start_ms) DO UPDATE SET
                    cpu_host_percent=excluded.cpu_host_percent,
                    cpu_core_percent=excluded.cpu_core_percent,
                    rss_bytes=excluded.rss_bytes,
                    runtime_ms=excluded.runtime_ms,
                    process_count=excluded.process_count,
                    read_bytes_per_sec=excluded.read_bytes_per_sec,
                    write_bytes_per_sec=excluded.write_bytes_per_sec",
            params![
                scope,
                session_id,
                bucket_start_ms,
                sample.cpu_host_percent,
                sample.cpu_core_percent,
                sample.rss_bytes as i64,
                sample.runtime_ms as i64,
                sample.process_count,
                sample.read_bytes_per_sec as i64,
                sample.write_bytes_per_sec as i64,
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn load_metric_history(
        &self,
        session_id: &str,
        limit: usize,
    ) -> CoreResult<Vec<MetricSample>> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare(
                "SELECT bucket_start_ms, cpu_host_percent, cpu_core_percent, rss_bytes,
                        runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec
                 FROM metric_buckets
                 WHERE scope = 'agent' AND session_id = ?1
                 ORDER BY bucket_start_ms DESC
                 LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![session_id, limit as i64], |row| {
                Ok(MetricSample {
                    at_ms: row.get(0)?,
                    cpu_host_percent: row.get(1)?,
                    cpu_core_percent: row.get(2)?,
                    rss_bytes: non_negative_u64(row.get::<_, i64>(3)?),
                    runtime_ms: non_negative_u64(row.get::<_, i64>(4)?),
                    process_count: row.get(5)?,
                    read_bytes_per_sec: non_negative_u64(row.get::<_, i64>(6)?),
                    write_bytes_per_sec: non_negative_u64(row.get::<_, i64>(7)?),
                    quality: MetricQuality {
                        attribution: AttributionQuality::Unknown,
                        cpu: MetricAvailability::Available,
                        io: IoQuality::Unavailable,
                        reason: Some(
                            "historical quality metadata is not persisted in schema v2".into(),
                        ),
                    },
                })
            })
            .map_err(db_err)?;
        let mut samples = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
        samples.reverse();
        Ok(samples)
    }

    pub fn load_persisted_metric_history(
        &self,
        requested_start_ms: i64,
        requested_end_ms: i64,
        max_points_per_series: usize,
    ) -> CoreResult<PersistedMetricHistory> {
        if max_points_per_series < 2 || requested_end_ms < requested_start_ms {
            return Err(CoreError::Validation(
                "invalid metric history range or per-series limit".into(),
            ));
        }
        let conn = self.connection();
        let mut statement = conn
            .prepare(
                "SELECT DISTINCT session_id
                 FROM metric_buckets
                 WHERE scope = 'agent'
                   AND bucket_start_ms BETWEEN ?1 AND ?2
                 ORDER BY session_id",
            )
            .map_err(db_err)?;
        let rows = statement
            .query_map(params![requested_start_ms, requested_end_ms], |row| {
                row.get::<_, String>(0)
            })
            .map_err(db_err)?;
        let session_ids = rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
        drop(statement);

        let mut agents = BTreeMap::new();
        for session_id in session_ids {
            agents.insert(
                session_id.clone(),
                load_metric_series(
                    &conn,
                    "agent",
                    &session_id,
                    requested_start_ms,
                    requested_end_ms,
                    max_points_per_series,
                )?,
            );
        }

        Ok(PersistedMetricHistory {
            requested_start_ms,
            requested_end_ms,
            host: load_metric_series(
                &conn,
                "host",
                HOST_METRIC_SESSION_KEY,
                requested_start_ms,
                requested_end_ms,
                max_points_per_series,
            )?,
            aggregate: load_metric_series(
                &conn,
                "aggregate",
                HOST_METRIC_SESSION_KEY,
                requested_start_ms,
                requested_end_ms,
                max_points_per_series,
            )?,
            agents,
        })
    }

    pub fn purge_metric_history(&self) -> CoreResult<u64> {
        let mut conn = self.connection();
        let transaction = conn.transaction().map_err(db_err)?;
        let deleted = transaction
            .execute("DELETE FROM metric_buckets", [])
            .map_err(db_err)? as u64;
        transaction
            .execute("UPDATE sessions SET latest_metric_json = NULL", [])
            .map_err(db_err)?;
        transaction.commit().map_err(db_err)?;
        Ok(deleted)
    }

    pub fn store_idempotency_key(&self, key: &str, created_at_ms: i64) -> CoreResult<bool> {
        let conn = self.connection();
        let changed = conn
            .execute(
                "INSERT OR IGNORE INTO idempotency_keys (key, created_at_ms) VALUES (?1,?2)",
                params![key, created_at_ms],
            )
            .map_err(db_err)?;
        Ok(changed == 0)
    }

    pub fn prune(&self, now_ms: i64) -> CoreResult<PurgeReport> {
        self.prune_with_metric_retention(now_ms, METRIC_RETENTION_MS)
    }

    pub fn prune_with_metric_retention(
        &self,
        now_ms: i64,
        metric_retention_ms: i64,
    ) -> CoreResult<PurgeReport> {
        let conn = self.connection();
        let event_cutoff = now_ms - EVENT_RETENTION_MS;
        let metric_cutoff = now_ms - metric_retention_ms.max(0);

        let events_deleted = conn
            .execute(
                "DELETE FROM session_events WHERE occurred_at_ms < ?1",
                [event_cutoff],
            )
            .map_err(db_err)? as u64;

        let metric_buckets_deleted = conn
            .execute(
                "DELETE FROM metric_buckets WHERE bucket_start_ms < ?1",
                [metric_cutoff],
            )
            .map_err(db_err)? as u64;

        if metric_buckets_deleted > 0 {
            conn.execute("UPDATE sessions SET latest_metric_json = NULL", [])
                .map_err(db_err)?;
        }

        let mut report = PurgeReport {
            events_deleted,
            metric_buckets_deleted,
            bytes_after: self.database_bytes()?,
        };

        while report.bytes_after > PRUNE_TARGET_BYTES {
            let deleted = conn
                .execute(
                    "DELETE FROM session_events WHERE id IN (
                        SELECT id FROM session_events ORDER BY occurred_at_ms ASC LIMIT 500
                     )",
                    [],
                )
                .map_err(db_err)? as u64;
            if deleted == 0 {
                break;
            }
            report.events_deleted += deleted;
            report.bytes_after = self.database_bytes()?;
        }

        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(db_err)?;

        Ok(report)
    }

    fn trim_session_events(&self, session_id: &str) -> CoreResult<()> {
        let conn = self.connection();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(db_err)?;

        if count > MAX_EVENTS_PER_SESSION as i64 {
            let overflow = count - MAX_EVENTS_PER_SESSION as i64;
            conn.execute(
                "DELETE FROM session_events WHERE id IN (
                        SELECT id FROM session_events WHERE session_id = ?1
                        ORDER BY sequence ASC LIMIT ?2
                     )",
                params![session_id, overflow],
            )
            .map_err(db_err)?;
        }
        Ok(())
    }

    fn database_bytes(&self) -> CoreResult<u64> {
        if self.path == std::path::Path::new(":memory:") {
            return Ok(0);
        }
        let metadata = std::fs::metadata(&self.path).map_err(db_err)?;
        Ok(metadata.len())
    }
}

fn non_negative_u64(value: i64) -> u64 {
    value.max(0) as u64
}

fn load_metric_series(
    conn: &Connection,
    scope: &str,
    session_id: &str,
    requested_start_ms: i64,
    requested_end_ms: i64,
    max_points: usize,
) -> CoreResult<PersistedMetricSeries> {
    let (total_points, actual_first_ms, actual_last_ms): (i64, Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT COUNT(*), MIN(bucket_start_ms), MAX(bucket_start_ms)
             FROM metric_buckets
             WHERE scope = ?1 AND session_id = ?2
               AND bucket_start_ms BETWEEN ?3 AND ?4",
            params![scope, session_id, requested_start_ms, requested_end_ms],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(db_err)?;
    if total_points == 0 {
        return Ok(PersistedMetricSeries::default());
    }

    let stride = ((total_points as usize).saturating_sub(1) + max_points.saturating_sub(2))
        / max_points.saturating_sub(1);
    let mut statement = conn
        .prepare(
            "WITH ranked AS (
                SELECT
                    bucket_start_ms, cpu_host_percent, cpu_core_percent, rss_bytes,
                    runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec,
                    ROW_NUMBER() OVER (ORDER BY bucket_start_ms) AS point_index,
                    COUNT(*) OVER () AS point_count
                FROM metric_buckets
                WHERE scope = ?1 AND session_id = ?2
                  AND bucket_start_ms BETWEEN ?3 AND ?4
             )
             SELECT
                bucket_start_ms, cpu_host_percent, cpu_core_percent, rss_bytes,
                runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec
             FROM ranked
             WHERE point_count <= ?5
                OR point_index = 1
                OR point_index = point_count
                OR ((point_index - 1) % ?6) = 0
             ORDER BY bucket_start_ms",
        )
        .map_err(db_err)?;
    let rows = statement
        .query_map(
            params![
                scope,
                session_id,
                requested_start_ms,
                requested_end_ms,
                max_points as i64,
                stride.max(1) as i64
            ],
            historical_sample_from_row,
        )
        .map_err(db_err)?;
    let points = downsample_preserving_edges(
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?,
        max_points,
    );
    Ok(PersistedMetricSeries {
        points,
        actual_first_ms,
        actual_last_ms,
        total_points: total_points as u64,
        downsampled: total_points as usize > max_points,
    })
}

fn downsample_preserving_edges(points: Vec<MetricSample>, max_points: usize) -> Vec<MetricSample> {
    if points.len() <= max_points {
        return points;
    }
    (0..max_points)
        .filter_map(|index| {
            let source_index =
                index.saturating_mul(points.len().saturating_sub(1)) / max_points.saturating_sub(1);
            points.get(source_index).cloned()
        })
        .collect()
}

fn historical_sample_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MetricSample> {
    historical_sample_from_offset_row(row, 0)
}

fn historical_sample_from_offset_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<MetricSample> {
    Ok(MetricSample {
        at_ms: row.get(offset)?,
        cpu_host_percent: row.get(offset + 1)?,
        cpu_core_percent: row.get(offset + 2)?,
        rss_bytes: non_negative_u64(row.get::<_, i64>(offset + 3)?),
        runtime_ms: non_negative_u64(row.get::<_, i64>(offset + 4)?),
        process_count: row.get(offset + 5)?,
        read_bytes_per_sec: non_negative_u64(row.get::<_, i64>(offset + 6)?),
        write_bytes_per_sec: non_negative_u64(row.get::<_, i64>(offset + 7)?),
        quality: MetricQuality {
            attribution: AttributionQuality::Unknown,
            cpu: MetricAvailability::Available,
            io: IoQuality::Unavailable,
            reason: Some("historical quality metadata is not persisted".into()),
        },
    })
}

fn parse_source(value: &str) -> AgentSource {
    match value {
        "Cursor" => AgentSource::Cursor,
        "ClaudeCode" => AgentSource::ClaudeCode,
        "Codex" => AgentSource::Codex,
        "Generic" => AgentSource::Generic,
        _ => AgentSource::Unknown,
    }
}

fn parse_status(value: &str) -> SessionStatus {
    match value {
        "Starting" => SessionStatus::Starting,
        "Running" => SessionStatus::Running,
        "Waiting" => SessionStatus::Waiting,
        "Paused" => SessionStatus::Paused,
        "Completed" => SessionStatus::Completed,
        "Failed" => SessionStatus::Failed,
        "Stale" => SessionStatus::Stale,
        _ => SessionStatus::Stale,
    }
}

fn parse_attention(value: &str) -> AttentionKind {
    match value {
        "None" => AttentionKind::None,
        "Approval" => AttentionKind::Approval,
        "Question" => AttentionKind::Question,
        "Permission" => AttentionKind::Permission,
        "Error" => AttentionKind::Error,
        _ => AttentionKind::None,
    }
}

fn parse_event_kind(value: &str) -> SessionEventKind {
    match value {
        "Lifecycle" => SessionEventKind::Lifecycle,
        "Tool" => SessionEventKind::Tool,
        "Attention" => SessionEventKind::Attention,
        "Status" => SessionEventKind::Status,
        _ => SessionEventKind::Lifecycle,
    }
}

fn parse_event_level(value: &str) -> EventLevel {
    match value {
        "Debug" => EventLevel::Debug,
        "Info" => EventLevel::Info,
        "Warning" => EventLevel::Warning,
        "Error" => EventLevel::Error,
        _ => EventLevel::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{
        AttentionCapability, AttributionQuality, IoQuality, MetricAvailability, MetricQuality,
    };

    fn sample_session(id: &str) -> AgentSession {
        AgentSession {
            id: id.into(),
            source: AgentSource::Cursor,
            external_session_id: format!("ext-{id}"),
            label: "label".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 1,
            last_event_at_ms: 1,
            ended_at_ms: None,
            process_root: None,
            latest_metric: None,
        }
    }

    #[test]
    fn restart_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notch.db");
        {
            let repo = SqliteRepository::open(&path).unwrap();
            repo.upsert_session(&sample_session("s1")).unwrap();
            repo.append_event(&SessionEvent {
                id: Uuid::new_v4(),
                session_id: "s1".into(),
                sequence: 1,
                occurred_at_ms: 10,
                kind: SessionEventKind::Lifecycle,
                level: EventLevel::Info,
                summary: "started".into(),
                tool_name: None,
            })
            .unwrap();
        }

        let repo = SqliteRepository::open(&path).unwrap();
        let sessions = repo.load_sessions().unwrap();
        let events = repo.load_events().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn retention_prunes_old_rows() {
        let repo = SqliteRepository::in_memory().unwrap();
        repo.upsert_session(&sample_session("s1")).unwrap();
        repo.append_event(&SessionEvent {
            id: Uuid::new_v4(),
            session_id: "s1".into(),
            sequence: 1,
            occurred_at_ms: 1,
            kind: SessionEventKind::Lifecycle,
            level: EventLevel::Info,
            summary: "old".into(),
            tool_name: None,
        })
        .unwrap();
        repo.record_metric_bucket(
            "host",
            None,
            1,
            &MetricSample {
                at_ms: 1,
                cpu_core_percent: 1.0,
                cpu_host_percent: 1.0,
                rss_bytes: 1,
                runtime_ms: 1,
                process_count: 1,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality: MetricQuality {
                    attribution: AttributionQuality::Exact,
                    cpu: MetricAvailability::Available,
                    io: IoQuality::Unavailable,
                    reason: None,
                },
            },
        )
        .unwrap();

        let now = EVENT_RETENTION_MS + METRIC_RETENTION_MS + 10;
        let report = repo.prune(now).unwrap();
        assert_eq!(report.events_deleted, 1);
        assert_eq!(report.metric_buckets_deleted, 1);
        assert!(repo.load_events().unwrap().is_empty());
    }

    #[test]
    fn integration_health_persists() {
        let repo = SqliteRepository::in_memory().unwrap();
        let caps = AdapterCapabilities {
            source: AgentSource::Cursor,
            events: true,
            attention: AttentionCapability::Partial,
            decision_response: false,
            context_open: true,
            process_attribution: AttributionQuality::Shared,
        };
        repo.upsert_integration(&caps, true, Some("ok"), 100)
            .unwrap();
        let loaded = repo.load_integrations().unwrap();
        assert_eq!(loaded, vec![caps]);
    }

    #[test]
    fn host_metric_bucket_upserts_are_unique() {
        let repo = SqliteRepository::in_memory().unwrap();
        let mut sample = MetricSample {
            at_ms: 1,
            cpu_core_percent: 1.0,
            cpu_host_percent: 1.0,
            rss_bytes: 1,
            runtime_ms: 1,
            process_count: 1,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            quality: MetricQuality {
                attribution: AttributionQuality::Unknown,
                cpu: MetricAvailability::Available,
                io: IoQuality::Unavailable,
                reason: None,
            },
        };
        repo.record_metric_bucket("host", None, 5_000, &sample)
            .unwrap();
        sample.cpu_host_percent = 42.0;
        repo.record_metric_bucket("host", None, 5_000, &sample)
            .unwrap();

        let conn = repo.connection();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM metric_buckets
                 WHERE scope = 'host' AND session_id = ?1 AND bucket_start_ms = 5000",
                [HOST_METRIC_SESSION_KEY],
                |row| row.get(0),
            )
            .unwrap();
        let cpu: f64 = conn
            .query_row(
                "SELECT cpu_host_percent FROM metric_buckets
                 WHERE scope = 'host' AND session_id = ?1 AND bucket_start_ms = 5000",
                [HOST_METRIC_SESSION_KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(cpu, 42.0);
    }

    #[test]
    fn migrates_v1_nullable_host_duplicates_safely() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(crate::persistence::migrations::MIGRATION_001)
                .unwrap();
            conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])
                .unwrap();
            for cpu in [1.0_f64, 9.0_f64] {
                conn.execute(
                    "INSERT INTO metric_buckets (
                        scope, session_id, bucket_start_ms, cpu_host_percent, cpu_core_percent,
                        rss_bytes, runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec
                     ) VALUES ('host', NULL, 5000, ?1, ?1, 1, 1, 1, 0, 0)",
                    [cpu],
                )
                .unwrap();
            }
        }

        let repo = SqliteRepository::open(&path).unwrap();
        let conn = repo.connection();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM metric_buckets
                 WHERE scope = 'host' AND session_id = ?1 AND bucket_start_ms = 5000",
                [HOST_METRIC_SESSION_KEY],
                |row| row.get(0),
            )
            .unwrap();
        let cpu: f64 = conn
            .query_row(
                "SELECT cpu_host_percent FROM metric_buckets
                 WHERE session_id = ?1",
                [HOST_METRIC_SESSION_KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 2);
        assert_eq!(count, 1);
        assert_eq!(cpu, 9.0);
    }

    #[test]
    fn purge_clears_persisted_latest_metrics() {
        let repo = SqliteRepository::in_memory().unwrap();
        let mut session = sample_session("metric");
        session.latest_metric = Some(MetricSample {
            at_ms: 1,
            cpu_core_percent: 1.0,
            cpu_host_percent: 1.0,
            rss_bytes: 1,
            runtime_ms: 1,
            process_count: 1,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            quality: MetricQuality {
                attribution: AttributionQuality::Exact,
                cpu: MetricAvailability::Available,
                io: IoQuality::Unavailable,
                reason: None,
            },
        });
        repo.upsert_session(&session).unwrap();
        repo.record_metric_bucket(
            "agent",
            Some(&session.id),
            0,
            session.latest_metric.as_ref().unwrap(),
        )
        .unwrap();

        assert_eq!(repo.purge_metric_history().unwrap(), 1);
        assert!(repo.load_sessions().unwrap()[0].latest_metric.is_none());
    }

    #[test]
    fn restore_queries_prioritize_active_and_bound_sessions() {
        let repo = SqliteRepository::in_memory().unwrap();
        for index in 0..(MAX_SESSIONS + 10) {
            let mut session = sample_session(&format!("terminal-{index}"));
            session.status = SessionStatus::Completed;
            session.last_event_at_ms = index as i64 + 100;
            session.ended_at_ms = Some(session.last_event_at_ms);
            repo.upsert_session(&session).unwrap();
        }
        let mut active = sample_session("active-old");
        active.last_event_at_ms = 1;
        repo.upsert_session(&active).unwrap();

        let restored = repo.load_sessions().unwrap();
        assert_eq!(restored.len(), MAX_SESSIONS);
        assert!(restored.iter().any(|session| session.id == active.id));
        assert!(
            restored
                .iter()
                .any(|session| session.id == format!("terminal-{}", MAX_SESSIONS + 9))
        );
        assert!(!restored.iter().any(|session| session.id == "terminal-0"));
    }

    #[test]
    fn restart_event_restore_is_bounded_to_newest_rows() {
        let repo = SqliteRepository::in_memory().unwrap();
        let session = sample_session("events");
        repo.upsert_session(&session).unwrap();
        {
            let mut conn = repo.connection();
            let transaction = conn.transaction().unwrap();
            {
                let mut statement = transaction
                    .prepare(
                        "INSERT INTO session_events
                         (id, session_id, sequence, occurred_at_ms, kind, level, summary, tool_name)
                         VALUES (?1, ?2, ?3, ?4, 'Lifecycle', 'Info', 'event', NULL)",
                    )
                    .unwrap();
                for sequence in 1..=(MAX_EVENTS_PER_SESSION as u64 + 5) {
                    statement
                        .execute(params![
                            Uuid::new_v4().to_string(),
                            session.id,
                            sequence as i64,
                            sequence as i64
                        ])
                        .unwrap();
                }
            }
            transaction.commit().unwrap();
        }

        let events = repo.load_events().unwrap();
        assert_eq!(events.len(), MAX_EVENTS_PER_SESSION);
        assert_eq!(events.first().unwrap().sequence, 6);
        assert_eq!(
            events.last().unwrap().sequence,
            MAX_EVENTS_PER_SESSION as u64 + 5
        );
    }

    #[test]
    fn session_event_pages_are_newest_first_pages_with_chronological_rows() {
        let repo = SqliteRepository::in_memory().unwrap();
        let session = sample_session("page");
        repo.upsert_session(&session).unwrap();
        for sequence in 1..=250_u64 {
            repo.append_event(&SessionEvent {
                id: Uuid::new_v4(),
                session_id: session.id.clone(),
                sequence,
                occurred_at_ms: sequence as i64,
                kind: SessionEventKind::Status,
                level: EventLevel::Info,
                summary: format!("event-{sequence}"),
                tool_name: None,
            })
            .unwrap();
        }

        let first = repo
            .load_session_event_page(&session.id, None, 100)
            .unwrap();
        assert_eq!(first.events.first().unwrap().sequence, 151);
        assert_eq!(first.events.last().unwrap().sequence, 250);
        assert_eq!(first.next_before_sequence, Some(151));
        assert!(first.has_more);

        let second = repo
            .load_session_event_page(&session.id, first.next_before_sequence, 100)
            .unwrap();
        assert_eq!(second.events.first().unwrap().sequence, 51);
        assert_eq!(second.events.last().unwrap().sequence, 150);
        assert_eq!(second.next_before_sequence, Some(51));
        assert!(second.has_more);

        let third = repo
            .load_session_event_page(&session.id, second.next_before_sequence, 100)
            .unwrap();
        assert_eq!(third.events.first().unwrap().sequence, 1);
        assert_eq!(third.events.last().unwrap().sequence, 50);
        assert!(!third.has_more);
        assert_eq!(third.next_before_sequence, None);
    }

    #[test]
    fn metric_history_downsamples_each_same_source_session_independently() {
        let repo = SqliteRepository::in_memory().unwrap();
        let first = sample_session("same-source-a");
        let second = sample_session("same-source-b");
        repo.upsert_session(&first).unwrap();
        repo.upsert_session(&second).unwrap();
        {
            let mut conn = repo.connection();
            let transaction = conn.transaction().unwrap();
            {
                let mut statement = transaction
                    .prepare(
                        "INSERT INTO metric_buckets (
                            scope, session_id, bucket_start_ms, cpu_host_percent,
                            cpu_core_percent, rss_bytes, runtime_ms, process_count,
                            read_bytes_per_sec, write_bytes_per_sec
                         ) VALUES ('agent', ?1, ?2, ?3, ?3, ?4, ?2, 1, 0, 0)",
                    )
                    .unwrap();
                for session_id in [&first.id, &second.id] {
                    for index in 0..=20_000_i64 {
                        statement
                            .execute(params![session_id, index, index as f64, index + 1])
                            .unwrap();
                    }
                }
            }
            transaction.commit().unwrap();
        }

        let history = repo.load_persisted_metric_history(0, 20_000, 120).unwrap();
        assert_eq!(history.agents.len(), 2);
        for session_id in [&first.id, &second.id] {
            let series = history.agents.get(session_id).unwrap();
            assert_eq!(series.total_points, 20_001);
            assert!(series.downsampled);
            assert!(series.points.len() <= 120);
            assert_eq!(series.points.first().unwrap().at_ms, 0);
            assert_eq!(series.points.last().unwrap().at_ms, 20_000);
        }
    }
}
