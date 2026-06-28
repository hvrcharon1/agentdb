use crate::conversations::ConversationStore;
use crate::error::Result;
use crate::fts::FullTextStore;
use crate::hybrid::{HybridQuery, HybridResult, HybridStore};
use crate::memory::MemoryGraph;
use crate::schema;
use crate::traces::TraceStore;
use crate::vectors::VectorStore;
use crate::workflows::WorkflowStore;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

/// The main AgentDB connection — your single-file AI database.
pub struct AgentDB {
    conn: Arc<Mutex<Connection>>,
}

impl AgentDB {
    /// Open or create an AgentDB database. Use `":memory:"` for tests.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::bootstrap(&conn)?;
        schema::check_version(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Access the vector store layer
    pub fn vectors(&self) -> VectorStore {
        VectorStore::new(Arc::clone(&self.conn))
    }

    /// Access the memory graph layer
    pub fn memory(&self) -> MemoryGraph {
        MemoryGraph::new(Arc::clone(&self.conn))
    }

    /// Access the full-text search layer
    pub fn fts(&self) -> FullTextStore {
        FullTextStore::new(Arc::clone(&self.conn))
    }

    /// Access the conversation / message-threading layer
    pub fn conversations(&self) -> ConversationStore {
        ConversationStore::new(Arc::clone(&self.conn))
    }

    /// Access the workflow persistence layer
    pub fn workflows(&self) -> WorkflowStore {
        WorkflowStore::new(Arc::clone(&self.conn))
    }

    /// Access the reasoning-trace layer
    pub fn traces(&self) -> TraceStore {
        TraceStore::new(Arc::clone(&self.conn))
    }

    /// Run a hybrid graph + vector query
    pub fn hybrid_query(&self, q: HybridQuery) -> Result<Vec<HybridResult>> {
        let dim: usize = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT dim FROM _adb_collections WHERE name = ?1",
                rusqlite::params![q.collection],
                |r| r.get::<_, i64>(0).map(|v| v as usize),
            )
            .unwrap_or(q.embedding.len())
        };
        let col = self.vectors().collection(q.collection, dim)?;
        let store = HybridStore::new(Arc::clone(&self.conn));
        store.query(q, &col)
    }

    /// Execute a raw SQL statement
    pub fn execute(&self, sql: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute(sql, [])?)
    }

    /// Execute a parameterized SQL statement
    pub fn execute_params(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute(sql, params)?)
    }

    /// Run multiple operations atomically inside a single SQLite transaction.
    ///
    /// The closure receives a [`rusqlite::Transaction`] and may perform any
    /// number of reads or writes.  If the closure returns `Ok`, the transaction
    /// is committed; if it returns `Err` (or panics), the transaction is rolled
    /// back automatically.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use agentdb::AgentDB;
    /// let db = AgentDB::open(":memory:").unwrap();
    /// db.transaction(|tx| {
    ///     tx.execute("INSERT INTO _adb_nodes (id, label, data) VALUES ('x','tag','{}')", [])?;
    ///     tx.execute("INSERT INTO _adb_nodes (id, label, data) VALUES ('y','tag','{}')", [])?;
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<T>,
    {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// Execute one or more semicolon-separated SQL statements as a single
    /// atomic batch.  This is a convenience wrapper around
    /// [`execute_batch`](rusqlite::Connection::execute_batch) that wraps the
    /// statements in an explicit transaction so partial execution is never
    /// visible to other threads.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use agentdb::AgentDB;
    /// let db = AgentDB::open(":memory:").unwrap();
    /// db.execute_batch(
    ///     "INSERT INTO _adb_nodes (id,label,data) VALUES ('a','t','{}');
    ///      INSERT INTO _adb_nodes (id,label,data) VALUES ('b','t','{}');"
    /// ).unwrap();
    /// ```
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.commit()?;
        Ok(())
    }

    /// Query and return rows as JSON values
    pub fn query_json(&self, sql: &str) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)?;
        let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let rows = stmt.query_map([], |row| {
            let mut map = serde_json::Map::new();
            for (i, name) in col_names.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i)?;
                map.insert(name.clone(), rusqlite_value_to_json(val));
            }
            Ok(serde_json::Value::Object(map))
        })?;
        rows.map(|r| r.map_err(crate::error::AgentDbError::Sqlite))
            .collect()
    }

    /// Flush dirty HNSW indexes and close gracefully
    pub fn close(self) -> Result<()> {
        let collections = self.vectors().list_collections()?;
        for (name, dim, _) in collections {
            let col = self.vectors().collection(&name, dim)?;
            let is_dirty: i64 = {
                let conn = self.conn.lock().unwrap();
                conn.query_row(
                    "SELECT COALESCE(
                        (SELECT is_dirty FROM _adb_hnsw_index
                         WHERE collection_id =
                           (SELECT id FROM _adb_collections WHERE name = ?1)
                        ), 0)",
                    rusqlite::params![name],
                    |r| r.get(0),
                )
                .unwrap_or(0)
            };
            if is_dirty == 1 {
                col.reindex()?;
            }
        }
        Ok(())
    }

    /// Return database-wide statistics
    pub fn stats(&self) -> Result<DbStats> {
        let conn = self.conn.lock().unwrap();
        let collections: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_collections", [], |r| r.get(0))?;
        let vectors: i64 = conn.query_row(
            "SELECT COALESCE(SUM(count), 0) FROM _adb_collections",
            [],
            |r| r.get(0),
        )?;
        let nodes: i64 = conn.query_row("SELECT COUNT(*) FROM _adb_nodes", [], |r| r.get(0))?;
        let edges: i64 = conn.query_row("SELECT COUNT(*) FROM _adb_edges", [], |r| r.get(0))?;
        let conversations: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_conversations", [], |r| r.get(0))?;
        let messages: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_messages", [], |r| r.get(0))?;
        let workflows: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_workflows", [], |r| r.get(0))?;
        let workflow_steps: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_workflow_steps", [], |r| r.get(0))?;
        let traces: i64 =
            conn.query_row("SELECT COUNT(*) FROM _adb_traces", [], |r| r.get(0))?;
        Ok(DbStats {
            collections,
            vectors,
            nodes,
            edges,
            conversations,
            messages,
            workflows,
            workflow_steps,
            traces,
        })
    }
}

/// Database-wide statistics returned by [`AgentDB::stats`].
#[derive(Debug)]
pub struct DbStats {
    /// Number of named vector collections.
    pub collections: i64,
    /// Total number of vectors across all collections.
    pub vectors: i64,
    /// Number of nodes in the memory graph.
    pub nodes: i64,
    /// Number of directed edges in the memory graph.
    pub edges: i64,
    /// Number of conversation threads.
    pub conversations: i64,
    /// Total number of messages across all conversations.
    pub messages: i64,
    /// Number of workflow records.
    pub workflows: i64,
    /// Total number of workflow steps across all workflows.
    pub workflow_steps: i64,
    /// Total number of reasoning trace entries.
    pub traces: i64,
}

fn rusqlite_value_to_json(val: rusqlite::types::Value) -> serde_json::Value {
    match val {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::Value::Number(i.into()),
        rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
        rusqlite::types::Value::Blob(b) => {
            serde_json::Value::String(format!("<blob {} bytes>", b.len()))
        }
    }
}
