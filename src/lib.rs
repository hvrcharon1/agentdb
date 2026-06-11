//! # AgentDB v0.3.0
//!
//! A single-file embedded database for AI agents.
//!
//! **Five layers, one file, zero servers:**
//!
//! | Layer | What it gives you |
//! |---|---|
//! | Relational SQL | Full SQL engine, ACID, WAL, user-defined tables |
//! | Vector Store | HNSW ANN search, cosine/euclidean/dot, batch upsert |
//! | Memory Graph | Typed nodes, weighted edges, recursive CTE traversal |
//! | Full-Text Search | FTS5 virtual tables, BM25 ranking, Porter stemmer |
//! | Hybrid Queries | Graph traversal + vector ANN with alpha blending |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use agentdb::{AgentDB, VectorEntry, SearchOptions, DistanceMetric};
//! use serde_json::json;
//!
//! let db = AgentDB::open(":memory:").unwrap();
//!
//! // SQL
//! db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY)").unwrap();
//!
//! // Vectors
//! let col = db.vectors().collection("thoughts", 4).unwrap();
//! col.upsert(VectorEntry {
//!     id: "t1".into(),
//!     vector: vec![0.9, 0.1, 0.0, 0.0],
//!     metadata: Some(json!({ "score": 9 })),
//! }).unwrap();
//!
//! // Memory graph
//! let graph = db.memory();
//! graph.add_node("s1", "session", None).unwrap();
//! graph.add_node("t1", "thought", None).unwrap();
//! graph.add_edge("s1", "t1", "recalled", 0.9).unwrap();
//!
//! // Stats
//! let stats = db.stats().unwrap();
//! println!("nodes={} edges={}", stats.nodes, stats.edges);
//! ```
//!
//! ## Feature flags
//!
//! | Flag | What it enables |
//! |---|---|
//! | `async` | Tokio async runtime wrappers |
//! | `ffi` | C FFI flat API (`extern "C"` functions in `src/ffi.rs`) |
//! | `python` | PyO3 Python bindings (enables `ffi`) |
//! | `wasm` | WASM/wasm-bindgen target |

pub mod db;
pub mod error;
pub mod filter;
pub mod fts;
pub mod hybrid;
pub mod memory;
pub mod schema;
pub mod vectors;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use db::{AgentDB, DbStats};
pub use error::{AgentDbError, Result};
pub use filter::matches as filter_matches;
pub use fts::{FtsResult, FullTextStore};
pub use hybrid::{HybridQuery, HybridResult, HybridStore};
pub use memory::{Edge, MemoryGraph, Node, TraversalOptions, TraversalResult};
pub use vectors::{
    BatchEntry, Collection, DistanceMetric, SearchOptions, SearchResult, VectorEntry, VectorStore,
};
