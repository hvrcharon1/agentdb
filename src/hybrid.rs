use crate::error::{AgentDbError, Result};
use crate::fts::FullTextStore;
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

/// Parameters for a tri-modal graph + vector + FTS query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriModalQuery {
    /// The memory-graph node to start traversal from.
    pub anchor_node: String,
    /// Query embedding to rank against the vector collection.
    pub embedding: Vec<f32>,
    /// Text query for full-text search.
    pub text_query: String,
    /// Name of the vector collection to search.
    pub collection: String,
    /// Maximum graph traversal depth from `anchor_node`.
    pub graph_depth: usize,
    /// Maximum number of results to return after blending.
    pub top_k: usize,
    /// Weight for vector similarity (must satisfy alpha + beta + gamma ≈ 1.0).
    pub alpha: f32,
    /// Weight for graph proximity.
    pub beta: f32,
    /// Weight for FTS BM25 score.
    pub gamma: f32,
    /// Optional metadata filter applied before vector scoring.
    pub filter: Option<Value>,
}

/// A single result from a tri-modal graph + vector + FTS query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TriModalResult {
    /// ID of the matched entity.
    pub id: String,
    /// Final blended rank score (higher is better).
    pub rank_score: f32,
    /// Normalized vector similarity score in [0, 1], if the item appeared in ANN results.
    pub vector_score: Option<f32>,
    /// Normalized graph proximity weight in [0, 1], if the item is reachable from the anchor.
    pub graph_weight: Option<f32>,
    /// Normalized FTS BM25 score in [0, 1], if the item appeared in FTS results.
    pub fts_rank: Option<f32>,
    /// Metadata stored alongside the vector, if any.
    pub metadata: Option<Value>,
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

    /// Run a tri-modal graph + vector + FTS query.
    ///
    /// The algorithm runs three searches and blends results:
    ///
    /// 1. **Vector ANN** — retrieves `top_k × 20` approximate nearest neighbours.
    /// 2. **Graph traversal** — walks the memory graph from `q.anchor_node` up to
    ///    `q.graph_depth` hops, recording the maximum edge weight per reachable node.
    /// 3. **FTS keyword search** — BM25 full-text search over the collection's FTS index.
    ///
    /// Each component is min-max normalised to [0, 1] within its own result set, then
    /// blended as `final_score = alpha × vec_score + beta × graph_weight + gamma × fts_score`.
    ///
    /// The weights must satisfy `alpha + beta + gamma ≈ 1.0` (tolerance ±0.01).
    pub fn tri_modal_query(
        &self,
        q: &TriModalQuery,
        col: &Collection,
    ) -> Result<Vec<TriModalResult>> {
        // Validate weights
        let weight_sum = q.alpha + q.beta + q.gamma;
        if (weight_sum - 1.0_f32).abs() > 0.01 {
            return Err(AgentDbError::InvalidArgument(format!(
                "tri_modal_query: alpha + beta + gamma must equal 1.0, got {weight_sum:.4}"
            )));
        }

        // ── Step 1: Graph traversal ────────────────────────────────────────
        let mut graph_weights: HashMap<String, f64> = HashMap::new();
        if q.beta > 0.0 {
            let graph = MemoryGraph::new(Arc::clone(&self.conn));
            let traversal = graph
                .neighbors(
                    &q.anchor_node,
                    TraversalOptions {
                        relation: None,
                        max_depth: q.graph_depth,
                        min_weight: Some(0.0),
                    },
                )
                .unwrap_or_default();
            for t in &traversal {
                let e = graph_weights.entry(t.node.id.clone()).or_insert(0.0);
                if t.weight > *e {
                    *e = t.weight;
                }
            }
        }

        // ── Step 2: Vector ANN search ──────────────────────────────────────
        let fetch_k = (q.top_k * 20).max(100);
        let vec_results: Vec<SearchResult> = if q.alpha > 0.0 && !q.embedding.is_empty() {
            col.search(
                &q.embedding,
                SearchOptions {
                    top_k: fetch_k,
                    metric: DistanceMetric::Cosine,
                    filter: q.filter.clone(),
                },
            )
            .unwrap_or_default()
        } else {
            vec![]
        };

        // ── Step 3: FTS search ─────────────────────────────────────────────
        let fts_results = if q.gamma > 0.0 && !q.text_query.is_empty() {
            let fts = FullTextStore::new(Arc::clone(&self.conn));
            fts.search(&q.collection, &q.text_query, fetch_k)
                .unwrap_or_default()
        } else {
            vec![]
        };

        // ── Collect candidate IDs from all three sources ───────────────────
        let mut candidate_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        for r in &vec_results {
            candidate_ids.insert(r.id.clone());
        }
        for r in &fts_results {
            candidate_ids.insert(r.id.clone());
        }
        for id in graph_weights.keys() {
            candidate_ids.insert(id.clone());
        }

        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        // ── Normalise vector scores ────────────────────────────────────────
        // ANN returns cosine distance (lower = more similar). Convert to similarity.
        let vec_map: HashMap<String, f32> = if !vec_results.is_empty() {
            let max_s = vec_results
                .iter()
                .map(|r| r.score)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_s = vec_results
                .iter()
                .map(|r| r.score)
                .fold(f32::INFINITY, f32::min);
            let range = (max_s - min_s).max(1e-6);
            vec_results
                .iter()
                .map(|r| {
                    let normalised = 1.0 - (r.score - min_s) / range;
                    (r.id.clone(), normalised)
                })
                .collect()
        } else {
            HashMap::new()
        };

        // ── Normalise graph weights ────────────────────────────────────────
        let graph_norm: HashMap<String, f32> = if !graph_weights.is_empty() {
            let max_g = graph_weights
                .values()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let min_g = graph_weights
                .values()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let range_g = (max_g - min_g).max(1e-9);
            graph_weights
                .iter()
                .map(|(id, &w)| {
                    let normalised = ((w - min_g) / range_g) as f32;
                    (id.clone(), normalised)
                })
                .collect()
        } else {
            HashMap::new()
        };

        // ── Normalise FTS ranks ────────────────────────────────────────────
        // BM25 scores from SQLite FTS5 are negative (more negative = better).
        // Invert and normalise so that higher is better.
        let fts_map: HashMap<String, f32> = if !fts_results.is_empty() {
            // Negate so the best result (most negative BM25) becomes the largest value.
            let negated: Vec<f64> = fts_results.iter().map(|r| -r.rank).collect();
            let max_f = negated.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let min_f = negated.iter().copied().fold(f64::INFINITY, f64::min);
            let range_f = (max_f - min_f).max(1e-9);
            fts_results
                .iter()
                .zip(negated.iter())
                .map(|(r, &neg)| {
                    let normalised = ((neg - min_f) / range_f) as f32;
                    (r.id.clone(), normalised)
                })
                .collect()
        } else {
            HashMap::new()
        };

        // ── Collect metadata from vector results ───────────────────────────
        let meta_map: HashMap<String, Option<Value>> = vec_results
            .iter()
            .map(|r| (r.id.clone(), r.metadata.clone()))
            .collect();

        // ── Blend scores ───────────────────────────────────────────────────
        let mut blended: Vec<TriModalResult> = candidate_ids
            .into_iter()
            .map(|id| {
                let vs = vec_map.get(&id).copied();
                let gw = graph_norm.get(&id).copied();
                let fr = fts_map.get(&id).copied();

                let rank = q.alpha * vs.unwrap_or(0.0)
                    + q.beta * gw.unwrap_or(0.0)
                    + q.gamma * fr.unwrap_or(0.0);

                let metadata = meta_map.get(&id).cloned().flatten();

                TriModalResult {
                    id,
                    rank_score: rank,
                    vector_score: vs,
                    graph_weight: gw,
                    fts_rank: fr,
                    metadata,
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
