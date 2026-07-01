use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: i64,
    pub actor: Option<String>,
    pub action: String,
    pub table_name: String,
    pub record_id: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub reason: Option<String>,
}

pub struct AuditStore {
    conn: Arc<Mutex<Connection>>,
}

impl AuditStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log(
        &self,
        actor: Option<&str>,
        action: &str,
        table_name: &str,
        record_id: &str,
        old_value: Option<Value>,
        new_value: Option<Value>,
        reason: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let old_str = old_value.as_ref().map(|v| v.to_string());
        let new_str = new_value.as_ref().map(|v| v.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_audit_log (id, timestamp, actor, action, table_name, record_id, old_value, new_value, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, now, actor, action, table_name, record_id, old_str, new_str, reason],
        )?;
        Ok(id)
    }

    pub fn query_by_record(
        &self,
        table_name: &str,
        record_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let conn = self.conn.lock().unwrap();
        let lim = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, actor, action, table_name, record_id, old_value, new_value, reason
             FROM _adb_audit_log
             WHERE table_name = ?1 AND record_id = ?2
             ORDER BY timestamp DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![table_name, record_id, lim], parse_audit_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn query_by_actor(&self, actor: &str, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let conn = self.conn.lock().unwrap();
        let lim = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, actor, action, table_name, record_id, old_value, new_value, reason
             FROM _adb_audit_log
             WHERE actor = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![actor, lim], parse_audit_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn query_recent(&self, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let conn = self.conn.lock().unwrap();
        let lim = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, actor, action, table_name, record_id, old_value, new_value, reason
             FROM _adb_audit_log
             ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![lim], parse_audit_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }
}

fn parse_audit_row(row: &rusqlite::Row) -> rusqlite::Result<AuditEntry> {
    let old_str: Option<String> = row.get(6)?;
    let new_str: Option<String> = row.get(7)?;
    Ok(AuditEntry {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        actor: row.get(2)?,
        action: row.get(3)?,
        table_name: row.get(4)?,
        record_id: row.get(5)?,
        old_value: old_str.and_then(|s| serde_json::from_str(&s).ok()),
        new_value: new_str.and_then(|s| serde_json::from_str(&s).ok()),
        reason: row.get(8)?,
    })
}
