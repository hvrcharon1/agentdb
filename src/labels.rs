use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataLabel {
    pub table_name: String,
    pub record_id: String,
    pub label: String,
    pub tagged_by: Option<String>,
    pub tagged_at: i64,
}

pub struct LabelStore {
    conn: Arc<Mutex<Connection>>,
}

impl LabelStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn tag(
        &self,
        table_name: &str,
        record_id: &str,
        label: &str,
        tagged_by: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = now_ms();
        conn.execute(
            "INSERT OR REPLACE INTO _adb_data_labels (table_name, record_id, label, tagged_by, tagged_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![table_name, record_id, label, tagged_by, now],
        )?;
        Ok(())
    }

    pub fn untag(&self, table_name: &str, record_id: &str, label: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_data_labels WHERE table_name = ?1 AND record_id = ?2 AND label = ?3",
            params![table_name, record_id, label],
        )?;
        Ok(())
    }

    pub fn get_labels(&self, table_name: &str, record_id: &str) -> Result<Vec<DataLabel>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT table_name, record_id, label, tagged_by, tagged_at
             FROM _adb_data_labels
             WHERE table_name = ?1 AND record_id = ?2
             ORDER BY tagged_at",
        )?;
        let rows = stmt.query_map(params![table_name, record_id], parse_label_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn find_by_label(&self, label: &str, limit: Option<usize>) -> Result<Vec<DataLabel>> {
        let conn = self.conn.lock().unwrap();
        let lim = limit.unwrap_or(100) as i64;
        let mut stmt = conn.prepare(
            "SELECT table_name, record_id, label, tagged_by, tagged_at
             FROM _adb_data_labels
             WHERE label = ?1
             ORDER BY tagged_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![label, lim], parse_label_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn has_label(&self, table_name: &str, record_id: &str, label: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM _adb_data_labels
                 WHERE table_name = ?1 AND record_id = ?2 AND label = ?3",
                params![table_name, record_id, label],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(count > 0)
    }

    pub fn clear_record(&self, table_name: &str, record_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_data_labels WHERE table_name = ?1 AND record_id = ?2",
            params![table_name, record_id],
        )?;
        Ok(())
    }
}

fn parse_label_row(row: &rusqlite::Row) -> rusqlite::Result<DataLabel> {
    Ok(DataLabel {
        table_name: row.get(0)?,
        record_id: row.get(1)?,
        label: row.get(2)?,
        tagged_by: row.get(3)?,
        tagged_at: row.get(4)?,
    })
}
