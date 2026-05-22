//! # AgentDB
//!
//! A single-file embedded database for AI agents.
//! Combines relational SQL, vector search, full-text search,
//! hybrid queries, and episodic memory graphs in one Rust crate.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use agentdb::{AgentDB, VectorEntry, TraversalOptions};
//! use serde_json::json;
//!
//! fn main() -> agentdb::Result<()> {
//!     let db = AgentDB::open("agent.agentdb")?;
//!
//!     // Relational
//!     db.execute("CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, kind TEXT)")?;
//!
//!     // Vector
//!     let col = db.vectors().collection("thoughts", 4)?;
//!     col.upsert(VectorEntry {
//!         id: "t1".into(), vector: vec![0.1, 0.8, 0.3, 0.0],
//!         metadata: Some(json!({"text": "hello"})),
//!     })?;
//!     let results = col.search(&[0.1, 0.8, 0.3, 0.0], Default::default())?;
//!
//!     // Graph
//!     let graph = db.memory();
//!     graph.add_node("s1", "session", None)?;
//!     graph.add_node("c1", "concept", None)?;
//!     graph.add_edge("s1", "c1", "discussed", 0.9)?;
//!
//!     Ok(())
//! }
//! ```

pub mod db;
pub mod error;
pub mod filter;
pub mod fts;
pub mod hybrid;
pub mod memory;
pub mod schema;
pub mod vectors;

pub use db::{AgentDB, DbStats};
pub use error::{AgentDbError, Result};
pub use filter::matches as filter_matches;
pub use fts::{FtsResult, FullTextStore};
pub use hybrid::{HybridQuery, HybridResult, HybridStore};
pub use memory::{Edge, MemoryGraph, Node, TraversalOptions, TraversalResult};
pub use vectors::{
    collection::{BatchEntry, Collection, SearchOptions, SearchResult, VectorEntry},
    hnsw::DistanceMetric,
    VectorStore,
};
