pub const MIGRATION_001: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    external_session_id TEXT NOT NULL,
    label TEXT NOT NULL,
    workspace_label TEXT,
    status TEXT NOT NULL,
    attention TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    last_event_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    process_root_pid INTEGER,
    process_root_started_at_ms INTEGER,
    latest_metric_json TEXT,
    UNIQUE(source, external_session_id)
);

CREATE TABLE IF NOT EXISTS session_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    occurred_at_ms INTEGER NOT NULL,
    kind TEXT NOT NULL,
    level TEXT NOT NULL,
    summary TEXT NOT NULL,
    tool_name TEXT,
    UNIQUE(session_id, sequence)
);

CREATE INDEX IF NOT EXISTS idx_session_events_session_seq
    ON session_events(session_id, sequence);

CREATE INDEX IF NOT EXISTS idx_session_events_occurred
    ON session_events(occurred_at_ms);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS integration_health (
    source TEXT PRIMARY KEY,
    capabilities_json TEXT NOT NULL,
    healthy INTEGER NOT NULL,
    message TEXT,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS metric_buckets (
    scope TEXT NOT NULL,
    session_id TEXT,
    bucket_start_ms INTEGER NOT NULL,
    cpu_host_percent REAL NOT NULL,
    cpu_core_percent REAL NOT NULL,
    rss_bytes INTEGER NOT NULL,
    runtime_ms INTEGER NOT NULL,
    process_count INTEGER NOT NULL,
    read_bytes_per_sec INTEGER NOT NULL,
    write_bytes_per_sec INTEGER NOT NULL,
    PRIMARY KEY (scope, session_id, bucket_start_ms)
);

CREATE INDEX IF NOT EXISTS idx_metric_buckets_start
    ON metric_buckets(bucket_start_ms);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key TEXT PRIMARY KEY,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub const MIGRATION_002: &str = r#"
BEGIN IMMEDIATE;

CREATE TABLE metric_buckets_v2 (
    scope TEXT NOT NULL,
    session_id TEXT NOT NULL,
    bucket_start_ms INTEGER NOT NULL,
    cpu_host_percent REAL NOT NULL,
    cpu_core_percent REAL NOT NULL,
    rss_bytes INTEGER NOT NULL,
    runtime_ms INTEGER NOT NULL,
    process_count INTEGER NOT NULL,
    read_bytes_per_sec INTEGER NOT NULL,
    write_bytes_per_sec INTEGER NOT NULL,
    PRIMARY KEY (scope, session_id, bucket_start_ms)
);

INSERT INTO metric_buckets_v2 (
    scope, session_id, bucket_start_ms, cpu_host_percent, cpu_core_percent,
    rss_bytes, runtime_ms, process_count, read_bytes_per_sec, write_bytes_per_sec
)
SELECT
    old.scope,
    COALESCE(old.session_id, '__host__'),
    old.bucket_start_ms,
    old.cpu_host_percent,
    old.cpu_core_percent,
    old.rss_bytes,
    old.runtime_ms,
    old.process_count,
    old.read_bytes_per_sec,
    old.write_bytes_per_sec
FROM metric_buckets old
JOIN (
    SELECT
        scope,
        COALESCE(session_id, '__host__') AS canonical_session_id,
        bucket_start_ms,
        MAX(rowid) AS newest_rowid
    FROM metric_buckets
    GROUP BY scope, COALESCE(session_id, '__host__'), bucket_start_ms
) newest ON old.rowid = newest.newest_rowid;

DROP TABLE metric_buckets;
ALTER TABLE metric_buckets_v2 RENAME TO metric_buckets;
CREATE INDEX idx_metric_buckets_start ON metric_buckets(bucket_start_ms);

UPDATE schema_version SET version = 2;
COMMIT;
"#;

pub const CURRENT_SCHEMA_VERSION: i32 = 2;

pub fn apply_migrations(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    let version: Option<i32> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .ok();

    let mut version = version;
    if version.is_none() {
        conn.execute_batch(MIGRATION_001)?;
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
        version = Some(1);
    }

    if version.unwrap_or_default() < CURRENT_SCHEMA_VERSION {
        conn.execute_batch(MIGRATION_002)?;
    }

    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(())
}
