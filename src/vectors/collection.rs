use rusqlite::{params, Connection};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use crate::vectors::hnsw::{DistanceMetric, HnswIndex};

#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: Option<Value>,
}

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

pub struct Collection {
    pub id: String,
    pub name: String,
    pub dim: usize,
    conn: Arc<Mutex<Connection>>,
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

    /// Insert or update a vector entry
    pub fn upsert(&self, entry: VectorEntry) -> Result<()> {
        if entry.vector.len() != self.dim {
            return Err(AgentDbError::DimensionMismatch {
                expected: self.dim,
                got: entry.vector.len(),
            });
        }

        // Serialize vector as raw f32 bytes
        let blob: Vec<u8> = entry.vector.iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        let metadata_str = entry.metadata
            .as_ref()
            .map(|m| m.to_string());

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_vectors (id, collection_id, vector, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id, collection_id) DO UPDATE SET
               vector = excluded.vector,
               metadata = excluded.metadata",
            params![entry.id, self.id, blob, metadata_str, now_ms()],
        )?;

        // Mark index as dirty
        conn.execute(
            "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
             VALUES (?1, X'', ?2, 1)
             ON CONFLICT(collection_id) DO UPDATE SET is_dirty = 1",
            params![self.id, now_ms()],
        )?;

        // Update count
        conn.execute(
            "UPDATE _adb_collections SET count = count + 1 WHERE id = ?1",
            params![self.id],
        )?;

        // Invalidate in-memory index
        *self.index.lock().unwrap() = None;

        Ok(())
    }

    /// Search for nearest neighbors
    pub fn search(&self, query: &[f32], opts: SearchOptions) -> Result<Vec<SearchResult>> {
        if query.len() != self.dim {
            return Err(AgentDbError::DimensionMismatch {
                expected: self.dim,
                got: query.len(),
            });
        }

        self.ensure_index()?;

        let index_guard = self.index.lock().unwrap();
        let index = index_guard.as_ref().unwrap();

        let raw_results = index.search(query, opts.top_k);

        // Fetch metadata for results
        let conn = self.conn.lock().unwrap();
        let mut results = Vec::new();

        for (id, score) in raw_results {
            let metadata: Option<String> = conn
                .query_row(
                    "SELECT metadata FROM _adb_vectors WHERE id = ?1 AND collection_id = ?2",
                    params![id, self.id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            let metadata_val: Option<Value> = metadata
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok());

            // Apply metadata filter if provided
            if let Some(ref filter) = opts.filter {
                if let Some(ref meta) = metadata_val {
                    if !metadata_matches(meta, filter) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            results.push(SearchResult {
                id,
                score,
                metadata: metadata_val,
            });
        }

        Ok(results)
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_vectors WHERE id = ?1 AND collection_id = ?2",
            params![id, self.id],
        )?;
        conn.execute(
            "UPDATE _adb_hnsw_index SET is_dirty = 1 WHERE collection_id = ?1",
            params![self.id],
        )?;
        *self.index.lock().unwrap() = None;
        Ok(())
    }

    /// Force rebuild the HNSW index from stored vectors
    pub fn reindex(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, vector FROM _adb_vectors WHERE collection_id = ?1"
        )?;

        let mut index = HnswIndex::new(16, 200, self.metric.clone());

        let rows = stmt.query_map(params![self.id], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })?;

        for row in rows {
            let (id, blob) = row?;
            let vector: Vec<f32> = blob.chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            index.insert(&id, vector);
        }

        let serialized = index.serialize()?;
        conn.execute(
            "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
             VALUES (?1, ?2, ?3, 0)
             ON CONFLICT(collection_id) DO UPDATE SET
               index_blob = excluded.index_blob,
               built_at = excluded.built_at,
               is_dirty = 0",
            params![self.id, serialized, now_ms()],
        )?;

        drop(conn);
        *self.index.lock().unwrap() = Some(index);
        Ok(())
    }

    fn ensure_index(&self) -> Result<()> {
        let mut guard = self.index.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
        drop(guard);
        self.reindex()?;
        Ok(())
    }

    pub fn count(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT count FROM _adb_collections WHERE id = ?1",
            params![self.id],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

fn metadata_matches(meta: &Value, filter: &Value) -> bool {
    if let (Value::Object(m), Value::Object(f)) = (meta, filter) {
        for (k, v) in f {
            if m.get(k) != Some(v) {
                return false;
            }
        }
        true
    } else {
        false
    }
}

pub struct VectorStore {
    conn: Arc<Mutex<Connection>>,
}

impl VectorStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Get or create a vector collection
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

        // Try to find existing collection
        let existing: Option<(String, usize, String)> = conn
            .query_row(
                "SELECT id, dim, metric FROM _adb_collections WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        if let Some((id, existing_dim, metric_str)) = existing {
            if existing_dim != dim {
                return Err(AgentDbError::DimensionMismatch {
                    expected: existing_dim,
                    got: dim,
                });
            }
            let metric = match metric_str.as_str() {
                "euclidean" => DistanceMetric::Euclidean,
                "dot" => DistanceMetric::DotProduct,
                _ => DistanceMetric::Cosine,
            };
            return Ok(Collection::new(id, name.to_string(), dim, metric, Arc::clone(&self.conn)));
        }

        // Create new collection
        let id = Uuid::new_v4().to_string();
        let metric_str = match &metric {
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Euclidean => "euclidean",
            DistanceMetric::DotProduct => "dot",
        };

        conn.execute(
            "INSERT INTO _adb_collections (id, name, dim, metric, count, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![id, name, dim, metric_str, now_ms()],
        )?;

        Ok(Collection::new(id, name.to_string(), dim, metric, Arc::clone(&self.conn)))
    }

    /// List all collections
    pub fn list_collections(&self) -> Result<Vec<(String, usize, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, dim, count FROM _adb_collections ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?, row.get::<_, i64>(2)?))
        })?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Drop a collection and all its vectors
    pub fn drop_collection(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM _adb_collections WHERE name = ?1", params![name])?;
        Ok(())
    }
}
