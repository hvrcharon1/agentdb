use rusqlite::params;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use crate::vectors::collection::{SearchOptions, SearchResult, VectorEntry};
use crate::vectors::hnsw::{DistanceMetric, HnswIndex};
use crate::filter;
use rusqlite::Connection;

/// Batch upsert input
pub struct BatchEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: Option<Value>,
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
        Self { id, name, dim, conn, index: Mutex::new(None), metric }
    }

    /// Insert or update a single vector entry
    pub fn upsert(&self, entry: VectorEntry) -> Result<()> {
        if entry.vector.len() != self.dim {
            return Err(AgentDbError::DimensionMismatch {
                expected: self.dim,
                got: entry.vector.len(),
            });
        }
        let blob: Vec<u8> = entry.vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let metadata_str = entry.metadata.as_ref().map(|m| m.to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_vectors (id, collection_id, vector, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id, collection_id) DO UPDATE SET
               vector = excluded.vector,
               metadata = excluded.metadata",
            params![entry.id, self.id, blob, metadata_str, now_ms()],
        )?;
        conn.execute(
            "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
             VALUES (?1, X'', ?2, 1)
             ON CONFLICT(collection_id) DO UPDATE SET is_dirty = 1",
            params![self.id, now_ms()],
        )?;
        conn.execute(
            "UPDATE _adb_collections SET count = count + 1 WHERE id = ?1",
            params![self.id],
        )?;
        *self.index.lock().unwrap() = None;
        Ok(())
    }

    /// Batch insert or update multiple vector entries in a single transaction
    pub fn upsert_batch(&self, entries: Vec<BatchEntry>) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        // Validate all dimensions upfront
        for e in &entries {
            if e.vector.len() != self.dim {
                return Err(AgentDbError::DimensionMismatch {
                    expected: self.dim,
                    got: e.vector.len(),
                });
            }
        }

        let count = entries.len();
        let conn = self.conn.lock().unwrap();

        conn.execute_batch("BEGIN")?;

        let result = (|| -> Result<()> {
            for entry in &entries {
                let blob: Vec<u8> = entry.vector.iter().flat_map(|f| f.to_le_bytes()).collect();
                let metadata_str = entry.metadata.as_ref().map(|m| m.to_string());
                conn.execute(
                    "INSERT INTO _adb_vectors (id, collection_id, vector, metadata, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id, collection_id) DO UPDATE SET
                       vector = excluded.vector,
                       metadata = excluded.metadata",
                    params![entry.id, self.id, blob, metadata_str, now_ms()],
                )?;
            }
            conn.execute(
                "INSERT INTO _adb_hnsw_index (collection_id, index_blob, built_at, is_dirty)
                 VALUES (?1, X'', ?2, 1)
                 ON CONFLICT(collection_id) DO UPDATE SET is_dirty = 1",
                params![self.id, now_ms()],
            )?;
            conn.execute(
                "UPDATE _adb_collections SET count = count + ?1 WHERE id = ?2",
                params![count as i64, self.id],
            )?;
            Ok(())
        })();

        match result {
            Ok(_) => {
                conn.execute_batch("COMMIT")?;
                *self.index.lock().unwrap() = None;
                Ok(count)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// ANN search with advanced metadata filtering
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

        // Fetch more candidates if filtering, to ensure top_k results after filter
        let fetch_k = if opts.filter.is_some() {
            (opts.top_k * 10).max(50)
        } else {
            opts.top_k
        };

        let raw_results = index.search(query, fetch_k);
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

            let metadata_val: Option<Value> =
                metadata.as_deref().and_then(|s| serde_json::from_str(s).ok());

            // Apply advanced filter
            if let Some(ref f) = opts.filter {
                match &metadata_val {
                    Some(meta) => {
                        if !filter::matches(meta, f) {
                            continue;
                        }
                    }
                    None => continue,
                }
            }

            results.push(SearchResult { id, score, metadata: metadata_val });

            if results.len() >= opts.top_k {
                break;
            }
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

    /// Force rebuild the HNSW index
    pub fn reindex(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, vector FROM _adb_vectors WHERE collection_id = ?1",
        )?;
        let mut index = HnswIndex::new(16, 200, self.metric.clone());
        let rows = stmt.query_map(params![self.id], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })?;
        for row in rows {
            let (id, blob) = row?;
            let vector: Vec<f32> = blob
                .chunks_exact(4)
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
               built_at   = excluded.built_at,
               is_dirty   = 0",
            params![self.id, serialized, now_ms()],
        )?;
        drop(conn);
        *self.index.lock().unwrap() = Some(index);
        Ok(())
    }

    fn ensure_index(&self) -> Result<()> {
        if self.index.lock().unwrap().is_some() {
            return Ok(());
        }
        self.reindex()
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
