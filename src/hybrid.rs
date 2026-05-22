use rusqlite::params;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::error::{AgentDbError, Result};
use crate::memory::MemoryGraph;
use crate::vectors::collection::{Collection, SearchOptions, SearchResult};
use crate::vectors::hnsw::DistanceMetric;
use rusqlite::Connection;

/// A single result from a hybrid graph + vector query
#[derive(Debug, Clone)]
pub struct HybridResult {
    pub id: String,
    pub vector_score: f32,
    pub graph_weight: f64,
    pub rank_score: f64,
    pub metadata: Option<Value>,
}

/// Options for a hybrid query
pub struct HybridQuery<'a> {
    pub anchor_node: &'a str,
    pub embedding: &'a [f32],
    pub collection: &'a str,
    pub graph_depth: usize,
    pub top_k: usize,
    /// 0.0 = pure graph, 1.0 = pure vector
    pub alpha: f64,
    pub filter: Option<Value>,
}

/// Runs hybrid graph + vector queries
pub struct HybridStore {
    conn: Arc<Mutex<Connection>>,
}

impl HybridStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn query(&self, q: HybridQuery, col: &Collection) -> Result<Vec<HybridResult>> {
        use crate::memory::TraversalOptions;
        // Step 1: graph traversal
        let graph = MemoryGraph::new(Arc::clone(&self.conn));
        let traversal = graph
            .neighbors(
                q.anchor_node,
                TraversalOptions {
                    relation: None,
                    max_depth: q.graph_depth,
                    min_weight: Some(0.0),
                },
            )
            .unwrap_or_default();

        let mut graph_weights: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for t in &traversal {
            let e = graph_weights.entry(t.node.id.clone()).or_insert(0.0);
            if t.weight > *e {
                *e = t.weight;
            }
        }

        // Step 2: vector search — fetch extra candidates when filter is active
        let fetch_k = (q.top_k * 20).max(100);
        let vec_results: Vec<SearchResult> = col.search(
            q.embedding,
            SearchOptions {
                top_k: fetch_k,
                metric: DistanceMetric::Cosine,
                filter: q.filter.clone(),
            },
        )?;

        if vec_results.is_empty() {
            return Ok(vec![]);
        }

        // Step 3: normalize vector scores (distance → similarity)
        let max_s = vec_results.iter().map(|r| r.score).fold(f32::NEG_INFINITY, f32::max);
        let min_s = vec_results.iter().map(|r| r.score).fold(f32::INFINITY, f32::min);
        let range = (max_s - min_s).max(1e-6);

        // Step 4: blend and rank
        let mut blended: Vec<HybridResult> = vec_results
            .into_iter()
            .map(|r| {
                let vec_sim = 1.0 - ((r.score - min_s) / range) as f64;
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

        blended.sort_by(|a, b| {
            b.rank_score
                .partial_cmp(&a.rank_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        blended.truncate(q.top_k);
        Ok(blended)
    }
}
