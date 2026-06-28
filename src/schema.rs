use crate::error::Result;
use rusqlite::Connection;

pub const SCHEMA_VERSION: &str = "2";

pub fn bootstrap(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        PRAGMA synchronous=NORMAL;

        CREATE TABLE IF NOT EXISTS _adb_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS _adb_collections (
            id         TEXT PRIMARY KEY,
            name       TEXT UNIQUE NOT NULL,
            dim        INTEGER NOT NULL,
            metric     TEXT NOT NULL DEFAULT 'cosine',
            count      INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS _adb_vectors (
            id            TEXT NOT NULL,
            collection_id TEXT NOT NULL,
            vector        BLOB NOT NULL,
            metadata      TEXT,
            created_at    INTEGER NOT NULL,
            PRIMARY KEY (id, collection_id),
            FOREIGN KEY (collection_id)
                REFERENCES _adb_collections(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS _adb_hnsw_index (
            collection_id TEXT PRIMARY KEY,
            index_blob    BLOB NOT NULL,
            built_at      INTEGER NOT NULL,
            is_dirty      INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS _adb_nodes (
            id         TEXT PRIMARY KEY,
            kind       TEXT NOT NULL,
            data       TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS _adb_edges (
            src        TEXT NOT NULL,
            dst        TEXT NOT NULL,
            relation   TEXT NOT NULL,
            weight     REAL NOT NULL DEFAULT 1.0,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (src, dst, relation),
            FOREIGN KEY (src) REFERENCES _adb_nodes(id) ON DELETE CASCADE,
            FOREIGN KEY (dst) REFERENCES _adb_nodes(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_edges_src  ON _adb_edges(src);
        CREATE INDEX IF NOT EXISTS idx_edges_dst  ON _adb_edges(dst);
        CREATE INDEX IF NOT EXISTS idx_vectors_col ON _adb_vectors(collection_id);
        CREATE INDEX IF NOT EXISTS idx_nodes_kind  ON _adb_nodes(kind);

        -- Conversations / message threading
        CREATE TABLE IF NOT EXISTS _adb_conversations (
            id         TEXT PRIMARY KEY,
            title      TEXT,
            metadata   TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        CREATE TABLE IF NOT EXISTS _adb_messages (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL REFERENCES _adb_conversations(id) ON DELETE CASCADE,
            role            TEXT NOT NULL,
            content         TEXT NOT NULL,
            metadata        TEXT,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_messages_conv ON _adb_messages(conversation_id, created_at);

        -- Workflow persistence
        CREATE TABLE IF NOT EXISTS _adb_workflows (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            status     TEXT NOT NULL DEFAULT 'pending',
            input      TEXT,
            output     TEXT,
            metadata   TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );

        CREATE TABLE IF NOT EXISTS _adb_workflow_steps (
            id           TEXT PRIMARY KEY,
            workflow_id  TEXT NOT NULL REFERENCES _adb_workflows(id) ON DELETE CASCADE,
            step_index   INTEGER NOT NULL,
            name         TEXT NOT NULL,
            status       TEXT NOT NULL DEFAULT 'pending',
            input        TEXT,
            output       TEXT,
            error        TEXT,
            started_at   INTEGER,
            completed_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_wf_steps ON _adb_workflow_steps(workflow_id, step_index);

        -- Reasoning traces
        CREATE TABLE IF NOT EXISTS _adb_traces (
            id         TEXT PRIMARY KEY,
            session_id TEXT,
            parent_id  TEXT REFERENCES _adb_traces(id),
            trace_type TEXT NOT NULL,
            content    TEXT NOT NULL,
            metadata   TEXT,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_traces_session ON _adb_traces(session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_traces_parent  ON _adb_traces(parent_id);
        ",
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO _adb_meta (key, value) VALUES ('schema_version', ?1)",
        rusqlite::params![SCHEMA_VERSION],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO _adb_meta (key, value) VALUES ('created_at', ?1)",
        rusqlite::params![now_ms()],
    )?;
    Ok(())
}

pub fn check_version(conn: &Connection) -> Result<()> {
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM _adb_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    match version.as_deref() {
        Some(v) if v == SCHEMA_VERSION => Ok(()),
        Some(_) => Err(crate::error::AgentDbError::SchemaMigration),
        None => Ok(()),
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
