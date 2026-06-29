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
use pyo3::types::{PyDict, PyList, PyModule};
use serde_json::Value;
use std::sync::{Arc, Mutex};

// ── Helpers ───────────────────────────────────────────────────────────

fn to_py_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn pyobj_to_json(obj: &Bound<'_, pyo3::PyAny>) -> PyResult<Option<Value>> {
    if obj.is_none() {
        return Ok(None);
    }
    let json_mod = obj.py().import("json")?;
    let s: String = json_mod.call_method1("dumps", (obj,))?.extract()?;
    Ok(serde_json::from_str(&s).ok())
}

fn json_to_pyobj(py: Python, val: &Value) -> PyResult<Py<PyAny>> {
    let json_mod = py.import("json")?;
    let s = val.to_string();
    Ok(json_mod.call_method1("loads", (s,))?.into_pyobject(py)?.into_any().unbind())
}

// ── SearchResult ──────────────────────────────────────────────────────

#[pyclass]
#[derive(Clone)]
pub struct SearchResult {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub score: f32,
    metadata_raw: Option<Value>,
}

#[pymethods]
impl SearchResult {
    #[getter]
    fn metadata(&self, py: Python) -> PyResult<Py<PyAny>> {
        match &self.metadata_raw {
            Some(v) => json_to_pyobj(py, v),
            None => Ok(py.None()),
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
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub snippet: String,
    #[pyo3(get)]
    pub rank: f64,
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
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub rank_score: f64,
    #[pyo3(get)]
    pub vector_score: f32,
    #[pyo3(get)]
    pub graph_weight: f64,
}

#[pymethods]
impl HybridResult {
    fn __repr__(&self) -> String {
        format!(
            "HybridResult(id={:?}, rank={:.4})",
            self.id, self.rank_score
        )
    }
}

// ── Collection ────────────────────────────────────────────────────────

#[pyclass]
pub struct Collection {
    inner: agentdb::Collection,
}

#[pymethods]
impl Collection {
    fn upsert(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<()> {
        let meta = metadata
            .as_ref()
            .map(pyobj_to_json)
            .transpose()?
            .flatten();
        self.inner
            .upsert(VectorEntry {
                id,
                vector,
                metadata: meta,
            })
            .map_err(to_py_err)
    }

    fn upsert_batch(&self, entries: &Bound<'_, PyList>) -> PyResult<usize> {
        let mut batch = Vec::with_capacity(entries.len());
        for item in entries.iter() {
            let d: Bound<'_, PyDict> = item.cast_into()?;
            let id: String = d
                .get_item("id")?
                .ok_or_else(|| to_py_err("missing 'id'"))?
                .extract()?;
            let vector: Vec<f32> = d
                .get_item("vector")?
                .ok_or_else(|| to_py_err("missing 'vector'"))?
                .extract()?;
            let meta = d
                .get_item("metadata")?
                .map(|m| pyobj_to_json(&m))
                .transpose()?
                .flatten();
            batch.push(BatchEntry {
                id,
                vector,
                metadata: meta,
            });
        }
        self.inner.upsert_batch(batch).map_err(to_py_err)
    }

    #[pyo3(signature = (query, top_k=10, filter=None))]
    fn search(
        &self,
        py: Python,
        query: Vec<f32>,
        top_k: usize,
        filter: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let f = filter.as_ref().map(pyobj_to_json).transpose()?.flatten();
        let results = self
            .inner
            .search(
                &query,
                SearchOptions {
                    top_k,
                    metric: DistanceMetric::Cosine,
                    filter: f,
                },
            )
            .map_err(to_py_err)?;
        results
            .into_iter()
            .map(|r| {
                let obj = SearchResult {
                    id: r.id,
                    score: r.score,
                    metadata_raw: r.metadata,
                };
                Ok(Py::new(py, obj)?.into_pyobject(py)?.into_any().unbind())
            })
            .collect()
    }

    fn count(&self) -> PyResult<i64> {
        self.inner.count().map_err(to_py_err)
    }

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
    #[staticmethod]
    fn open(path: &str) -> PyResult<Self> {
        RustDB::open(path)
            .map(|db| AgentDB {
                db: Arc::new(Mutex::new(db)),
            })
            .map_err(to_py_err)
    }

    fn execute(&self, sql: &str) -> PyResult<usize> {
        self.db.lock().unwrap().execute(sql).map_err(to_py_err)
    }

    fn query(&self, py: Python, sql: &str) -> PyResult<Vec<Py<PyAny>>> {
        let rows = self.db.lock().unwrap().query_json(sql).map_err(to_py_err)?;
        rows.iter().map(|v| json_to_pyobj(py, v)).collect()
    }

    fn collection(&self, name: &str, dim: usize) -> PyResult<Collection> {
        let db = self.db.lock().unwrap();
        db.vectors()
            .collection(name, dim)
            .map(|inner| Collection { inner })
            .map_err(to_py_err)
    }

    #[pyo3(signature = (id, kind, data=None))]
    fn add_node(&self, id: &str, kind: &str, data: Option<Bound<'_, pyo3::PyAny>>) -> PyResult<()> {
        let d = data.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .memory()
            .add_node(id, kind, d)
            .map_err(to_py_err)
    }

    fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> PyResult<()> {
        self.db
            .lock()
            .unwrap()
            .memory()
            .add_edge(src, dst, relation, weight)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (node_id, max_depth=2, min_weight=0.0))]
    fn neighbors(
        &self,
        py: Python,
        node_id: &str,
        max_depth: usize,
        min_weight: f64,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let opts = TraversalOptions {
            relation: None,
            max_depth,
            min_weight: Some(min_weight),
        };
        let results = self
            .db
            .lock()
            .unwrap()
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

    fn fts_index(
        &self,
        collection: &str,
        id: &str,
        collection_id: &str,
        text: &str,
    ) -> PyResult<()> {
        self.db
            .lock()
            .unwrap()
            .fts()
            .index_text(collection, id, collection_id, text)
            .map_err(to_py_err)
    }

    fn fts_search(
        &self,
        py: Python,
        collection: &str,
        query: &str,
        top_k: usize,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let results = self
            .db
            .lock()
            .unwrap()
            .fts()
            .search(collection, query, top_k)
            .map_err(to_py_err)?;
        results
            .into_iter()
            .map(|r| {
                let obj = FtsResult {
                    id: r.id,
                    snippet: r.snippet,
                    rank: r.rank,
                };
                Ok(Py::new(py, obj)?.into_pyobject(py)?.into_any().unbind())
            })
            .collect()
    }

    #[pyo3(signature = (anchor_node, embedding, collection, graph_depth=2, top_k=10, alpha=0.6, filter=None))]
    fn hybrid_query(
        &self,
        py: Python,
        anchor_node: &str,
        embedding: Vec<f32>,
        collection: &str,
        graph_depth: usize,
        top_k: usize,
        alpha: f64,
        filter: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let filter_val = filter.as_ref().map(pyobj_to_json).transpose()?.flatten();
        let db = self.db.lock().unwrap();
        let q = HybridQuery {
            anchor_node,
            embedding: &embedding,
            collection,
            graph_depth,
            top_k,
            alpha,
            filter: filter_val,
        };
        db.hybrid_query(q)
            .map_err(to_py_err)?
            .into_iter()
            .map(|r| {
                let obj = HybridResult {
                    id: r.id,
                    rank_score: r.rank_score,
                    vector_score: r.vector_score,
                    graph_weight: r.graph_weight,
                };
                Ok(Py::new(py, obj)?.into_pyobject(py)?.into_any().unbind())
            })
            .collect()
    }

    fn stats(&self, py: Python) -> PyResult<Py<PyAny>> {
        let s = self.db.lock().unwrap().stats().map_err(to_py_err)?;
        let v = serde_json::json!({
            "collections":    s.collections,
            "vectors":        s.vectors,
            "nodes":          s.nodes,
            "edges":          s.edges,
            "conversations":  s.conversations,
            "messages":       s.messages,
            "workflows":      s.workflows,
            "workflow_steps": s.workflow_steps,
            "traces":         s.traces
        });
        json_to_pyobj(py, &v)
    }

    // ── Conversations ─────────────────────────────────────────────────

    #[pyo3(signature = (id, title=None, metadata=None))]
    fn create_conversation(
        &self,
        id: &str,
        title: Option<&str>,
        metadata: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<()> {
        let meta = metadata.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .conversations()
            .create_conversation(id, title, meta)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (conversation_id, role, content, metadata=None))]
    fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        metadata: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<String> {
        let meta = metadata.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .conversations()
            .add_message(conversation_id, role, content, meta)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (conversation_id, limit=None))]
    fn get_messages(
        &self,
        py: Python,
        conversation_id: &str,
        limit: Option<usize>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let msgs = self
            .db
            .lock()
            .unwrap()
            .conversations()
            .get_messages(conversation_id, limit)
            .map_err(to_py_err)?;
        msgs.iter()
            .map(|m| {
                let v = serde_json::json!({
                    "id": m.id,
                    "conversation_id": m.conversation_id,
                    "role": m.role,
                    "content": m.content,
                    "metadata": m.metadata,
                    "created_at": m.created_at
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }

    fn list_conversations(&self, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        let convos = self
            .db
            .lock()
            .unwrap()
            .conversations()
            .list_conversations()
            .map_err(to_py_err)?;
        convos
            .iter()
            .map(|c| {
                let v = serde_json::json!({
                    "id": c.id,
                    "title": c.title,
                    "metadata": c.metadata,
                    "created_at": c.created_at,
                    "updated_at": c.updated_at
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }

    fn delete_conversation(&self, id: &str) -> PyResult<()> {
        self.db
            .lock()
            .unwrap()
            .conversations()
            .delete_conversation(id)
            .map_err(to_py_err)
    }

    // ── Workflows ─────────────────────────────────────────────────────

    #[pyo3(signature = (id, name, input=None))]
    fn create_workflow(
        &self,
        id: &str,
        name: &str,
        input: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<()> {
        let inp = input.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .workflows()
            .create_workflow(id, name, inp, None)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (workflow_id, name, input=None))]
    fn add_workflow_step(
        &self,
        workflow_id: &str,
        name: &str,
        input: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<String> {
        let inp = input.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .workflows()
            .add_step(workflow_id, name, inp)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (step_id, status, output=None, error=None))]
    fn update_workflow_step(
        &self,
        step_id: &str,
        status: &str,
        output: Option<Bound<'_, pyo3::PyAny>>,
        error: Option<&str>,
    ) -> PyResult<()> {
        let out = output.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .workflows()
            .update_step(step_id, status, out, error)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (id, output=None))]
    fn complete_workflow(
        &self,
        id: &str,
        output: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<()> {
        let out = output.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .workflows()
            .complete_workflow(id, out)
            .map_err(to_py_err)
    }

    #[pyo3(signature = (id, error=None))]
    fn fail_workflow(&self, id: &str, error: Option<&str>) -> PyResult<()> {
        self.db
            .lock()
            .unwrap()
            .workflows()
            .fail_workflow(id, error)
            .map_err(to_py_err)
    }

    fn get_workflow(&self, py: Python, id: &str) -> PyResult<Py<PyAny>> {
        let w = self
            .db
            .lock()
            .unwrap()
            .workflows()
            .get_workflow(id)
            .map_err(to_py_err)?;
        let steps: Vec<Value> = w
            .steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "step_index": s.step_index,
                    "name": s.name,
                    "status": s.status,
                    "input": s.input,
                    "output": s.output,
                    "error": s.error,
                    "started_at": s.started_at,
                    "completed_at": s.completed_at
                })
            })
            .collect();
        let v = serde_json::json!({
            "id": w.id,
            "name": w.name,
            "status": w.status,
            "input": w.input,
            "output": w.output,
            "metadata": w.metadata,
            "created_at": w.created_at,
            "updated_at": w.updated_at,
            "steps": steps
        });
        json_to_pyobj(py, &v)
    }

    #[pyo3(signature = (status=None))]
    fn list_workflows(&self, py: Python, status: Option<&str>) -> PyResult<Vec<Py<PyAny>>> {
        let workflows = self
            .db
            .lock()
            .unwrap()
            .workflows()
            .list_workflows(status)
            .map_err(to_py_err)?;
        workflows
            .iter()
            .map(|w| {
                let v = serde_json::json!({
                    "id": w.id,
                    "name": w.name,
                    "status": w.status,
                    "created_at": w.created_at,
                    "updated_at": w.updated_at
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }

    // ── Traces ────────────────────────────────────────────────────────

    #[pyo3(signature = (trace_type, content, session_id=None, parent_id=None, metadata=None))]
    fn add_trace(
        &self,
        trace_type: &str,
        content: &str,
        session_id: Option<&str>,
        parent_id: Option<&str>,
        metadata: Option<Bound<'_, pyo3::PyAny>>,
    ) -> PyResult<String> {
        let meta = metadata.as_ref().map(pyobj_to_json).transpose()?.flatten();
        self.db
            .lock()
            .unwrap()
            .traces()
            .add_trace(session_id, parent_id, trace_type, content, meta)
            .map_err(to_py_err)
    }

    fn get_traces(&self, py: Python, session_id: &str) -> PyResult<Vec<Py<PyAny>>> {
        let traces = self
            .db
            .lock()
            .unwrap()
            .traces()
            .get_traces(session_id)
            .map_err(to_py_err)?;
        traces
            .iter()
            .map(|t| {
                let v = serde_json::json!({
                    "id": t.id,
                    "session_id": t.session_id,
                    "parent_id": t.parent_id,
                    "trace_type": t.trace_type,
                    "content": t.content,
                    "metadata": t.metadata,
                    "created_at": t.created_at
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }

    fn get_trace_tree(&self, py: Python, root_id: &str) -> PyResult<Vec<Py<PyAny>>> {
        let traces = self
            .db
            .lock()
            .unwrap()
            .traces()
            .get_trace_tree(root_id)
            .map_err(to_py_err)?;
        traces
            .iter()
            .map(|t| {
                let v = serde_json::json!({
                    "id": t.id,
                    "session_id": t.session_id,
                    "parent_id": t.parent_id,
                    "trace_type": t.trace_type,
                    "content": t.content,
                    "metadata": t.metadata,
                    "created_at": t.created_at
                });
                json_to_pyobj(py, &v)
            })
            .collect()
    }
}

// ── Module registration ───────────────────────────────────────────────

#[pymodule]
fn _agentdb(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<AgentDB>()?;
    m.add_class::<Collection>()?;
    m.add_class::<SearchResult>()?;
    m.add_class::<FtsResult>()?;
    m.add_class::<HybridResult>()?;
    Ok(())
}
