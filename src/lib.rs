//! # AgentDB v0.2.0
//!
//! A single-file embedded database for AI agents.
//! SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs.

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
    BatchEntry, Collection, DistanceMetric, SearchOptions, SearchResult, VectorEntry, VectorStore,
};
