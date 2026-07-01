use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub template: String,
    pub model_hint: Option<String>,
    pub max_tokens: Option<i64>,
    pub metadata: Option<Value>,
    pub created_at: i64,
}

pub struct PromptStore {
    conn: Arc<Mutex<Connection>>,
}

impl PromptStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn create_template(
        &self,
        name: &str,
        template: &str,
        model_hint: Option<&str>,
        max_tokens: Option<i64>,
        metadata: Option<Value>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let meta_str = metadata.as_ref().map(|v| v.to_string());
        let now = now_ms();
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version) + 1, 1) FROM _adb_prompt_templates WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .unwrap_or(1);
        conn.execute(
            "INSERT INTO _adb_prompt_templates (id, name, version, template, model_hint, max_tokens, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, name, version, template, model_hint, max_tokens, meta_str, now],
        )?;
        Ok(id)
    }

    /// Get the latest version of a named template.
    pub fn get_template(&self, name: &str) -> Result<PromptTemplate> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, version, template, model_hint, max_tokens, metadata, created_at
             FROM _adb_prompt_templates
             WHERE name = ?1
             ORDER BY version DESC LIMIT 1",
            params![name],
            parse_template_row,
        )
        .map_err(|_| AgentDbError::InvalidArgument(format!("template not found: {name}")))
    }

    /// Get a specific version of a template.
    pub fn get_template_version(&self, name: &str, version: i64) -> Result<PromptTemplate> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, name, version, template, model_hint, max_tokens, metadata, created_at
             FROM _adb_prompt_templates
             WHERE name = ?1 AND version = ?2",
            params![name, version],
            parse_template_row,
        )
        .map_err(|_| {
            AgentDbError::InvalidArgument(format!("template not found: {name} v{version}"))
        })
    }

    pub fn list_templates(&self) -> Result<Vec<PromptTemplate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, version, template, model_hint, max_tokens, metadata, created_at
             FROM _adb_prompt_templates
             ORDER BY name, version DESC",
        )?;
        let rows = stmt.query_map([], parse_template_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Render a template by substituting `{{key}}` placeholders with values from `vars`.
    pub fn render(
        &self,
        name: &str,
        vars: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        let tmpl = self.get_template(name)?;
        let mut output = tmpl.template;
        for (key, value) in vars {
            output = output.replace(&format!("{{{{{key}}}}}"), value);
        }
        Ok(output)
    }

    pub fn delete_template(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_prompt_templates WHERE name = ?1",
            params![name],
        )?;
        Ok(())
    }
}

fn parse_template_row(row: &rusqlite::Row) -> rusqlite::Result<PromptTemplate> {
    let meta_str: Option<String> = row.get(6)?;
    Ok(PromptTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        version: row.get(2)?,
        template: row.get(3)?,
        model_hint: row.get(4)?,
        max_tokens: row.get(5)?,
        metadata: meta_str.and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get(7)?,
    })
}
