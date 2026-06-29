use crate::error::{AgentDbError, Result};
use crate::filter;
use crate::fts::FullTextStore;
use crate::schema::now_ms;
use crate::vectors::hnsw::{DistanceMetric, HnswIndex};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A single vector entry
#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: Option<Value>,
}

/// A single vector search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: Option<Value>,
}

/// Options controlling a vector search
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub top_k: usize,
    pub metric: DistanceMetric,
    pub filter: Option<Value>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: None,
        }
    }
}

/// An entry for batch upsert
#[derive(Debug, Clone)]
pub struct BatchEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: Option<Value>,
}

/// A named vector collection
pub struct Collection {
    pub id: String,
    pub name: String,
    pub dim: usize,
    pub(crate) conn: Arc<Mutex<Connection>>,
    index: Mutex<Option<HnswIndex>>,
    metric: DistanceMetric,
}

impl Collection {
    pub(crate) fn new(
        id: String,
        name: String,
        dim: usize,
        metric: DistanceMetric,
        conn: Arc<Mutex<Connection>>,
    ) -> Self {
        Self {
            id,
            name,
            dim,
            conn,
            index: Mutex::new(None),
            metric,
        }
    }

    /// Insert or update a single vector
    pub fn upsert(&self, entry: VectorEntry) -> Result<()> {
        if entry.vector.len() != self.dim {
            return Err(AgentDbError::DimensionMismatch {
                expected: self.dim,
                got: entry.vector.len(),
            });
        }
        let blob: Vec<u8> = entry.vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let meta = entry.metadata.as_ref().map(|m| m.to_string());
        let now = now_ms();
        let conn = self.conn.lock().unwrap();
        // INSERT OR IGNORE returns changes()=1 for a new row, 0 for a duplicate.
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO _adb_vectors
                 (id, collection_id, vector, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![entry.id, self.id, blob, meta, now],
        )?;
        if inserted == 0 {
            conn.execute(
                "UPDATE _adb_vectors SET vector = ?1, metadata = ?2, updated_at = ?5
                 WHERE id = ?3 AND collection_id = ?4",
                params![blob, meta, entry.id, self.id, now],
            )?;
        }
        conn.execute(
            "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
             VALUES (?1, X'', ?2, 1)
             ON CONFLICT(collection_id) DO UPDATE SET is_dirty = 1",
            params![self.id, now_ms()],
        )?;
        if inserted > 0 {
            conn.execute(
                "UPDATE _adb_collections SET count = count + 1 WHERE id = ?1",
                params![self.id],
            )?;
        }
        *self.index.lock().unwrap() = None;
        Ok(())
    }

