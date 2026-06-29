use crate::error::{AgentDbError, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Distance metric used for vector similarity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (norm_a * norm_b))
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn dot_product_distance(a: &[f32], b: &[f32]) -> f32 {
    // Raw dot product (no normalisation). A higher dot product means closer,
    // so we negate to convert similarity into a distance.
    // Callers that want cosine behaviour should use DistanceMetric::Cosine.
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    -dot
}

pub fn dist(a: &[f32], b: &[f32], metric: &DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Cosine => cosine_distance(a, b),
        DistanceMetric::Euclidean => euclidean_distance(a, b),
        DistanceMetric::DotProduct => dot_product_distance(a, b),
    }
}

/// Pure-Rust HNSW approximate nearest-neighbour index.
#[derive(Serialize, Deserialize)]
pub struct HnswIndex {
    m: usize,
    ef_construction: usize,
    vectors: Vec<Vec<f32>>,
    id_map: HashMap<String, usize>,
    rev_map: Vec<String>,
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
        let mut rng = rand::thread_rng();
        let m_l = 1.0 / (self.m as f64).ln();
        let mut level = 0usize;
        while level < 16 && rng.gen::<f64>() < (-1.0_f64 / m_l).exp() {
            level += 1;
        }
        level
    }

    pub fn insert(&mut self, id: &str, vector: Vec<f32>) {
        if let Some(&idx) = self.id_map.get(id) {
            self.vectors[idx] = vector;
            return;
        }
        let idx = self.vectors.len();
        self.vectors.push(vector);
        self.id_map.insert(id.to_string(), idx);
        self.rev_map.push(id.to_string());

        let level = self.random_level();
        while self.layers.len() <= level {
            self.layers.push(HashMap::new());
        }
        for l in 0..=level {
            self.layers[l].insert(idx, Vec::new());
        }

        if let Some(ep) = self.entry_point {
            let max_l = level.min(self.layers.len().saturating_sub(1));
            for l in (0..=max_l).rev() {
                let neighbours = self.search_layer_for(idx, ep, self.m, l);
                if let Some(layer) = self.layers.get_mut(l) {
                    if let Some(nn) = layer.get_mut(&idx) {
                        *nn = neighbours.iter().map(|&(i, _)| i).collect();
                    }
                    for &(ni, _) in &neighbours {
                        if let Some(nlist) = layer.get_mut(&ni) {
                            nlist.push(idx);
                            if nlist.len() > self.m * 2 {
                                nlist.truncate(self.m * 2);
                            }
                        }
                    }
                }
            }
        }

        if self.entry_point.is_none() || level >= self.layers.len().saturating_sub(1) {
            self.entry_point = Some(idx);
        }
    }

    fn search_layer_for(
        &self,
        query_idx: usize,
        entry: usize,
        k: usize,
        level: usize,
    ) -> Vec<(usize, f32)> {
        let query = self.vectors[query_idx].clone();
        self.search_layer_vec(&query, entry, k, level)
    }

    fn search_layer_vec(
        &self,
        query: &[f32],
        entry: usize,
        k: usize,
        level: usize,
    ) -> Vec<(usize, f32)> {
        let mut visited: HashSet<usize> = HashSet::new();
        let mut candidates: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        let mut result: BinaryHeap<(u32, usize)> = BinaryHeap::new();

        let d0 = dist(query, &self.vectors[entry], &self.metric);
        candidates.push(Reverse((d0.to_bits(), entry)));
        result.push((d0.to_bits(), entry));
        visited.insert(entry);

        while let Some(Reverse((d_bits, curr))) = candidates.pop() {
            if let Some(&(worst_bits, _)) = result.peek() {
                if d_bits > worst_bits && result.len() >= k {
                    break;
                }
            }
            if let Some(layer) = self.layers.get(level) {
                if let Some(neighbours) = layer.get(&curr) {
                    for &nb in neighbours {
                        if visited.insert(nb) {
                            let nd = dist(query, &self.vectors[nb], &self.metric);
                            let nd_bits = nd.to_bits();
                            candidates.push(Reverse((nd_bits, nb)));
                            result.push((nd_bits, nb));
                            while result.len() > k * 2 {
                                result.pop();
                            }
                        }
                    }
                }
            }
        }

        let mut out: Vec<(usize, f32)> = result
            .into_iter()
            .map(|(bits, i)| (i, f32::from_bits(bits)))
            .collect();
        out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(k);
        out
    }

    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut ep = match self.entry_point {
            Some(e) => e,
            None => return vec![],
        };
        let num_layers = self.layers.len();
        if num_layers == 0 {
            return vec![];
        }

        for l in (1..num_layers).rev() {
            let mut improved = true;
            while improved {
                improved = false;
                if let Some(layer) = self.layers.get(l) {
                    if let Some(neighbours) = layer.get(&ep) {
                        let d_ep = dist(query, &self.vectors[ep], &self.metric);
                        for &nb in neighbours {
                            let d_nb = dist(query, &self.vectors[nb], &self.metric);
                            if d_nb < d_ep {
                                ep = nb;
                                improved = true;
                                break;
                            }
                        }
                    }
                }
            }
        }

        let ef = k.max(self.ef_construction);
        let raw = self.search_layer_vec(query, ep, ef, 0);

        raw.into_iter()
            .take(k)
            .map(|(idx, d)| (self.rev_map[idx].clone(), d))
            .collect()
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(|e| AgentDbError::Serialization(e.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::serde::decode_from_slice(bytes, bincode::config::standard())
            .map(|(val, _)| val)
            .map_err(|e| AgentDbError::Serialization(e.to_string()))
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}
