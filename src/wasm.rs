//! # AgentDB — WASM bindings (stub)
//!
//! Status: in-progress. In-memory databases work today via wasm-pack.
//! Persistent storage requires an OPFS (Origin Private File System) VFS
//! adapter for SQLite, which is tracked in the v0.4.0 milestone.
//!
//! ## Building today (in-memory only)
//!
//! ```bash
//! cargo install wasm-pack
//! wasm-pack build --target web --features wasm
//! ```
//!
//! ## Usage in the browser
//!
//! ```js
//! import init, { WasmAgentDB } from './pkg/agentdb.js';
//! await init();
//! const db = WasmAgentDB.open_memory();
//! db.execute("CREATE TABLE notes (id TEXT PRIMARY KEY, body TEXT)");
//! const stats = JSON.parse(db.stats());
//! console.log(stats); // { collections: 0, vectors: 0, nodes: 0, edges: 0 }
//! ```

use wasm_bindgen::prelude::*;

use crate::db::AgentDB;
use crate::vectors::{DistanceMetric, SearchOptions, VectorEntry};
use serde_json::Value;

/// WASM-exposed AgentDB handle.
///
/// All methods accept and return JSON strings to avoid complex
/// ownership issues across the WASM boundary.
#[wasm_bindgen]
pub struct WasmAgentDB {
    db: AgentDB,
}

#[wasm_bindgen]
impl WasmAgentDB {
    /// Open an in-memory AgentDB database.
    ///
    /// Persistent file storage via OPFS is tracked for v0.4.0.
    #[wasm_bindgen(js_name = open_memory)]
    pub fn open_memory() -> Result<WasmAgentDB, JsValue> {
        AgentDB::open(":memory:")
            .map(|db| WasmAgentDB { db })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute a raw SQL statement. Returns rows affected.
    pub fn execute(&self, sql: &str) -> Result<i64, JsValue> {
        self.db
            .execute(sql)
            .map(|n| n as i64)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Query and return all rows as a JSON array string.
    pub fn query_json(&self, sql: &str) -> Result<String, JsValue> {
        self.db
            .query_json(sql)
            .map(|rows| Value::Array(rows).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Upsert a vector. `metadata_json` may be an empty string.
    pub fn vector_upsert(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata_json: &str,
    ) -> Result<(), JsValue> {
        let meta: Option<Value> = if metadata_json.is_empty() {
            None
        } else {
            serde_json::from_str(metadata_json).ok()
        };
        let dim = vector.len();
        let col = self
            .db
            .vectors()
            .collection(collection, dim)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        col.upsert(VectorEntry {
            id: id.to_string(),
            vector,
            metadata: meta,
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search a vector collection. Returns a JSON array string.
    pub fn vector_search(
        &self,
        collection: &str,
        query: Vec<f32>,
        top_k: usize,
    ) -> Result<String, JsValue> {
        let dim = query.len();
        let col = self
            .db
            .vectors()
            .collection(collection, dim)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        col.search(
            &query,
            SearchOptions {
                top_k,
                metric: DistanceMetric::Cosine,
                filter: None,
            },
        )
        .map(|results| {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({ "id": r.id, "score": r.score }))
                .collect();
            Value::Array(json).to_string()
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Add a node to the memory graph.
    pub fn graph_add_node(&self, id: &str, kind: &str, data_json: &str) -> Result<(), JsValue> {
        let data: Option<Value> = if data_json.is_empty() {
            None
        } else {
            serde_json::from_str(data_json).ok()
        };
        self.db
            .memory()
            .add_node(id, kind, data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Add a directed edge in the memory graph.
    pub fn graph_add_edge(
        &self,
        src: &str,
        dst: &str,
        relation: &str,
        weight: f64,
    ) -> Result<(), JsValue> {
        self.db
            .memory()
            .add_edge(src, dst, relation, weight)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return database stats as a JSON object string.
    pub fn stats(&self) -> Result<String, JsValue> {
        self.db
            .stats()
            .map(|s| {
                serde_json::json!({
                    "collections": s.collections,
                    "vectors": s.vectors,
                    "nodes": s.nodes,
                    "edges": s.edges
                })
                .to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
