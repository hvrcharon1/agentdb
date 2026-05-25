//! PyO3 Python bindings for AgentDB.
//!
//! Build with maturin:
//! ```bash
//! cd python
//! pip install maturin
//! maturin develop          # editable install into current venv
//! maturin build --release  # produce a wheel in target/wheels/
//! ```
//!
//! Publish to PyPI:
//! ```bash
//! maturin publish
//! ```

use agentdb::{
    AgentDB as RustDB, BatchEntry, DistanceMetric, HybridQuery, SearchOptions, TraversalOptions,
    VectorEntry,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;
use std::sync::{Arc, Mutex};

// ── Helpers ───────────────────────────────────────────────────────────

fn to_py_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn pyobj_to_json(obj: &PyAny) -> PyResult<Option<Value>> {
    if obj.is_none() {
        return Ok(None);
    }
    let json_mod = obj.py().import("json")?;
    let s: String = json_mod.call_method1("dumps", (obj,))?.extract()?;
    Ok(serde_json::from_str(&s).ok())
}

fn json_to_pyobj(py: Python, val: &Value) -> PyResult<PyObject> {
    let json_mod = py.import("json")?;
    let s = val.to_string();
    Ok(json_mod.call_method1("loads", (s,))?.into_py(py))
}

// ── SearchResult ──────────────────────────────────────────────────────

#[pyclass]
#[derive(Clone)]
pub struct SearchResult {
    #[pyo3(get)] pub id:       String,
    #[pyo3(get)] pub score:    f32,
    metadata_raw: Option<Value>,
}

#[pymethods]
impl SearchResult {
    #[getter]
    fn metadata(&self, py: Python) -> PyResult<PyObject> {
        match &self.metadata_raw {
            Some(v) => json_to_pyobj(py, v),
            None    => Ok(py.None()),
        }
    }
    fn __repr__(&self) -> String {
        format!("SearchResult(id={:?}, score={:.4})", self.id, self.score)
    }
}

// ── FtsResult ─────────────────────────────────────────────────────────

#[pyclass]
#[derive(Clone)]
pub struct FtsResult {
    #[pyo3(get)] pub id:      String,
    #[pyo3(get)] pub snippet: String,
    #[pyo3(get)] pub rank:    f64,
}

#[pymethods]
impl FtsResult {
    fn __repr__(&self) -> String {
        format!("FtsResult(id={:?}, rank={:.4})", self.id, self.rank)
    }
}

// ── HybridResult ──────────────────────────────────────────────────────

#[pyclass]
#[derive(Clone)]
pub struct HybridResult {
    #[pyo3(get)] pub id:           String,
    #[pyo3(get)] pub rank_score:   f64,
    #[pyo3(get)] pub vector_score: f32,
    #[pyo3(get)] pub graph_weight: f64,
}

#[pymethods]
impl HybridResult {
    fn __repr__(&self) -> String {
        format!("HybridResult(id={:?}, rank={:.4})", self.id, self.rank_score)
    }
}

// ── Collection ────────────────────────────────────────────────────────

#[pyclass]
pub struct Collection {
    inner: agentdb::Collection,
}

#[pymethods]
impl Collection {
    /// Upsert a single vector.
    ///
    /// :param id: Unique string identifier.
    /// :param vector: List[float] or numpy array of floats.
    /// :param metadata: Optional dict of metadata.
    fn upsert(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: Option<&PyAny>,
    ) -> PyResult<()> {
        let meta = metadata.map(pyobj_to_json).transpose()?.flatten();
        self.inner
            .upsert(VectorEntry { id, vector, metadata: meta })
            .map_err(to_py_err)
    }

    /// Upsert multiple vectors in a single transaction.
    ///
    /// :param entries: List of dicts with keys 'id', 'vector', and optional 'metadata'.
    fn upsert_batch(&self, entries: &PyList) -> PyResult<usize> {
        let mut batch = Vec::with_capacity(entries.len());
        for item in entries.iter() {
            let d: &PyDict = item.downcast()?;
            let id: String     = d.get_item("id")?.ok_or_else(|| to_py_err("missing 'id'"))?.extract()?;
            let vector: Vec<f32> = d.get_item("vector")?.ok_or_else(|| to_py_err("missing 'vector'"))?.extract()?;
            let meta = d.get_item("metadata")?
                .map(|m| pyobj_to_json(m))
                .transpose()?
                .flatten();
            batch.push(BatchEntry { id, vector, metadata: meta });
        }
        self.inner.upsert_batch(batch).map_err(to_py_err)
    }

    /// Approximate nearest-neighbor search.
    ///
    /// :param query: List[float] or numpy array.
    /// :param top_k: Maximum results to return.
    /// :param filter: Optional metadata filter dict (MongoDB-style operators).
    /// :returns: List of SearchResult objects.
    #[pyo3(signature = (query, top_k=10, filter=None))]
    fn search(
        &self,
        py: Python,
        query: Vec<f32>,
        top_k: usize,
        filter: Option<&PyAny>,
    ) -> PyResult<Vec<PyObject>> {
        let f = filter.map(pyobj_to_json).transpose()?.flatten();
        let results = self.inner
            .search(&query, SearchOptions { top_k, metric: DistanceMetric::Cosine, filter: f })
            .map_err(to_py_err)?;
        results
            .into_iter()
            .map(|r| {
                let obj = SearchResult { id: r.id, score: r.score, metadata_raw: r.metadata };
                Ok(Py::new(py, obj)?.into_py(py))
            })
            .collect()
    }

    /// Number of vectors in this collection.
    fn count(&self) -> PyResult<i64> {
        self.inner.count().map_err(to_py_err)
    }

    /// Rebuild the HNSW index.
    fn reindex(&self) -> PyResult<()> {
        self.inner.reindex().map_err(to_py_err)
    }
}

// ── AgentDB ───────────────────────────────────────────────────────────

#[pyclass]
pub struct AgentDB {
    db: Arc<Mutex<RustDB>>,
}

#[pymethods]
impl AgentDB {
    /// Open or create an AgentDB database.
    ///
    /// :param path: File path, or ':memory:' for an in-memory database.
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        RustDB::open(path)
            .map(|db| AgentDB { db: Arc::new(Mutex::new(db)) })
            .map_err(to_py_err)
    }

    /// Execute a raw SQL statement.
    fn execute(&self, sql: &str) -> PyResult<usize> {
        self.db.lock().unwrap().execute(sql).map_err(to_py_err)
    }

    /// Query and return rows as a list of dicts.
    fn query(&self, py: Python, sql: &str) -> PyResult<Vec<PyObject>> {
        let rows = self.db.lock().unwrap().query_json(sql).map_err(to_py_err)?;
        rows.iter().map(|v| json_to_pyobj(py, v)).collect()
    }

    /// Return a Collection handle for vector operations.
    ///
    /// :param name: Collection name (created if absent).
    /// :param dim:  Embedding dimensionality.
    fn collection(&self, name: &str, dim: usize) -> PyResult<Collection> {
        let db = self.db.lock().unwrap();
        db.vectors()
            .collection(name, dim)
            .map(|inner| Collection { inner })
            .map_err(to_py_err)
    }

    /// Add or update a memory graph node.
    #[pyo3(signature = (id, kind, data=None))]
    fn add_node(&self, id: &str, kind: &str, data: Option<&PyAny>) -> PyResult<()> {
        let d = data.map(pyobj_to_json).transpose()?.flatten();
        self.db.lock().unwrap().memory().add_node(id, kind, d).map_err(to_py_err)
    }

    /// Add or update a directed edge in the memory graph.
    fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> PyResult<()> {
        self.db.lock().unwrap().memory().add_edge(src, dst, relation, weight).map_err(to_py_err)
    }

    /// Traverse the memory graph from a node.
    ///
    /// :param node_id:   Start node.
    /// :param max_depth: Maximum hops (default 2).
    /// :param min_weight: Minimum edge weight to follow (default 0.0).
    /// :returns: List of dicts with 'id', 'kind', 'depth', 'weight', 'data'.
    #[pyo3(signature = (node_id, max_depth=2, min_weight=0.0))]
    fn neighbors(
        &self,
        py: Python,
        node_id: &str,
        max_depth: usize,
        min_weight: f64,
    ) -> PyResult<Vec<PyObject>> {
        let opts = TraversalOptions { relation: None, max_depth, min_weight: Some(min_weight) };
        let results = self.db.lock().unwrap()
            .memory()
            .neighbors(node_id, opts)
            .map_err(to_py_err)?;
        results
            .iter()
            .map(|r| {
                let v = serde_json::json!({
                    "id":    r.node.id,
                    "kind":  r.node.kind,
                    "depth": r.depth,
                    "weight": r.weight,
                    "data":  r.node.data
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }

    /// Index text for full-text search.
    fn fts_index(&self, collection: &str, id: &str, collection_id: &str, text: &str) -> PyResult<()> {
        self.db.lock().unwrap().fts().index_text(collection, id, collection_id, text).map_err(to_py_err)
    }

    /// Full-text search over a collection.
    ///
    /// :returns: List of FtsResult objects.
    fn fts_search(&self, py: Python, collection: &str, query: &str, top_k: usize) -> PyResult<Vec<PyObject>> {
        let results = self.db.lock().unwrap().fts().search(collection, query, top_k).map_err(to_py_err)?;
        results
            .into_iter()
            .map(|r| {
                let obj = FtsResult { id: r.id, snippet: r.snippet, rank: r.rank };
                Ok(Py::new(py, obj)?.into_py(py))
            })
            .collect()
    }

    /// Run a hybrid graph + vector query.
    ///
    /// :param anchor_node: Graph traversal start node.
    /// :param embedding:   Query vector (List[float] or numpy array).
    /// :param collection:  Vector collection name.
    /// :param graph_depth: Max hops from anchor (default 2).
    /// :param top_k:       Results to return (default 10).
    /// :param alpha:       0.0 = pure graph, 1.0 = pure vector (default 0.6).
    /// :returns: List of HybridResult objects.
    #[pyo3(signature = (anchor_node, embedding, collection, graph_depth=2, top_k=10, alpha=0.6))]
    fn hybrid_query(
        &self,
        py: Python,
        anchor_node: &str,
        embedding: Vec<f32>,
        collection: &str,
        graph_depth: usize,
        top_k: usize,
        alpha: f64,
    ) -> PyResult<Vec<PyObject>> {
        let db = self.db.lock().unwrap();
        let q = HybridQuery {
            anchor_node,
            embedding: &embedding,
            collection,
            graph_depth,
            top_k,
            alpha,
            filter: None,
        };
        db.hybrid_query(q)
            .map_err(to_py_err)?
            .into_iter()
            .map(|r| {
                let obj = HybridResult {
                    id:           r.id,
                    rank_score:   r.rank_score,
                    vector_score: r.vector_score,
                    graph_weight: r.graph_weight,
                };
                Ok(Py::new(py, obj)?.into_py(py))
            })
            .collect()
    }

    /// Return database-wide statistics as a dict.
    fn stats(&self, py: Python) -> PyResult<PyObject> {
        let s = self.db.lock().unwrap().stats().map_err(to_py_err)?;
        let v = serde_json::json!({
            "collections": s.collections,
            "vectors":     s.vectors,
            "nodes":       s.nodes,
            "edges":       s.edges
        });
        json_to_pyobj(py, &v)
    }
}

// ── Module registration ───────────────────────────────────────────────

#[pymodule]
fn _agentdb(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<AgentDB>()?;
    m.add_class::<Collection>()?;
    m.add_class::<SearchResult>()?;
    m.add_class::<FtsResult>()?;
    m.add_class::<HybridResult>()?;
    Ok(())
}
