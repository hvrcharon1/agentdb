//! # AgentDB
//!
//! A single-file embedded database for AI agents.
//! Combines relational SQL, vector search, and episodic memory graphs
//! in one SQLite-compatible engine — written in Rust.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use agentdb::{AgentDB, VectorEntry, TraversalOptions};
//! use serde_json::json;
//!
//! fn main() -> agentdb::Result<()> {
//!     // Open (or create) a database file
//!     let db = AgentDB::open("agent.agentdb")?;
//!
//!     // --- Relational layer ---
//!     db.execute("CREATE TABLE IF NOT EXISTS events (
//!         id TEXT PRIMARY KEY, kind TEXT, data TEXT, ts INTEGER
//!     )")?;
//!
//!     // --- Vector layer ---
//!     let col = db.vectors().collection("thoughts", 3)?;
//!     col.upsert(VectorEntry {
//!         id: "thought_1".into(),
//!         vector: vec![0.1, 0.8, 0.3],
//!         metadata: Some(json!({"text": "Rust is fast"})),
//!     })?;
//!     let results = col.search(&[0.1, 0.8, 0.3], Default::default())?;
//!
//!     // --- Memory graph layer ---
//!     let graph = db.memory();
//!     graph.add_node("session_1", "session", Some(json!({"user": "harshal"})))?;
//!     graph.add_node("concept_rust", "concept", Some(json!({"label": "Rust"})))?;
//!     graph.add_edge("session_1", "concept_rust", "discussed", 0.9)?;
//!
//!     let neighbors = graph.neighbors("session_1", TraversalOptions {
//!         relation: Some("discussed".into()),
//!         max_depth: 2,
//!         min_weight: Some(0.5),
//!     })?;
//!
//!     println!("Found {} neighbors", neighbors.len());
//!     Ok(())
//! }
//! ```

pub mod db;
pub mod error;
pub mod memory;
pub mod schema;
pub mod vectors;

pub use db::{AgentDB, DbStats};
pub use error::{AgentDbError, Result};
pub use memory::{Edge, MemoryGraph, Node, TraversalOptions, TraversalResult};
pub use vectors::{
    Collection, DistanceMetric, SearchOptions, SearchResult, VectorEntry, VectorStore,
};
