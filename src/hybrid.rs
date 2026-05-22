use rusqlite::{params, Connection};
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::error::{AgentDbError, Result};
use crate::memory::graph::{Node, TraversalOptions, TraversalResult};
use crate::vectors::collection::{Collection, SearchOptions, SearchResult};
use crate::filter;

/// A single result from a hybrid query
#[derive(Debug, Clone)]
pub struct HybridResult {
    pub id: String,
    /// Normalized vector similarity score [0, 1]
    pub vector_score: f32,
    /// Graph edge weight reaching this node [0, 1]
    pub graph_weight: f64,
    /// Final blended rank score
    pub rank_score: f64,
    pub metadata: Option<Value>,
}

/// Options for a hybrid graph + vector query
pub struct HybridQuery<'a> {
    /// ID of the anchor node in the memory graph
    pub anchor_node: &'a str,
    /// Query embedding vector
    pub embedding: &'a [f32],
    /// Name of the vector collection to search
    pub collection: &'a str,
    /// Max depth to traverse from anchor node
    pub graph_depth: usize,
    /// Number of results to return
    pub top_k: usize,
    /// Blending factor: 0.0 = pure graph, 1.0 = pure vector
    pub alpha: f64,
    /// Optional metadata filter on vector results
    pub filter: Option<Value>,
}

pub struct HybridStore {
    conn: Arc<Mutex<Connection>>,
}

impl HybridStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn query(
        &self,
        q: HybridQuery,
        col: &Collection,
    ) -> Result<Vec<HybridResult>> {
        // ── Step 1: Graph traversal from anchor node ────────────────────────
        let graph = crate::memory::MemoryGraph::new(Arc::clone(&self.conn));
        let traversal: Vec<TraversalResult> = graph
            .neighbors(
                q.anchor_node,
                TraversalOptions {
                    relation: None,
                    max_depth: q.graph_depth,
                    min_weight: Some(0.0),
                },
            )
            .unwrap_or_default();

        // Build a map: node_id -> max graph weight reaching it
        let mut graph_weights: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for t in &traversal {
            let entry = graph_weights
                .entry(t.node.id.clone())
                .or_insert(0.0);
            if t.weight > *entry {
                *entry = t.weight;
            }
        }

        // ── Step 2: Vector ANN search (fetch extra candidates) ───────────────
        let fetch_k = (q.top_k * 20).max(100);
        let vec_results: Vec<SearchResult> = col.search(
            q.embedding,
            SearchOptions {
                top_k: fetch_k,
                metric: crate::vectors::hnsw::DistanceMetric::Cosine,
                filter: q.filter.clone(),
            },
        )?;

        // Normalize vector scores to [0, 1] (scores are distances; lower = better)
        let max_score = vec_results
            .iter()
            .map(|r| r.score)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_score = vec_results
            .iter()
            .map(|r| r.score)
            .fold(f32::INFINITY, f32::min);
        let score_range = (max_score - min_score).max(1e-6);

        // ── Step 3: Blend scores ──────────────────────────────────────────
        let mut blended: Vec<HybridResult> = vec_results
            .into_iter()
            .map(|r| {
                // Invert distance to similarity [0, 1]
                let vec_sim =
                    1.0 - ((r.score - min_score) / score_range) as f64;
                let gw = graph_weights.get(&r.id).copied().unwrap_or(0.0);
                let rank = q.alpha * vec_sim + (1.0 - q.alpha) * gw;
                HybridResult {
                    id: r.id,
                    vector_score: r.score,
                    graph_weight: gw,
                    rank_score: rank,
                    metadata: r.metadata,
                }
            })
            .collect();

        // ── Step 4: Sort by rank score descending, take top_k ───────────────
        blended.sort_by(|a, b| {
            b.rank_score
                .partial_cmp(&a.rank_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        blended.truncate(q.top_k);

        Ok(blended)
    }
}
