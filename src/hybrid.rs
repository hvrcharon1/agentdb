use crate::error::Result;
use crate::memory::MemoryGraph;
use crate::memory::TraversalOptions;
use crate::vectors::collection::{Collection, SearchOptions, SearchResult};
use crate::vectors::hnsw::DistanceMetric;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A single result from a hybrid graph + vector query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HybridResult {
    /// ID of the matched entity.
    pub id: String,
    /// Raw cosine distance from the vector ANN search (lower = more similar).
    pub vector_score: f32,
    /// Maximum edge weight along any graph path from the anchor to this node.
    /// `0.0` if the node is not reachable from the anchor.
    pub graph_weight: f64,
    /// Final blended rank score: `alpha × vec_similarity + (1 − alpha) × graph_weight`.
    /// Higher is better.
    pub rank_score: f64,
    /// Metadata stored alongside the vector, if any.
    pub metadata: Option<Value>,
}

/// Parameters for a hybrid graph + vector query.
pub struct HybridQuery<'a> {
    /// The memory-graph node to start traversal from.
    pub anchor_node: &'a str,
    /// Query embedding to rank against the vector collection.
    pub embedding: &'a [f32],
    /// Name of the vector collection to search.
    pub collection: &'a str,
    /// Maximum graph traversal depth from `anchor_node`.
    pub graph_depth: usize,
    /// Maximum number of results to return after blending.
    pub top_k: usize,
    /// Interpolation factor between vector similarity and graph proximity.
    /// `0.0` = pure graph weight, `1.0` = pure vector similarity.
    pub alpha: f64,
    /// Optional metadata filter applied before vector scoring.
    pub filter: Option<Value>,
}

/// Executes hybrid graph + vector queries.
pub struct HybridStore {
    conn: Arc<Mutex<Connection>>,
}

impl HybridStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Run a hybrid graph + vector query.
    ///
    /// The algorithm proceeds in three stages:
    ///
    /// 1. **Graph traversal** — walks the memory graph from `q.anchor_node` up to
    ///    `q.graph_depth` hops, recording the maximum edge weight seen for each
    ///    reachable node.
    /// 2. **Vector search** — retrieves the top `q.top_k × 20` approximate nearest
    ///    neighbours from the named collection.
    /// 3. **Score blending** — for each candidate, computes
    ///    `rank = q.alpha × vec_similarity + (1 − q.alpha) × graph_weight`,
    ///    then returns the top `q.top_k` results sorted by rank descending.
    pub fn query(&self, q: HybridQuery, col: &Collection) -> Result<Vec<HybridResult>> {
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

        let mut graph_weights: HashMap<String, f64> = HashMap::new();
        for t in &traversal {
            let e = graph_weights.entry(t.node.id.clone()).or_insert(0.0);
            if t.weight > *e {
                *e = t.weight;
            }
        }

        // Step 2: vector search
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

        // Step 3: normalize vector scores (distance -> similarity)
        let max_s = vec_results
            .iter()
            .map(|r| r.score)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_s = vec_results
            .iter()
            .map(|r| r.score)
            .fold(f32::INFINITY, f32::min);
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
