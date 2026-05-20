use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::error::{AgentDbError, Result};

/// Distance metric for vector search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

/// Compute cosine distance between two vectors
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (norm_a * norm_b))
}

/// Compute euclidean distance between two vectors
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Compute dot product distance (1 - dot for similarity to distance)
pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    1.0 - dot
}

pub fn distance(a: &[f32], b: &[f32], metric: &DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Cosine => cosine_distance(a, b),
        DistanceMetric::Euclidean => euclidean_distance(a, b),
        DistanceMetric::DotProduct => dot_product_distance(a, b),
    }
}

/// Pure Rust HNSW (Hierarchical Navigable Small World) index
#[derive(Serialize, Deserialize)]
pub struct HnswIndex {
    m: usize,
    ef_construction: usize,
    vectors: Vec<Vec<f32>>,
    id_map: HashMap<String, usize>,
    rev_map: Vec<String>,
    // layers[level][node] = list of neighbor indices
    layers: Vec<HashMap<usize, Vec<usize>>>,
    entry_point: Option<usize>,
    metric: DistanceMetric,
}

impl HnswIndex {
    pub fn new(m: usize, ef_construction: usize, metric: DistanceMetric) -> Self {
        Self {
            m,
            ef_construction,
            vectors: Vec::new(),
            id_map: HashMap::new(),
            rev_map: Vec::new(),
            layers: Vec::new(),
            entry_point: None,
            metric,
        }
    }

    fn random_level(&self) -> usize {
        let mut level = 0;
        let m_l = 1.0 / (self.m as f32).ln();
        loop {
            if rand::random::<f32>() > (-rand::random::<f32>() * m_l).exp() || level > 16 {
                break;
            }
            level += 1;
        }
        level
    }

    pub fn insert(&mut self, id: &str, vector: Vec<f32>) {
        if self.id_map.contains_key(id) {
            // Update existing vector
            let idx = self.id_map[id];
            self.vectors[idx] = vector;
            return;
        }

        let idx = self.vectors.len();
        self.vectors.push(vector);
        self.id_map.insert(id.to_string(), idx);
        self.rev_map.push(id.to_string());

        let level = self.random_level();

        // Ensure layers exist up to this level
        while self.layers.len() <= level {
            self.layers.push(HashMap::new());
        }

        // Add node to all levels up to its level
        for l in 0..=level {
            self.layers[l].insert(idx, Vec::new());
        }

        if let Some(ep) = self.entry_point {
            // Simple greedy search for neighbors at each level
            for l in (0..=level.min(self.layers.len() - 1)).rev() {
                let neighbors = self.search_layer(idx, ep, self.m, l);
                if let Some(layer) = self.layers.get_mut(l) {
                    if let Some(node_neighbors) = layer.get_mut(&idx) {
                        *node_neighbors = neighbors.iter().map(|(i, _)| *i).collect();
                    }
                    // Add back-links
                    for (neighbor_idx, _) in &neighbors {
                        if let Some(n_neighbors) = layer.get_mut(neighbor_idx) {
                            n_neighbors.push(idx);
                            if n_neighbors.len() > self.m * 2 {
                                n_neighbors.truncate(self.m * 2);
                            }
                        }
                    }
                }
            }
        }

        // Update entry point if this node has higher level
        if self.entry_point.is_none()
            || level >= self.layers.len().saturating_sub(1)
        {
            self.entry_point = Some(idx);
        }
    }

    fn search_layer(
        &self,
        query_idx: usize,
        entry: usize,
        k: usize,
        level: usize,
    ) -> Vec<(usize, f32)> {
        let query = &self.vectors[query_idx];
        let mut visited = std::collections::HashSet::new();
        let mut candidates = std::collections::BinaryHeap::new();
        let mut result = std::collections::BinaryHeap::new();

        let d = distance(query, &self.vectors[entry], &self.metric);
        candidates.push(std::cmp::Reverse((ordered_float(d), entry)));
        result.push((ordered_float(d), entry));
        visited.insert(entry);

        while let Some(std::cmp::Reverse((dist, curr))) = candidates.pop() {
            if let Some((worst, _)) = result.iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()) {
                if dist > *worst && result.len() >= k {
                    break;
                }
            }

            if let Some(layer) = self.layers.get(level) {
                if let Some(neighbors) = layer.get(&curr) {
                    for &neighbor in neighbors {
                        if visited.insert(neighbor) {
                            let nd = distance(query, &self.vectors[neighbor], &self.metric);
                            candidates.push(std::cmp::Reverse((ordered_float(nd), neighbor)));
                            result.push((ordered_float(nd), neighbor));
                            if result.len() > k * 2 {
                                // Keep only best k*2
                                let mut v: Vec<_> = result.drain().collect();
                                v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                                v.truncate(k * 2);
                                result.extend(v);
                            }
                        }
                    }
                }
            }
        }

        let mut results: Vec<(usize, f32)> = result
            .into_iter()
            .map(|(d, i)| (i, d))
            .collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        results.truncate(k);
        results
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let Some(mut ep) = self.entry_point else {
            return vec![];
        };

        let num_layers = self.layers.len();
        if num_layers == 0 {
            return vec![];
        }

        // Greedy search from top layer down
        for l in (1..num_layers).rev() {
            if let Some(layer) = self.layers.get(l) {
                if layer.contains_key(&ep) {
                    // Find closest neighbor at this layer
                    if let Some(neighbors) = layer.get(&ep) {
                        for &n in neighbors {
                            let d_n = distance(query, &self.vectors[n], &self.metric);
                            let d_ep = distance(query, &self.vectors[ep], &self.metric);
                            if d_n < d_ep {
                                ep = n;
                            }
                        }
                    }
                }
            }
        }

        // Search bottom layer with ef
        let ef = k.max(self.ef_construction);
        let mut visited = std::collections::HashSet::new();
        let mut candidates: Vec<(f32, usize)> = Vec::new();

        let d_ep = distance(query, &self.vectors[ep], &self.metric);
        candidates.push((d_ep, ep));
        visited.insert(ep);

        let mut i = 0;
        while i < candidates.len() {
            let (_, curr) = candidates[i];
            i += 1;

            if let Some(layer) = self.layers.get(0) {
                if let Some(neighbors) = layer.get(&curr) {
                    for &n in neighbors {
                        if visited.insert(n) {
                            let d = distance(query, &self.vectors[n], &self.metric);
                            candidates.push((d, n));
                        }
                    }
                }
            }

            if candidates.len() > ef * 4 {
                candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                candidates.truncate(ef * 2);
                i = i.min(candidates.len());
            }
        }

        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        candidates.truncate(k);

        candidates
            .into_iter()
            .map(|(d, idx)| (self.rev_map[idx].clone(), d))
            .collect()
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| AgentDbError::Serialization(e.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| AgentDbError::Serialization(e.to_string()))
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

fn ordered_float(f: f32) -> u32 {
    f.to_bits()
}
