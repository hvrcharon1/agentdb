use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A single reasoning trace entry.
#[derive(Debug, Clone)]
pub struct Trace {
    /// Unique identifier for this trace entry.
    pub id: String,
    /// Optional session identifier that groups related traces.
    pub session_id: Option<String>,
    /// ID of the parent trace (for tree structures), if any.
    pub parent_id: Option<String>,
    /// Semantic type of the trace (e.g. `"thought"`, `"tool_call"`, `"observation"`).
    pub trace_type: String,
    /// Text content of the trace entry.
    pub content: String,
    /// Arbitrary JSON payload attached to this trace.
    pub metadata: Option<Value>,
    /// Unix-millisecond timestamp when the trace was recorded.
    pub created_at: i64,
}

/// Stores and retrieves reasoning traces.
pub struct TraceStore {
    conn: Arc<Mutex<Connection>>,
}

impl TraceStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Record a new trace entry. Returns the generated trace ID.
    ///
    /// - `session_id`: optional grouping key (e.g. a request or agent run ID).
    /// - `parent_id`: optional ID of a parent trace for tree structures.
    /// - `trace_type`: semantic label (e.g. `"thought"`, `"tool_call"`).
    /// - `content`: the text body of the trace.
    /// - `metadata`: optional JSON payload.
    pub fn add_trace(
        &self,
        session_id: Option<&str>,
        parent_id: Option<&str>,
        trace_type: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String> {
        let trace_id = Uuid::new_v4().to_string();
        let meta_str = metadata.as_ref().map(|m| m.to_string());
        let now = now_ms();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_traces
                 (id, session_id, parent_id, trace_type, content, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![trace_id, session_id, parent_id, trace_type, content, meta_str, now],
        )?;
        Ok(trace_id)
    }

    /// Return traces for a given session in chronological order.
    ///
    /// Use `limit` and `offset` for pagination. Pass `None` for both to
    /// retrieve all traces.
    pub fn get_traces(
        &self,
        session_id: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Trace>> {
        let conn = self.conn.lock().unwrap();
        let lim: i64 = limit.map(|n| n as i64).unwrap_or(i64::MAX);
        let off: i64 = offset.map(|n| n as i64).unwrap_or(0);
        let mut stmt = conn.prepare(
            "SELECT id, session_id, parent_id, trace_type, content, metadata, created_at
             FROM _adb_traces
             WHERE session_id = ?1
             ORDER BY created_at ASC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![session_id, lim, off], parse_trace)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Return a subtree of traces rooted at `root_id`.
    ///
    /// Uses a recursive CTE to follow `parent_id` links. The root trace itself
    /// is included. Results are ordered by `created_at` ascending.
    pub fn get_trace_tree(&self, root_id: &str) -> Result<Vec<Trace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "WITH RECURSIVE tree(id) AS (
                 SELECT id FROM _adb_traces WHERE id = ?1
                 UNION ALL
                 SELECT t.id
                 FROM _adb_traces t
                 JOIN tree ON t.parent_id = tree.id
             )
             SELECT t.id, t.session_id, t.parent_id, t.trace_type,
                    t.content, t.metadata, t.created_at
             FROM _adb_traces t
             JOIN tree ON t.id = tree.id
             ORDER BY t.created_at ASC",
        )?;
        let rows = stmt.query_map(params![root_id], parse_trace)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }
}

// ── Row parsers ──────────────────────────────────────────────────────────────

fn parse_trace(row: &rusqlite::Row) -> rusqlite::Result<Trace> {
    let meta_str: Option<String> = row.get(5)?;
    Ok(Trace {
        id: row.get(0)?,
        session_id: row.get(1)?,
        parent_id: row.get(2)?,
        trace_type: row.get(3)?,
        content: row.get(4)?,
        metadata: meta_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.get(6)?,
    })
}
