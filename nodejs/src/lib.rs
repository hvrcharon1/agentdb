//! napi-rs Node.js bindings for AgentDB.
//!
//! Build:
//! ```bash
//! cd nodejs
//! npm install
//! npm run build
//! ```
//!
//! Publish:
//! ```bash
//! npm publish
//! ```
//!
//! Usage:
//! ```js
//! const { AgentDB } = require('agentdb');
//! const db = AgentDB.open(':memory:');
//! db.execute('CREATE TABLE notes (id TEXT PRIMARY KEY)');
//! const col = db.collection('thoughts', 4);
//! col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { score: 9 });
//! const results = col.search([0.9, 0.1, 0.0, 0.0], { topK: 5 });
//! ```

#![allow(dead_code)]

use agentdb::{
    AgentDB as RustDB, BatchEntry, DistanceMetric, HybridQuery, SearchOptions, TraversalOptions,
    VectorEntry,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;
use std::sync::{Arc, Mutex};

// ── SearchResult ──────────────────────────────────────────────────────

#[napi(object)]
#[derive(Clone)]
pub struct SearchResult {
    pub id:       String,
    pub score:    f64,
    pub metadata: Option<serde_json::Value>,
}

// ── FtsResult ─────────────────────────────────────────────────────────

#[napi(object)]
#[derive(Clone)]
pub struct FtsResult {
    pub id:      String,
    pub snippet: String,
    pub rank:    f64,
}

// ── HybridResult ──────────────────────────────────────────────────────

#[napi(object)]
#[derive(Clone)]
pub struct HybridResult {
    pub id:           String,
    pub rank_score:   f64,
    pub vector_score: f64,
    pub graph_weight: f64,
}

// ── NeighborResult ────────────────────────────────────────────────────

#[napi(object)]
#[derive(Clone)]
pub struct NeighborResult {
    pub id:     String,
    pub kind:   String,
    pub depth:  u32,
    pub weight: f64,
    pub data:   Option<serde_json::Value>,
}

// ── DbStats ───────────────────────────────────────────────────────────

#[napi(object)]
#[derive(Clone)]
pub struct DbStats {
    pub collections: i64,
    pub vectors:     i64,
    pub nodes:       i64,
    pub edges:       i64,
}

// ── Collection ────────────────────────────────────────────────────────

#[napi]
pub struct Collection {
    inner: agentdb::Collection,
}

#[napi]
impl Collection {
    /// Upsert a single vector.
    #[napi]
    pub fn upsert(
        &self,
        id: String,
        vector: Vec<f64>,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let vec_f32: Vec<f32> = vector.iter().map(|&v| v as f32).collect();
        self.inner
            .upsert(VectorEntry { id, vector: vec_f32, metadata })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Upsert multiple vectors in a single transaction.
    #[napi]
    pub fn upsert_batch(&self, entries: Vec<serde_json::Value>) -> Result<u32> {
        let batch: std::result::Result<Vec<BatchEntry>, _> = entries
            .iter()
            .map(|e| {
                let id = e["id"].as_str().ok_or_else(|| Error::from_reason("missing 'id'"))?.to_string();
                let vector: Vec<f32> = e["vector"]
                    .as_array()
                    .ok_or_else(|| Error::from_reason("missing 'vector'"))?
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();
                let metadata = e.get("metadata").cloned();
                Ok(BatchEntry { id, vector, metadata })
            })
            .collect();
        self.inner
            .upsert_batch(batch?)
            .map(|n| n as u32)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// ANN search. Returns an array of SearchResult objects.
    #[napi]
    pub fn search(
        &self,
        query: Vec<f64>,
        top_k: Option<u32>,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<SearchResult>> {
        let q: Vec<f32> = query.iter().map(|&v| v as f32).collect();
        self.inner
            .search(
                &q,
                SearchOptions {
                    top_k:  top_k.unwrap_or(10) as usize,
                    metric: DistanceMetric::Cosine,
                    filter,
                },
            )
            .map(|results| {
                results
                    .into_iter()
                    .map(|r| SearchResult { id: r.id, score: r.score as f64, metadata: r.metadata })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Number of vectors in this collection.
    #[napi]
    pub fn count(&self) -> Result<i64> {
        self.inner.count().map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Rebuild the HNSW index.
    #[napi]
    pub fn reindex(&self) -> Result<()> {
        self.inner.reindex().map_err(|e| Error::from_reason(e.to_string()))
    }
}

// ── AgentDB ───────────────────────────────────────────────────────────

#[napi]
pub struct AgentDB {
    db: Arc<Mutex<RustDB>>,
}

#[napi]
impl AgentDB {
    /// Open or create an AgentDB database.
    #[napi(factory)]
    pub fn open(path: String) -> Result<Self> {
        RustDB::open(&path)
            .map(|db| AgentDB { db: Arc::new(Mutex::new(db)) })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Execute a raw SQL statement. Returns rows affected.
    #[napi]
    pub fn execute(&self, sql: String) -> Result<u32> {
        self.db
            .lock()
            .unwrap()
            .execute(&sql)
            .map(|n| n as u32)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Query and return rows as an array of plain objects.
    #[napi]
    pub fn query(&self, sql: String) -> Result<Vec<serde_json::Value>> {
        self.db
            .lock()
            .unwrap()
            .query_json(&sql)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Get or create a vector collection.
    #[napi]
    pub fn collection(&self, name: String, dim: u32) -> Result<Collection> {
        self.db
            .lock()
            .unwrap()
            .vectors()
            .collection(&name, dim as usize)
            .map(|inner| Collection { inner })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Add or update a memory graph node.
    #[napi]
    pub fn add_node(
        &self,
        id: String,
        kind: String,
        data: Option<serde_json::Value>,
    ) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .memory()
            .add_node(&id, &kind, data)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Add or update a directed weighted edge.
    #[napi]
    pub fn add_edge(
        &self,
        src: String,
        dst: String,
        relation: String,
        weight: f64,
    ) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .memory()
            .add_edge(&src, &dst, &relation, weight)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Traverse the memory graph from a node.
    #[napi]
    pub fn neighbors(
        &self,
        node_id: String,
        max_depth: Option<u32>,
        min_weight: Option<f64>,
    ) -> Result<Vec<NeighborResult>> {
        let opts = TraversalOptions {
            relation:   None,
            max_depth:  max_depth.unwrap_or(2) as usize,
            min_weight: Some(min_weight.unwrap_or(0.0)),
        };
        self.db
            .lock()
            .unwrap()
            .memory()
            .neighbors(&node_id, opts)
            .map(|results| {
                results
                    .into_iter()
                    .map(|r| NeighborResult {
                        id:     r.node.id,
                        kind:   r.node.kind,
                        depth:  r.depth as u32,
                        weight: r.weight,
                        data:   r.node.data,
                    })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Index text for full-text search.
    #[napi]
    pub fn fts_index(
        &self,
        collection: String,
        id: String,
        collection_id: String,
        text: String,
    ) -> Result<()> {
        self.db
            .lock()
            .unwrap()
            .fts()
            .index_text(&collection, &id, &collection_id, &text)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Full-text search.
    #[napi]
    pub fn fts_search(
        &self,
        collection: String,
        query: String,
        top_k: u32,
    ) -> Result<Vec<FtsResult>> {
        self.db
            .lock()
            .unwrap()
            .fts()
            .search(&collection, &query, top_k as usize)
            .map(|results| {
                results
                    .into_iter()
                    .map(|r| FtsResult { id: r.id, snippet: r.snippet, rank: r.rank })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Hybrid graph + vector query.
    #[napi]
    pub fn hybrid_query(
        &self,
        anchor_node: String,
        embedding: Vec<f64>,
        collection: String,
        graph_depth: Option<u32>,
        top_k: Option<u32>,
        alpha: Option<f64>,
    ) -> Result<Vec<HybridResult>> {
        let emb: Vec<f32> = embedding.iter().map(|&v| v as f32).collect();
        let db = self.db.lock().unwrap();
        let q = HybridQuery {
            anchor_node: &anchor_node,
            embedding:   &emb,
            collection:  &collection,
            graph_depth: graph_depth.unwrap_or(2) as usize,
            top_k:       top_k.unwrap_or(10) as usize,
            alpha:       alpha.unwrap_or(0.6),
            filter:      None,
        };
        db.hybrid_query(q)
            .map(|results| {
                results
                    .into_iter()
                    .map(|r| HybridResult {
                        id:           r.id,
                        rank_score:   r.rank_score,
                        vector_score: r.vector_score as f64,
                        graph_weight: r.graph_weight,
                    })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Return database-wide statistics.
    #[napi]
    pub fn stats(&self) -> Result<DbStats> {
        self.db
            .lock()
            .unwrap()
            .stats()
            .map(|s| DbStats {
                collections: s.collections,
                vectors:     s.vectors,
                nodes:       s.nodes,
                edges:       s.edges,
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}
