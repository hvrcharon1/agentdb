use crate::error::{AgentDbError, Result};
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A full-text search result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FtsResult {
    pub id: String,
    pub collection_id: String,
    pub snippet: String,
    pub rank: f64,
    pub metadata: Option<Value>,
}

/// Manages FTS5 virtual tables per collection
pub struct FullTextStore {
    conn: Arc<Mutex<Connection>>,
}

impl FullTextStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Ensure the FTS5 virtual table exists for a collection
    pub fn ensure_fts_table(&self, collection_name: &str) -> Result<()> {
        let table = fts_table_name(collection_name);
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {table}
             USING fts5(
               vec_id,
               collection_id UNINDEXED,
               text,
               tokenize='porter ascii'
             );"
        ))?;
        Ok(())
    }

    /// Index a text document for a given collection
    pub fn index_text(
        &self,
        collection_name: &str,
        vec_id: &str,
        collection_id: &str,
        text: &str,
    ) -> Result<()> {
        self.ensure_fts_table(collection_name)?;
        let table = fts_table_name(collection_name);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!("DELETE FROM {table} WHERE vec_id = ?1"),
            params![vec_id],
        )?;
        conn.execute(
            &format!("INSERT INTO {table} (vec_id, collection_id, text) VALUES (?1, ?2, ?3)"),
            params![vec_id, collection_id, text],
        )?;
        Ok(())
    }

    /// Full-text search over a collection
    pub fn search(
        &self,
        collection_name: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<FtsResult>> {
        let table = fts_table_name(collection_name);
        let conn = self.conn.lock().unwrap();

        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                params![table],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !exists {
            return Ok(vec![]);
        }

        let sql = format!(
            "SELECT f.vec_id,
                    f.collection_id,
                    snippet({table}, 2, '<b>', '</b>', '...', 10),
                    bm25({table}) AS rank,
                    v.metadata
             FROM {table} f
             LEFT JOIN _adb_vectors v
               ON v.id = f.vec_id AND v.collection_id = f.collection_id
             WHERE {table} MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![query, top_k as i64], |row| {
            let meta_str: Option<String> = row.get(4)?;
            Ok(FtsResult {
                id: row.get(0)?,
                collection_id: row.get(1)?,
                snippet: row.get(2)?,
                rank: row.get(3)?,
                metadata: meta_str
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
            })
        })?;

        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Delete a document from the FTS index
    pub fn delete_text(&self, collection_name: &str, vec_id: &str) -> Result<()> {
        let table = fts_table_name(collection_name);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            &format!("DELETE FROM {table} WHERE vec_id = ?1"),
            params![vec_id],
        )?;
        Ok(())
    }

    /// Merge FTS index segments for faster queries
    pub fn optimize(&self, collection_name: &str) -> Result<()> {
        let table = fts_table_name(collection_name);
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(&format!("INSERT INTO {table}({table}) VALUES('optimize');"))?;
        Ok(())
    }
}

fn fts_table_name(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("_adb_fts_{safe}")
}
