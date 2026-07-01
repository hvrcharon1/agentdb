use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parameters_schema: Option<Value>,
    pub version: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub session_id: Option<String>,
    pub tool_name: String,
    pub arguments: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub latency_ms: Option<i64>,
    pub created_at: i64,
}

pub struct ToolStore {
    conn: Arc<Mutex<Connection>>,
}

impl ToolStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn register_tool(
        &self,
        name: &str,
        description: Option<&str>,
        parameters_schema: Option<Value>,
        version: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let schema_str = parameters_schema.as_ref().map(|v| v.to_string());
        let ver = version.unwrap_or("1.0.0");
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_tools (id, name, description, parameters_schema, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(name) DO UPDATE SET
                 description = excluded.description,
                 parameters_schema = excluded.parameters_schema,
                 version = excluded.version,
                 updated_at = excluded.updated_at",
            params![id, name, description, schema_str, ver, now, now],
        )?;
        Ok(id)
    }

    pub fn get_tool(&self, name: &str) -> Result<Tool> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, description, parameters_schema, version, created_at, updated_at
             FROM _adb_tools WHERE name = ?1",
            params![name],
            |row| {
                let schema_str: Option<String> = row.get(3)?;
                Ok(Tool {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    parameters_schema: schema_str.and_then(|s| serde_json::from_str(&s).ok()),
                    version: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|_| AgentDbError::InvalidArgument(format!("tool not found: {name}")))
    }

    pub fn list_tools(&self) -> Result<Vec<Tool>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, parameters_schema, version, created_at, updated_at
             FROM _adb_tools ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            let schema_str: Option<String> = row.get(3)?;
            Ok(Tool {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                parameters_schema: schema_str.and_then(|s| serde_json::from_str(&s).ok()),
                version: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn delete_tool(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM _adb_tools WHERE name = ?1", params![name])?;
        Ok(())
    }

    pub fn log_tool_call(
        &self,
        session_id: Option<&str>,
        tool_name: &str,
        arguments: Option<Value>,
        result: Option<Value>,
        error: Option<&str>,
        latency_ms: Option<i64>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let args_str = arguments.as_ref().map(|v| v.to_string());
        let result_str = result.as_ref().map(|v| v.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_tool_calls (id, session_id, tool_name, arguments, result, error, latency_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, session_id, tool_name, args_str, result_str, error, latency_ms, now],
        )?;
        Ok(id)
    }

    pub fn get_tool_calls(
        &self,
        session_id: Option<&str>,
        tool_name: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<ToolCall>> {
        let conn = self.conn.lock().unwrap();
        let sql = match (session_id, tool_name) {
            (Some(_), Some(_)) => {
                "SELECT id, session_id, tool_name, arguments, result, error, latency_ms, created_at
                 FROM _adb_tool_calls WHERE session_id = ?1 AND tool_name = ?2
                 ORDER BY created_at DESC LIMIT ?3"
            }
            (Some(_), None) => {
                "SELECT id, session_id, tool_name, arguments, result, error, latency_ms, created_at
                 FROM _adb_tool_calls WHERE session_id = ?1
                 ORDER BY created_at DESC LIMIT ?3"
            }
            (None, Some(_)) => {
                "SELECT id, session_id, tool_name, arguments, result, error, latency_ms, created_at
                 FROM _adb_tool_calls WHERE tool_name = ?2
                 ORDER BY created_at DESC LIMIT ?3"
            }
            (None, None) => {
                "SELECT id, session_id, tool_name, arguments, result, error, latency_ms, created_at
                 FROM _adb_tool_calls
                 ORDER BY created_at DESC LIMIT ?3"
            }
        };
        let lim = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(
            params![session_id.unwrap_or(""), tool_name.unwrap_or(""), lim],
            parse_tool_call_row,
        )?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }
}

fn parse_tool_call_row(row: &rusqlite::Row) -> rusqlite::Result<ToolCall> {
    let args_str: Option<String> = row.get(3)?;
    let result_str: Option<String> = row.get(4)?;
    Ok(ToolCall {
        id: row.get(0)?,
        session_id: row.get(1)?,
        tool_name: row.get(2)?,
        arguments: args_str.and_then(|s| serde_json::from_str(&s).ok()),
        result: result_str.and_then(|s| serde_json::from_str(&s).ok()),
        error: row.get(5)?,
        latency_ms: row.get(6)?,
        created_at: row.get(7)?,
    })
}
