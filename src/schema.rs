use crate::error::Result;
use rusqlite::Connection;

pub const SCHEMA_VERSION: &str = "5";

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
            updated_at    INTEGER NOT NULL DEFAULT 0,
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
            error      TEXT,
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

        -- Message full-text search (schema v4)
        CREATE VIRTUAL TABLE IF NOT EXISTS _adb_messages_fts
        USING fts5(
            message_id UNINDEXED,
            conversation_id UNINDEXED,
            content,
            tokenize='porter ascii'
        );

        -- Tool registry (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_tools (
            id                TEXT PRIMARY KEY,
            name              TEXT UNIQUE NOT NULL,
            description       TEXT,
            parameters_schema TEXT,
            version           TEXT NOT NULL DEFAULT '1.0.0',
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tools_name ON _adb_tools(name);

        -- Structured tool call log (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_tool_calls (
            id         TEXT PRIMARY KEY,
            session_id TEXT,
            tool_name  TEXT NOT NULL,
            arguments  TEXT,
            result     TEXT,
            error      TEXT,
            latency_ms INTEGER,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tool_calls_session   ON _adb_tool_calls(session_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_tool_calls_tool_name ON _adb_tool_calls(tool_name, created_at);

        -- Immutable audit log (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_audit_log (
            id         TEXT PRIMARY KEY,
            timestamp  INTEGER NOT NULL,
            actor      TEXT,
            action     TEXT NOT NULL,
            table_name TEXT NOT NULL,
            record_id  TEXT NOT NULL,
            old_value  TEXT,
            new_value  TEXT,
            reason     TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_audit_timestamp  ON _adb_audit_log(timestamp);
        CREATE INDEX IF NOT EXISTS idx_audit_table_rec  ON _adb_audit_log(table_name, record_id);
        CREATE INDEX IF NOT EXISTS idx_audit_actor      ON _adb_audit_log(actor, timestamp);

        -- Token-budgeted context window entries (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_context_entries (
            id              TEXT PRIMARY KEY,
            session_id      TEXT NOT NULL,
            source_type     TEXT NOT NULL,
            source_id       TEXT NOT NULL,
            content_preview TEXT,
            token_count     INTEGER NOT NULL DEFAULT 0,
            relevance_score REAL NOT NULL DEFAULT 0.0,
            priority        INTEGER NOT NULL DEFAULT 0,
            included_at     INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ctx_session ON _adb_context_entries(session_id, priority DESC, relevance_score DESC);
        CREATE INDEX IF NOT EXISTS idx_ctx_source  ON _adb_context_entries(source_type, source_id);

        -- Versioned prompt templates (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_prompt_templates (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            version    INTEGER NOT NULL DEFAULT 1,
            template   TEXT NOT NULL,
            model_hint TEXT,
            max_tokens INTEGER,
            metadata   TEXT,
            created_at INTEGER NOT NULL,
            UNIQUE (name, version)
        );
        CREATE INDEX IF NOT EXISTS idx_prompt_name_ver ON _adb_prompt_templates(name, version DESC);

        -- Privacy / data classification labels (schema v5)
        CREATE TABLE IF NOT EXISTS _adb_data_labels (
            table_name TEXT NOT NULL,
            record_id  TEXT NOT NULL,
            label      TEXT NOT NULL,
            tagged_by  TEXT,
            tagged_at  INTEGER NOT NULL,
            PRIMARY KEY (table_name, record_id, label)
        );
        CREATE INDEX IF NOT EXISTS idx_data_labels_table ON _adb_data_labels(table_name, label);
        CREATE INDEX IF NOT EXISTS idx_data_labels_label ON _adb_data_labels(label);
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
        // Older or newer schema on disk: caller must run `agentdb migrate`
        // (or call `schema::migrate(conn)` programmatically).
        Some(_) => Err(crate::error::AgentDbError::SchemaMigration),
        // Missing version key indicates a corrupt or pre-v0.1 database.
        None => Err(crate::error::AgentDbError::SchemaMigration),
    }
}

/// Idempotent migration runner.
///
/// Re-runs `bootstrap()` (all DDL uses `CREATE … IF NOT EXISTS` so existing
/// tables/columns are left intact), then stamps the current schema version.
/// Safe to call on databases created by any prior version of AgentDB.
///
/// In addition to adding new tables, this applies additive `ALTER TABLE …
/// ADD COLUMN` statements for columns introduced in later schema versions.
/// SQLite ignores duplicate columns via `IF NOT EXISTS` semantics.
pub fn migrate(conn: &Connection) -> Result<()> {
    // Re-run bootstrap to create any tables introduced after the DB was first opened.
    bootstrap(conn)?;

    // v2 → v3: add `error` column to _adb_workflows (was missing before v0.5.0).
    let _ = conn.execute_batch("ALTER TABLE _adb_workflows ADD COLUMN error TEXT;");

    // v2 → v3: add `updated_at` column to _adb_vectors.
    let _ = conn.execute_batch(
        "ALTER TABLE _adb_vectors ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;",
    );

    // v3 → v4: message FTS virtual table (bootstrap already uses IF NOT EXISTS).
    // No ALTER TABLE needed — new table is created by bootstrap() above.

    // v4 → v5: add embedding model provenance to vectors.
    let _ = conn.execute_batch("ALTER TABLE _adb_vectors ADD COLUMN model TEXT;");

    // v4 → v5: add token count to messages for context budgeting.
    let _ = conn.execute_batch("ALTER TABLE _adb_messages ADD COLUMN token_count INTEGER;");

    // Stamp the new version.
    conn.execute(
        "INSERT INTO _adb_meta (key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![SCHEMA_VERSION],
    )?;
    Ok(())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
