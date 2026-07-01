use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextEntry {
    pub id: String,
    pub session_id: String,
    pub source_type: String,
    pub source_id: String,
    pub content_preview: Option<String>,
    pub token_count: i64,
    pub relevance_score: f64,
    pub priority: i64,
    pub included_at: i64,
}

pub struct ContextStore {
    conn: Arc<Mutex<Connection>>,
}

impl ContextStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_entry(
        &self,
        session_id: &str,
        source_type: &str,
        source_id: &str,
        content_preview: Option<&str>,
        token_count: i64,
        relevance_score: f64,
        priority: i64,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_context_entries
                 (id, session_id, source_type, source_id, content_preview, token_count, relevance_score, priority, included_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, session_id, source_type, source_id, content_preview, token_count, relevance_score, priority, now],
        )?;
        Ok(id)
    }

    /// Build a context window for a session, filling up to `max_tokens`.
    ///
    /// Returns entries ordered by priority (desc), then relevance (desc),
    /// stopping when the running token sum would exceed `max_tokens`.
    pub fn build_window(&self, session_id: &str, max_tokens: i64) -> Result<Vec<ContextEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source_type, source_id, content_preview, token_count, relevance_score, priority, included_at
             FROM _adb_context_entries
             WHERE session_id = ?1
             ORDER BY priority DESC, relevance_score DESC",
        )?;
        let rows = stmt.query_map(params![session_id], parse_context_row)?;
        let mut result = Vec::new();
        let mut running_tokens: i64 = 0;
        for row in rows {
            let entry = row.map_err(AgentDbError::Sqlite)?;
            if running_tokens + entry.token_count > max_tokens {
                continue;
            }
            running_tokens += entry.token_count;
            result.push(entry);
        }
        Ok(result)
    }

    pub fn get_entries(&self, session_id: &str) -> Result<Vec<ContextEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, source_type, source_id, content_preview, token_count, relevance_score, priority, included_at
             FROM _adb_context_entries
             WHERE session_id = ?1
             ORDER BY priority DESC, relevance_score DESC",
        )?;
        let rows = stmt.query_map(params![session_id], parse_context_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn clear_session(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_context_entries WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn remove_entry(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_context_entries WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}

fn parse_context_row(row: &rusqlite::Row) -> rusqlite::Result<ContextEntry> {
    Ok(ContextEntry {
        id: row.get(0)?,
        session_id: row.get(1)?,
        source_type: row.get(2)?,
        source_id: row.get(3)?,
        content_preview: row.get(4)?,
        token_count: row.get(5)?,
        relevance_score: row.get(6)?,
        priority: row.get(7)?,
        included_at: row.get(8)?,
    })
}