    /// Insert or update multiple vectors in a single transaction
    pub fn upsert_batch(&self, entries: Vec<BatchEntry>) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }
        for e in &entries {
            if e.vector.len() != self.dim {
                return Err(AgentDbError::DimensionMismatch {
                    expected: self.dim,
                    got: e.vector.len(),
                });
            }
        }
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("BEGIN")?;
        let result: Result<usize> = (|| {
            let mut new_rows: usize = 0;
            for e in &entries {
                let blob: Vec<u8> = e.vector.iter().flat_map(|f| f.to_le_bytes()).collect();
                let meta = e.metadata.as_ref().map(|m| m.to_string());
                let now = now_ms();
                // INSERT OR IGNORE: changes()=1 for new, 0 for existing
                let inserted = conn.execute(
                    "INSERT OR IGNORE INTO _adb_vectors
                         (id, collection_id, vector, metadata, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![e.id, self.id, blob, meta, now],
                )?;
                if inserted == 0 {
                    conn.execute(
                        "UPDATE _adb_vectors SET vector = ?1, metadata = ?2, updated_at = ?5
                         WHERE id = ?3 AND collection_id = ?4",
                        params![blob, meta, e.id, self.id, now],
                    )?;
                } else {
                    new_rows += 1;
                }
            }
            conn.execute(
                "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
                 VALUES (?1, X'', ?2, 1)
                 ON CONFLICT(collection_id) DO UPDATE SET is_dirty = 1",
                params![self.id, now_ms()],
            )?;
            if new_rows > 0 {
                conn.execute(
                    "UPDATE _adb_collections SET count = count + ?1 WHERE id = ?2",
                    params![new_rows as i64, self.id],
                )?;
            }
            Ok(new_rows)
        })();
        match result {
            Ok(new_rows) => {
                conn.execute_batch("COMMIT")?;
                *self.index.lock().unwrap() = None;
                Ok(new_rows)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// ANN search with optional advanced metadata filtering
    pub fn search(&self, query: &[f32], opts: SearchOptions) -> Result<Vec<SearchResult>> {
        if query.len() != self.dim {
            return Err(AgentDbError::DimensionMismatch {
                expected: self.dim,
                got: query.len(),
            });
        }
        self.ensure_index()?;
        let guard = self.index.lock().unwrap();
        let index = guard.as_ref().unwrap();
        let fetch_k = if opts.filter.is_some() {
            (opts.top_k * 10).max(50)
        } else {
            opts.top_k
        };
        let raw = index.search(query, fetch_k);
        let conn = self.conn.lock().unwrap();
        let mut out = Vec::new();
        for (id, score) in raw {
            let meta_str: Option<String> = conn
                .query_row(
                    "SELECT metadata FROM _adb_vectors
                     WHERE id = ?1 AND collection_id = ?2",
                    params![id, self.id],
                    |r| r.get(0),
                )
                .ok()
                .flatten();
            let meta: Option<Value> = meta_str
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());
            if let Some(ref f) = opts.filter {
                match &meta {
                    Some(m) if filter::matches(m, f) => {}
                    _ => continue,
                }
            }
            out.push(SearchResult {
                id,
                score,
                metadata: meta,
            });
            if out.len() >= opts.top_k {
                break;
            }
        }
        Ok(out)
    }

    /// Insert or update a vector AND index its text content for FTS in one atomic call.
    ///
    /// This keeps the vector index and the FTS index in sync — callers no longer
    /// need to call `col.upsert()` + `fts.index_text()` separately.
    pub fn upsert_with_text(&self, entry: VectorEntry, text: &str) -> Result<()> {
        let id = entry.id.clone();
        self.upsert(entry)?;
        let fts = FullTextStore::new(Arc::clone(&self.conn));
        fts.index_text(&self.name, &id, &self.id, text)
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM _adb_vectors WHERE id = ?1 AND collection_id = ?2",
            params![id, self.id],
        )?;
        if deleted > 0 {
            conn.execute(
                "UPDATE _adb_collections SET count = MAX(0, count - 1) WHERE id = ?1",
                params![self.id],
            )?;
        }
        conn.execute(
            "UPDATE _adb_hnsw_index SET is_dirty = 1 WHERE collection_id = ?1",
            params![self.id],
        )?;
        *self.index.lock().unwrap() = None;
        Ok(())
    }

    /// Rebuild the HNSW index from stored vectors
    pub fn reindex(&self) -> Result<()> {
        let mut index = HnswIndex::new(16, 200, self.metric.clone());
        // Scope the borrow of conn so stmt and rows are dropped before the second lock.
        {
            let conn = self.conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT id, vector FROM _adb_vectors WHERE collection_id = ?1")?;
            let rows = stmt.query_map(params![self.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?;
            for row in rows {
                let (id, blob) = row?;
                let vec: Vec<f32> = blob
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                index.insert(&id, vec);
            }
        }
        let serialized = index.serialize()?;
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
                 VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT(collection_id) DO UPDATE SET
                   index_blob = excluded.index_blob,
                   built_at   = excluded.built_at,
                   is_dirty   = 0",
                params![self.id, serialized, now_ms()],
            )?;
        }
        *self.index.lock().unwrap() = Some(index);
        Ok(())
    }

    fn ensure_index(&self) -> Result<()> {
        if self.index.lock().unwrap().is_some() {
            return Ok(());
        }
        // Try to deserialize the stored blob first; fall back to a full rebuild
        // only when the blob is absent, empty, or corrupt.
        let blob_opt: Option<Vec<u8>> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT index_blob FROM _adb_hnsw_index
                 WHERE collection_id = ?1 AND is_dirty = 0",
                params![self.id],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .ok()
        };
        if let Some(blob) = blob_opt {
            if !blob.is_empty() {
                if let Ok(index) = HnswIndex::deserialize(&blob) {
                    *self.index.lock().unwrap() = Some(index);
                    return Ok(());
                }
            }
        }
        self.reindex()
    }

    /// Number of vectors in this collection
    pub fn count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT count FROM _adb_collections WHERE id = ?1",
            params![self.id],
            |r| r.get(0),
        )?)
    }
}

/// Manages all vector collections
pub struct VectorStore {
    conn: Arc<Mutex<Connection>>,
}

impl VectorStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn collection(&self, name: &str, dim: usize) -> Result<Collection> {
        self.collection_with_metric(name, dim, DistanceMetric::Cosine)
    }

    pub fn collection_with_metric(
        &self,
        name: &str,
        dim: usize,
        metric: DistanceMetric,
    ) -> Result<Collection> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<(String, usize, String)> = conn
            .query_row(
                "SELECT id, dim, metric FROM _adb_collections WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize, row.get(2)?)),
            )
            .ok();
        if let Some((id, edim, mstr)) = existing {
            if edim != dim {
                return Err(AgentDbError::DimensionMismatch {
                    expected: edim,
                    got: dim,
                });
            }
            let m = match mstr.as_str() {
                "euclidean" => DistanceMetric::Euclidean,
                "dot" => DistanceMetric::DotProduct,
                _ => DistanceMetric::Cosine,
            };
            return Ok(Collection::new(
                id,
                name.to_string(),
                dim,
                m,
                Arc::clone(&self.conn),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let mstr = match &metric {
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Euclidean => "euclidean",
            DistanceMetric::DotProduct => "dot",
        };
        conn.execute(
            "INSERT INTO _adb_collections (id, name, dim, metric, count, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![id, name, dim as i64, mstr, now_ms()],
        )?;
        Ok(Collection::new(
            id,
            name.to_string(),
            dim,
            metric,
            Arc::clone(&self.conn),
        ))
    }

    pub fn list_collections(&self) -> Result<Vec<(String, usize, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT name, dim, count FROM _adb_collections ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)?,
            ))
        })?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    pub fn drop_collection(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_collections WHERE name = ?1",
            params![name],
        )?;
        Ok(())
    }
}
