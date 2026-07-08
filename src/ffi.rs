//! # AgentDB — C FFI
//!
//! A flat `extern "C"` API that allows any language with C interop
//! (Go via cgo, Ruby via ffi gem, Swift, Kotlin/JNI, PHP, etc.) to
//! use AgentDB without writing Rust.
//!
//! ## Memory contract
//! - Strings returned by AgentDB (`agentdb_*_json`, `agentdb_last_error`)
//!   are heap-allocated CStrings.  The caller **must** free them with
//!   `agentdb_free_string` — never with `free()` or `delete`.
//! - Input `*const c_char` pointers are borrowed for the duration of the
//!   call only; the caller owns them.
//! - `AgentDbHandle` is an opaque pointer. Close with `agentdb_close`.
//!
//! Build the shared library:
//! ```bash
//! cargo build --release --features ffi --lib
//! # Linux:   target/release/libagentdb.so
//! # macOS:   target/release/libagentdb.dylib
//! # Windows: target/release/agentdb.dll
//! ```
//!
//! Generate the C header (requires cbindgen):
//! ```bash
//! cbindgen --config cbindgen.toml --output agentdb.h
//! ```

#![allow(clippy::missing_safety_doc)]

use crate::db::AgentDB;
use crate::hybrid::{HybridQuery, TriModalQuery};
use crate::vectors::{DistanceMetric, SearchOptions, VectorEntry};
use serde_json::Value;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ── Thread-local last error ───────────────────────────────────────────

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(msg.into()));
}

fn clear_last_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Return the last error message as a UTF-8 C string, or NULL if none.
/// The returned pointer must be freed with `agentdb_free_string`.
#[no_mangle]
pub extern "C" fn agentdb_last_error() -> *mut c_char {
    LAST_ERROR.with(|e| match e.borrow().as_deref() {
        Some(msg) => CString::new(msg)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    })
}

/// Free a string previously returned by AgentDB.
///
/// # Safety
/// `ptr` must be a pointer previously returned by an `agentdb_*` function
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn agentdb_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

// ── Opaque handle ────────────────────────────────────────────────────

pub struct AgentDbHandle {
    db: AgentDB,
}

// ── Lifecycle ────────────────────────────────────────────────────────

/// Open or create an AgentDB database at `path`.
/// Use `":memory:"` for an in-memory database.
///
/// Returns an opaque handle on success, or NULL on failure.
/// Check `agentdb_last_error()` on NULL.
#[no_mangle]
pub unsafe extern "C" fn agentdb_open(path: *const c_char) -> *mut AgentDbHandle {
    clear_last_error();
    let path_str = match path.as_ref().and_then(|p| CStr::from_ptr(p).to_str().ok()) {
        Some(s) => s,
        None => {
            set_last_error("agentdb_open: invalid path string");
            return std::ptr::null_mut();
        }
    };
    match AgentDB::open(path_str) {
        Ok(db) => Box::into_raw(Box::new(AgentDbHandle { db })),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Close and free an AgentDB handle.
///
/// # Safety
/// `handle` must be a valid pointer previously returned by `agentdb_open`
/// and must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn agentdb_close(handle: *mut AgentDbHandle) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle);
    }
}

// ── SQL ──────────────────────────────────────────────────────────────

/// Execute a raw SQL statement (no parameters).
///
/// Returns the number of rows affected, or -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_execute(handle: *mut AgentDbHandle, sql: *const c_char) -> i64 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("agentdb_execute: null handle");
            return -1;
        }
    };
    let sql_str = match CStr::from_ptr(sql).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("agentdb_execute: invalid SQL string");
            return -1;
        }
    };
    match h.db.execute(sql_str) {
        Ok(n) => n as i64,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Query and return all rows as a JSON array string.
///
/// Returns a heap-allocated JSON string — free with `agentdb_free_string`.
/// Returns NULL on error; check `agentdb_last_error()`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_query_json(
    handle: *mut AgentDbHandle,
    sql: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("agentdb_query_json: null handle");
            return std::ptr::null_mut();
        }
    };
    let sql_str = match CStr::from_ptr(sql).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("agentdb_query_json: invalid SQL");
            return std::ptr::null_mut();
        }
    };
    match h.db.query_json(sql_str) {
        Ok(rows) => {
            let json = Value::Array(rows).to_string();
            CString::new(json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Query with positional parameters and return all rows as a JSON array string.
///
/// `params_json` is a JSON array of parameter values (e.g. `["alice", 42]`).
/// Returns a heap-allocated JSON string — free with `agentdb_free_string`.
/// Returns NULL on error; check `agentdb_last_error()`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_query_json_params(
    handle: *mut AgentDbHandle,
    sql: *const c_char,
    params_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("agentdb_query_json_params: null handle");
            return std::ptr::null_mut();
        }
    };
    let sql_str = match CStr::from_ptr(sql).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("agentdb_query_json_params: invalid SQL");
            return std::ptr::null_mut();
        }
    };
    let params: Vec<String> = if params_json.is_null() {
        vec![]
    } else {
        match CStr::from_ptr(params_json).to_str() {
            Ok(s) => serde_json::from_str::<Vec<serde_json::Value>>(s)
                .unwrap_or_default()
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
            Err(_) => {
                set_last_error("agentdb_query_json_params: invalid params");
                return std::ptr::null_mut();
            }
        }
    };
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    match h.db.query_json_params(sql_str, &param_refs) {
        Ok(rows) => {
            let json = Value::Array(rows).to_string();
            CString::new(json)
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Vector store ─────────────────────────────────────────────────────

/// Upsert a single vector into `collection` (created if absent).
///
/// `id`       — unique string identifier for this vector
/// `vector`   — pointer to `dim` f32 values
/// `dim`      — number of dimensions
/// `metadata` — JSON string (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_vector_upsert(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    id: *const c_char,
    vector: *const f32,
    dim: usize,
    metadata: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection name");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let vec: Vec<f32> = std::slice::from_raw_parts(vector, dim).to_vec();
    let meta: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let col = match h.db.vectors().collection(col_name, dim) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    match col.upsert(VectorEntry {
        id: id_str.to_string(),
        vector: vec,
        metadata: meta,
    }) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Search a vector collection and return results as a JSON array.
///
/// `query`      — pointer to `dim` f32 query values
/// `dim`        — number of dimensions
/// `top_k`      — maximum results to return
/// `filter_json`— MongoDB-style metadata filter JSON string (may be NULL)
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
/// Returns NULL on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_vector_search(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    query: *const f32,
    dim: usize,
    top_k: usize,
    filter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection name");
            return std::ptr::null_mut();
        }
    };
    let q: Vec<f32> = std::slice::from_raw_parts(query, dim).to_vec();
    let filter: Option<Value> = if filter_json.is_null() {
        None
    } else {
        CStr::from_ptr(filter_json)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let col = match h.db.vectors().collection(col_name, dim) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e.to_string());
            return std::ptr::null_mut();
        }
    };
    match col.search(
        &q,
        SearchOptions {
            top_k,
            metric: DistanceMetric::Cosine,
            filter,
        },
    ) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(
                    |r| serde_json::json!({ "id": r.id, "score": r.score, "metadata": r.metadata }),
                )
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Memory graph ─────────────────────────────────────────────────────

/// Add or update a node in the memory graph.
///
/// `id`       — unique node identifier
/// `kind`     — node type label (e.g. "session", "concept")
/// `data_json`— JSON metadata string (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_add_node(
    handle: *mut AgentDbHandle,
    id: *const c_char,
    kind: *const c_char,
    data_json: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let kind_str = match CStr::from_ptr(kind).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid kind");
            return -1;
        }
    };
    let data: Option<Value> = if data_json.is_null() {
        None
    } else {
        CStr::from_ptr(data_json)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h.db.memory().add_node(id_str, kind_str, data) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Add or update a directed weighted edge in the memory graph.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_add_edge(
    handle: *mut AgentDbHandle,
    src: *const c_char,
    dst: *const c_char,
    relation: *const c_char,
    weight: f64,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let src_str = match CStr::from_ptr(src).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid src");
            return -1;
        }
    };
    let dst_str = match CStr::from_ptr(dst).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid dst");
            return -1;
        }
    };
    let relation_str = match CStr::from_ptr(relation).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid relation");
            return -1;
        }
    };
    match h
        .db
        .memory()
        .add_edge(src_str, dst_str, relation_str, weight)
    {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Traverse the memory graph from `node_id` and return results as JSON.
///
/// `max_depth`  — maximum hops from the anchor node
/// `min_weight` — minimum edge weight to traverse (0.0 = all edges)
/// `relation`   — optional edge relation filter (NULL = all relations)
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_neighbors(
    handle: *mut AgentDbHandle,
    node_id: *const c_char,
    max_depth: usize,
    min_weight: f64,
    relation: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let id_str = match CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid node_id");
            return std::ptr::null_mut();
        }
    };
    let relation_str: Option<String> = if relation.is_null() {
        None
    } else {
        CStr::from_ptr(relation)
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
    let opts = crate::memory::TraversalOptions {
        relation: relation_str,
        max_depth,
        min_weight: Some(min_weight),
    };
    match h.db.memory().neighbors(id_str, opts) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.node.id,
                        "kind": r.node.kind,
                        "depth": r.depth,
                        "weight": r.weight,
                        "data": r.node.data
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Full-text search ──────────────────────────────────────────────────

/// Index a text document for full-text search.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_fts_index(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    vec_id: *const c_char,
    collection_id: *const c_char,
    text: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection");
            return -1;
        }
    };
    let vid = match CStr::from_ptr(vec_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid vec_id");
            return -1;
        }
    };
    let cid = match CStr::from_ptr(collection_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection_id");
            return -1;
        }
    };
    let txt = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid text");
            return -1;
        }
    };
    match h.db.fts().index_text(col, vid, cid, txt) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Full-text search over a collection, returning results as JSON.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_fts_search(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    query: *const c_char,
    top_k: usize,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection");
            return std::ptr::null_mut();
        }
    };
    let q = match CStr::from_ptr(query).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid query");
            return std::ptr::null_mut();
        }
    };
    match h.db.fts().search(col, q, top_k) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({ "id": r.id, "snippet": r.snippet, "rank": r.rank }))
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Hybrid query ─────────────────────────────────────────────────────

/// Run a hybrid graph + vector query and return results as JSON.
///
/// `anchor_node`  — graph traversal start node id
/// `embedding`    — pointer to `dim` f32 query values
/// `dim`          — embedding dimensions
/// `collection`   — vector collection name
/// `graph_depth`  — max hops from anchor
/// `top_k`        — results to return
/// `alpha`        — 0.0 = pure graph, 1.0 = pure vector
/// `filter_json`  — optional MongoDB-style metadata filter JSON (may be NULL)
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_hybrid_query(
    handle: *mut AgentDbHandle,
    anchor_node: *const c_char,
    embedding: *const f32,
    dim: usize,
    collection: *const c_char,
    graph_depth: usize,
    top_k: usize,
    alpha: f64,
    filter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let anchor = match CStr::from_ptr(anchor_node).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid anchor_node");
            return std::ptr::null_mut();
        }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection");
            return std::ptr::null_mut();
        }
    };
    let emb: Vec<f32> = std::slice::from_raw_parts(embedding, dim).to_vec();
    let filter: Option<serde_json::Value> = if filter_json.is_null() {
        None
    } else {
        CStr::from_ptr(filter_json)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let q = HybridQuery {
        anchor_node: anchor,
        embedding: &emb,
        collection: col,
        graph_depth,
        top_k,
        alpha,
        filter,
    };
    match h.db.hybrid_query(q) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "rank_score": r.rank_score,
                        "vector_score": r.vector_score,
                        "graph_weight": r.graph_weight
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Tri-modal query ───────────────────────────────────────────────────

/// Run a tri-modal graph + vector + FTS query and return results as JSON.
///
/// `query_json` — a JSON object with fields:
///   - `anchor_node`  (string)  — graph traversal start node id
///   - `embedding`    (array)   — f32 query vector
///   - `text_query`   (string)  — full-text search query
///   - `collection`   (string)  — vector collection name
///   - `graph_depth`  (integer) — max hops from anchor (default 3)
///   - `top_k`        (integer) — results to return (default 10)
///   - `alpha`        (number)  — vector weight (default 0.33)
///   - `beta`         (number)  — graph weight (default 0.33)
///   - `gamma`        (number)  — FTS weight (default 0.34)
///   - `filter`       (object)  — optional metadata filter
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
/// Returns NULL on error; check `agentdb_last_error()`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_tri_modal_query(
    handle: *mut AgentDbHandle,
    query_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let json_str = match CStr::from_ptr(query_json).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("agentdb_tri_modal_query: invalid query_json");
            return std::ptr::null_mut();
        }
    };
    let obj: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("agentdb_tri_modal_query: parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let anchor_node = obj["anchor_node"].as_str().unwrap_or("").to_string();
    let embedding: Vec<f32> = obj["embedding"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect()
        })
        .unwrap_or_default();
    let text_query = obj["text_query"].as_str().unwrap_or("").to_string();
    let collection = obj["collection"].as_str().unwrap_or("").to_string();
    let graph_depth = obj["graph_depth"].as_u64().unwrap_or(3) as usize;
    let top_k = obj["top_k"].as_u64().unwrap_or(10) as usize;
    let alpha = obj["alpha"].as_f64().unwrap_or(0.33) as f32;
    let beta = obj["beta"].as_f64().unwrap_or(0.33) as f32;
    let gamma = obj["gamma"].as_f64().unwrap_or(0.34) as f32;
    let filter: Option<serde_json::Value> =
        obj.get("filter")
            .cloned()
            .and_then(|v| if v.is_null() { None } else { Some(v) });

    let q = TriModalQuery {
        anchor_node,
        embedding,
        text_query,
        collection,
        graph_depth,
        top_k,
        alpha,
        beta,
        gamma,
        filter,
    };
    match h.db.tri_modal_query(&q) {
        Ok(results) => {
            let json: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "rank_score": r.rank_score,
                        "vector_score": r.vector_score,
                        "graph_weight": r.graph_weight,
                        "fts_rank": r.fts_rank,
                        "metadata": r.metadata
                    })
                })
                .collect();
            let s = serde_json::Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Stats ─────────────────────────────────────────────────────────────

/// Return database statistics as a JSON object.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_stats(handle: *mut AgentDbHandle) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    match h.db.stats() {
        Ok(s) => {
            let json = serde_json::json!({
                "collections":      s.collections,
                "vectors":          s.vectors,
                "nodes":            s.nodes,
                "edges":            s.edges,
                "conversations":    s.conversations,
                "messages":         s.messages,
                "workflows":        s.workflows,
                "workflow_steps":   s.workflow_steps,
                "traces":           s.traces,
                "tools":            s.tools,
                "tool_calls":       s.tool_calls,
                "audit_entries":    s.audit_entries,
                "prompt_templates": s.prompt_templates
            });
            CString::new(json.to_string())
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Conversations ────────────────────────────────────────────────────────

/// Create a new conversation.
///
/// `id`       — unique conversation identifier
/// `title`    — optional title (may be NULL)
/// `metadata` — optional JSON string (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_conversation_create(
    handle: *mut AgentDbHandle,
    id: *const c_char,
    title: *const c_char,
    metadata: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let title_opt = if title.is_null() {
        None
    } else {
        CStr::from_ptr(title).to_str().ok()
    };
    let meta: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h
        .db
        .conversations()
        .create_conversation(id_str, title_opt, meta)
    {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Add a message to an existing conversation.
///
/// Returns the new message ID as a heap-allocated string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_conversation_add_message(
    handle: *mut AgentDbHandle,
    conversation_id: *const c_char,
    role: *const c_char,
    content: *const c_char,
    metadata: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let cid = match CStr::from_ptr(conversation_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid conversation_id");
            return std::ptr::null_mut();
        }
    };
    let role_str = match CStr::from_ptr(role).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid role");
            return std::ptr::null_mut();
        }
    };
    let content_str = match CStr::from_ptr(content).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid content");
            return std::ptr::null_mut();
        }
    };
    let meta: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h
        .db
        .conversations()
        .add_message(cid, role_str, content_str, meta)
    {
        Ok(msg_id) => CString::new(msg_id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get messages for a conversation as a JSON array.
///
/// `limit` — maximum messages to return (0 = all).
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_conversation_get_messages(
    handle: *mut AgentDbHandle,
    conversation_id: *const c_char,
    limit: usize,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let cid = match CStr::from_ptr(conversation_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid conversation_id");
            return std::ptr::null_mut();
        }
    };
    let lim = if limit == 0 { None } else { Some(limit) };
    match h.db.conversations().get_messages(cid, lim) {
        Ok(msgs) => {
            let json: Vec<Value> = msgs
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m.id,
                        "conversation_id": m.conversation_id,
                        "role": m.role,
                        "content": m.content,
                        "metadata": m.metadata,
                        "created_at": m.created_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// List all conversations as a JSON array.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_conversation_list(handle: *mut AgentDbHandle) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    match h.db.conversations().list_conversations() {
        Ok(convos) => {
            let json: Vec<Value> = convos
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "title": c.title,
                        "metadata": c.metadata,
                        "created_at": c.created_at,
                        "updated_at": c.updated_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Delete a conversation and all its messages.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_conversation_delete(
    handle: *mut AgentDbHandle,
    id: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    match h.db.conversations().delete_conversation(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── Workflows ────────────────────────────────────────────────────────────

/// Create a new workflow in `pending` status.
///
/// `id`       — unique workflow identifier
/// `name`     — human-readable workflow name
/// `input`    — optional JSON input (may be NULL)
/// `metadata` — optional JSON metadata (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_create(
    handle: *mut AgentDbHandle,
    id: *const c_char,
    name: *const c_char,
    input: *const c_char,
    metadata: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid name");
            return -1;
        }
    };
    let input_val: Option<Value> = if input.is_null() {
        None
    } else {
        CStr::from_ptr(input)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let metadata_val: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h
        .db
        .workflows()
        .create_workflow(id_str, name_str, input_val, metadata_val)
    {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Add a step to an existing workflow.
///
/// Returns the step ID as a heap-allocated string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_add_step(
    handle: *mut AgentDbHandle,
    workflow_id: *const c_char,
    name: *const c_char,
    input: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let wid = match CStr::from_ptr(workflow_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid workflow_id");
            return std::ptr::null_mut();
        }
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid name");
            return std::ptr::null_mut();
        }
    };
    let input_val: Option<Value> = if input.is_null() {
        None
    } else {
        CStr::from_ptr(input)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h.db.workflows().add_step(wid, name_str, input_val) {
        Ok(step_id) => CString::new(step_id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Update a workflow step's status, output, and/or error.
///
/// `status` — new status string ("running", "completed", "failed")
/// `output` — optional JSON output (may be NULL)
/// `error`  — optional error message (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_update_step(
    handle: *mut AgentDbHandle,
    step_id: *const c_char,
    status: *const c_char,
    output: *const c_char,
    error: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let sid = match CStr::from_ptr(step_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid step_id");
            return -1;
        }
    };
    let status_str = match CStr::from_ptr(status).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid status");
            return -1;
        }
    };
    let output_val: Option<Value> = if output.is_null() {
        None
    } else {
        CStr::from_ptr(output)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let error_str = if error.is_null() {
        None
    } else {
        CStr::from_ptr(error).to_str().ok()
    };
    match h
        .db
        .workflows()
        .update_step(sid, status_str, output_val, error_str)
    {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Mark a workflow as completed with optional output.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_complete(
    handle: *mut AgentDbHandle,
    id: *const c_char,
    output: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let output_val: Option<Value> = if output.is_null() {
        None
    } else {
        CStr::from_ptr(output)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h.db.workflows().complete_workflow(id_str, output_val) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Mark a workflow as failed with an optional error message.
///
/// `error` — optional error string (may be NULL)
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_fail(
    handle: *mut AgentDbHandle,
    id: *const c_char,
    error: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let error_str = if error.is_null() {
        None
    } else {
        CStr::from_ptr(error).to_str().ok()
    };
    match h.db.workflows().fail_workflow(id_str, error_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get a workflow and its steps as a JSON object.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_get(
    handle: *mut AgentDbHandle,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return std::ptr::null_mut();
        }
    };
    match h.db.workflows().get_workflow(id_str) {
        Ok(w) => {
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
            let json = serde_json::json!({
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
            CString::new(json.to_string())
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// List workflows as a JSON array, optionally filtered by status.
///
/// `status_filter` — status to filter by (may be NULL for all).
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_workflow_list(
    handle: *mut AgentDbHandle,
    status_filter: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let filter = if status_filter.is_null() {
        None
    } else {
        CStr::from_ptr(status_filter).to_str().ok()
    };
    match h.db.workflows().list_workflows(filter) {
        Ok(workflows) => {
            let json: Vec<Value> = workflows
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "id": w.id,
                        "name": w.name,
                        "status": w.status,
                        "created_at": w.created_at,
                        "updated_at": w.updated_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Traces ───────────────────────────────────────────────────────────────

/// Record a new reasoning trace entry.
///
/// Returns the trace ID as a heap-allocated string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_trace_add(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
    parent_id: *const c_char,
    trace_type: *const c_char,
    content: *const c_char,
    metadata: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let sid = if session_id.is_null() {
        None
    } else {
        CStr::from_ptr(session_id).to_str().ok()
    };
    let pid = if parent_id.is_null() {
        None
    } else {
        CStr::from_ptr(parent_id).to_str().ok()
    };
    let tt = match CStr::from_ptr(trace_type).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid trace_type");
            return std::ptr::null_mut();
        }
    };
    let content_str = match CStr::from_ptr(content).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid content");
            return std::ptr::null_mut();
        }
    };
    let meta: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h.db.traces().add_trace(sid, pid, tt, content_str, meta) {
        Ok(trace_id) => CString::new(trace_id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get all traces for a session as a JSON array.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_trace_get_by_session(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let sid = match CStr::from_ptr(session_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid session_id");
            return std::ptr::null_mut();
        }
    };
    match h.db.traces().get_traces(sid, None, None) {
        Ok(traces) => {
            let json: Vec<Value> = traces
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "session_id": t.session_id,
                        "parent_id": t.parent_id,
                        "trace_type": t.trace_type,
                        "content": t.content,
                        "metadata": t.metadata,
                        "created_at": t.created_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Missing vector operations ─────────────────────────────────────────────

/// Delete a single vector from a collection by ID.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_vector_delete(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    id: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection name");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    let col = match h.db.vectors().get_collection(col_name) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    match col.delete(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Drop an entire vector collection and all its vectors.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_drop_collection(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection name");
            return -1;
        }
    };
    match h.db.vectors().drop_collection(col_name) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Rebuild the HNSW index for a collection from stored vectors.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_reindex(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    dim: usize,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection name");
            return -1;
        }
    };
    let col = match h.db.vectors().collection(col_name, dim) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(e.to_string());
            return -1;
        }
    };
    match col.reindex() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── Missing graph operations ──────────────────────────────────────────────

/// Get a graph node as a JSON object, or NULL if not found.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_get_node(
    handle: *mut AgentDbHandle,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return std::ptr::null_mut();
        }
    };
    match h.db.memory().get_node(id_str) {
        Ok(node) => {
            let json = serde_json::json!({
                "id": node.id,
                "kind": node.kind,
                "data": node.data,
                "created_at": node.created_at,
                "updated_at": node.updated_at
            });
            CString::new(json.to_string())
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Delete a graph node (and all its connected edges via CASCADE).
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_delete_node(
    handle: *mut AgentDbHandle,
    id: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid id");
            return -1;
        }
    };
    match h.db.memory().delete_node(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Delete the directed edge from `src` to `dst` with the given `relation`.
///
/// Returns 0 on success, -1 on error (including edge not found).
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_delete_edge(
    handle: *mut AgentDbHandle,
    src: *const c_char,
    dst: *const c_char,
    relation: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let src_str = match CStr::from_ptr(src).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid src");
            return -1;
        }
    };
    let dst_str = match CStr::from_ptr(dst).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid dst");
            return -1;
        }
    };
    let rel_str = match CStr::from_ptr(relation).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid relation");
            return -1;
        }
    };
    match h.db.memory().delete_edge(src_str, dst_str, rel_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── Missing FTS operations ────────────────────────────────────────────────

/// Delete a document from the FTS index.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_fts_delete(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
    vec_id: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection");
            return -1;
        }
    };
    let vid = match CStr::from_ptr(vec_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid vec_id");
            return -1;
        }
    };
    match h.db.fts().delete_text(col, vid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Merge FTS index segments for faster queries (optimize).
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_fts_optimize(
    handle: *mut AgentDbHandle,
    collection: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid collection");
            return -1;
        }
    };
    match h.db.fts().optimize(col) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── Tool Registry ────────────────────────────────────────────────────────

/// Register or update a tool definition.
///
/// `name`              — unique tool name
/// `description`       — human-readable description (may be NULL)
/// `parameters_schema` — JSON Schema string (may be NULL)
/// `version`           — semver version string (may be NULL, defaults to "1.0.0")
///
/// Returns the tool ID as a heap-allocated string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_tool_register(
    handle: *mut AgentDbHandle,
    name: *const c_char,
    description: *const c_char,
    parameters_schema: *const c_char,
    version: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid name");
            return std::ptr::null_mut();
        }
    };
    let desc = if description.is_null() {
        None
    } else {
        CStr::from_ptr(description).to_str().ok()
    };
    let schema: Option<Value> = if parameters_schema.is_null() {
        None
    } else {
        CStr::from_ptr(parameters_schema)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let ver = if version.is_null() {
        None
    } else {
        CStr::from_ptr(version).to_str().ok()
    };
    match h.db.tools().register_tool(name_str, desc, schema, ver) {
        Ok(id) => CString::new(id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// List all registered tools as a JSON array.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_tool_list(handle: *mut AgentDbHandle) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    match h.db.tools().list_tools() {
        Ok(tools) => {
            let json: Vec<Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "name": t.name,
                        "description": t.description,
                        "parameters_schema": t.parameters_schema,
                        "version": t.version,
                        "created_at": t.created_at,
                        "updated_at": t.updated_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Log a tool call invocation.
///
/// `session_id`    — optional session context (may be NULL)
/// `tool_name`     — name of the tool called
/// `arguments`     — JSON arguments (may be NULL)
/// `result`        — JSON result (may be NULL)
/// `error`         — error message (may be NULL)
/// `latency_ms`    — execution time in milliseconds (-1 if unknown)
///
/// Returns the tool call ID as a heap-allocated string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_tool_log_call(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
    tool_name: *const c_char,
    arguments: *const c_char,
    result: *const c_char,
    error: *const c_char,
    latency_ms: i64,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let sid = if session_id.is_null() {
        None
    } else {
        CStr::from_ptr(session_id).to_str().ok()
    };
    let tn = match CStr::from_ptr(tool_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid tool_name");
            return std::ptr::null_mut();
        }
    };
    let args: Option<Value> = if arguments.is_null() {
        None
    } else {
        CStr::from_ptr(arguments)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let res: Option<Value> = if result.is_null() {
        None
    } else {
        CStr::from_ptr(result)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let err = if error.is_null() {
        None
    } else {
        CStr::from_ptr(error).to_str().ok()
    };
    let lat = if latency_ms < 0 {
        None
    } else {
        Some(latency_ms)
    };
    match h.db.tools().log_tool_call(sid, tn, args, res, err, lat) {
        Ok(id) => CString::new(id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Audit Log ────────────────────────────────────────────────────────────

/// Append an entry to the immutable audit log.
///
/// `actor`      — who performed the action (may be NULL)
/// `action`     — action type (e.g. "insert", "update", "delete")
/// `table_name` — target table
/// `record_id`  — target record ID
/// `old_value`  — JSON of previous state (may be NULL)
/// `new_value`  — JSON of new state (may be NULL)
/// `reason`     — human-readable reason (may be NULL)
///
/// Returns the audit entry ID, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_audit_log(
    handle: *mut AgentDbHandle,
    actor: *const c_char,
    action: *const c_char,
    table_name: *const c_char,
    record_id: *const c_char,
    old_value: *const c_char,
    new_value: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let actor_opt = if actor.is_null() {
        None
    } else {
        CStr::from_ptr(actor).to_str().ok()
    };
    let action_str = match CStr::from_ptr(action).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid action");
            return std::ptr::null_mut();
        }
    };
    let tbl = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid table_name");
            return std::ptr::null_mut();
        }
    };
    let rid = match CStr::from_ptr(record_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid record_id");
            return std::ptr::null_mut();
        }
    };
    let old: Option<Value> = if old_value.is_null() {
        None
    } else {
        CStr::from_ptr(old_value)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let new: Option<Value> = if new_value.is_null() {
        None
    } else {
        CStr::from_ptr(new_value)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    let reason_opt = if reason.is_null() {
        None
    } else {
        CStr::from_ptr(reason).to_str().ok()
    };
    match h
        .db
        .audit()
        .log(actor_opt, action_str, tbl, rid, old, new, reason_opt)
    {
        Ok(id) => CString::new(id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Query recent audit log entries as a JSON array.
///
/// `limit` — max entries to return (0 = default 100).
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_audit_query_recent(
    handle: *mut AgentDbHandle,
    limit: usize,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let lim = if limit == 0 { None } else { Some(limit) };
    match h.db.audit().query_recent(lim) {
        Ok(entries) => {
            let json: Vec<Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "timestamp": e.timestamp,
                        "actor": e.actor,
                        "action": e.action,
                        "table_name": e.table_name,
                        "record_id": e.record_id,
                        "old_value": e.old_value,
                        "new_value": e.new_value,
                        "reason": e.reason
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Context Window ───────────────────────────────────────────────────────

/// Add an entry to the context window for a session.
///
/// Returns the entry ID, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_context_add(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
    source_type: *const c_char,
    source_id: *const c_char,
    content_preview: *const c_char,
    token_count: i64,
    relevance_score: f64,
    priority: i64,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let sid = match CStr::from_ptr(session_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid session_id");
            return std::ptr::null_mut();
        }
    };
    let st = match CStr::from_ptr(source_type).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid source_type");
            return std::ptr::null_mut();
        }
    };
    let si = match CStr::from_ptr(source_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid source_id");
            return std::ptr::null_mut();
        }
    };
    let preview = if content_preview.is_null() {
        None
    } else {
        CStr::from_ptr(content_preview).to_str().ok()
    };
    match h
        .db
        .context()
        .add_entry(sid, st, si, preview, token_count, relevance_score, priority)
    {
        Ok(id) => CString::new(id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Build a token-budgeted context window for a session.
///
/// Returns entries as a JSON array, filling up to `max_tokens`.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_context_build_window(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
    max_tokens: i64,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let sid = match CStr::from_ptr(session_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid session_id");
            return std::ptr::null_mut();
        }
    };
    match h.db.context().build_window(sid, max_tokens) {
        Ok(entries) => {
            let json: Vec<Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "session_id": e.session_id,
                        "source_type": e.source_type,
                        "source_id": e.source_id,
                        "content_preview": e.content_preview,
                        "token_count": e.token_count,
                        "relevance_score": e.relevance_score,
                        "priority": e.priority,
                        "included_at": e.included_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Clear all context entries for a session.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_context_clear(
    handle: *mut AgentDbHandle,
    session_id: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let sid = match CStr::from_ptr(session_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid session_id");
            return -1;
        }
    };
    match h.db.context().clear_session(sid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ── Prompt Templates ─────────────────────────────────────────────────────

/// Create a new version of a prompt template.
///
/// `name`      — template name (versions auto-increment per name)
/// `template`  — template body with {{placeholder}} syntax
/// `model_hint`— suggested model (may be NULL)
/// `max_tokens`— suggested max tokens (-1 if not set)
/// `metadata`  — optional JSON metadata (may be NULL)
///
/// Returns the template ID, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_prompt_create(
    handle: *mut AgentDbHandle,
    name: *const c_char,
    template: *const c_char,
    model_hint: *const c_char,
    max_tokens: i64,
    metadata: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid name");
            return std::ptr::null_mut();
        }
    };
    let tmpl_str = match CStr::from_ptr(template).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid template");
            return std::ptr::null_mut();
        }
    };
    let model = if model_hint.is_null() {
        None
    } else {
        CStr::from_ptr(model_hint).to_str().ok()
    };
    let max_tok = if max_tokens < 0 {
        None
    } else {
        Some(max_tokens)
    };
    let meta: Option<Value> = if metadata.is_null() {
        None
    } else {
        CStr::from_ptr(metadata)
            .to_str()
            .ok()
            .and_then(|s| serde_json::from_str(s).ok())
    };
    match h
        .db
        .prompts()
        .create_template(name_str, tmpl_str, model, max_tok, meta)
    {
        Ok(id) => CString::new(id)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Render a prompt template with variable substitution.
///
/// `name`      — template name (uses latest version)
/// `vars_json` — JSON object of key-value pairs for {{placeholder}} substitution
///
/// Returns the rendered string, or NULL on error.
/// Free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_prompt_render(
    handle: *mut AgentDbHandle,
    name: *const c_char,
    vars_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid name");
            return std::ptr::null_mut();
        }
    };
    let vars: std::collections::HashMap<String, String> = if vars_json.is_null() {
        std::collections::HashMap::new()
    } else {
        match CStr::from_ptr(vars_json).to_str() {
            Ok(s) => serde_json::from_str::<serde_json::Map<String, Value>>(s)
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        match v {
                            Value::String(s) => s,
                            other => other.to_string(),
                        },
                    )
                })
                .collect(),
            Err(_) => {
                set_last_error("invalid vars_json");
                return std::ptr::null_mut();
            }
        }
    };
    match h.db.prompts().render(name_str, &vars) {
        Ok(rendered) => CString::new(rendered)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ── Data Labels ──────────────────────────────────────────────────────────

/// Tag a record with a privacy/classification label.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_label_tag(
    handle: *mut AgentDbHandle,
    table_name: *const c_char,
    record_id: *const c_char,
    label: *const c_char,
    tagged_by: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let tbl = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid table_name");
            return -1;
        }
    };
    let rid = match CStr::from_ptr(record_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid record_id");
            return -1;
        }
    };
    let lbl = match CStr::from_ptr(label).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid label");
            return -1;
        }
    };
    let by = if tagged_by.is_null() {
        None
    } else {
        CStr::from_ptr(tagged_by).to_str().ok()
    };
    match h.db.labels().tag(tbl, rid, lbl, by) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Remove a specific label from a record.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_label_untag(
    handle: *mut AgentDbHandle,
    table_name: *const c_char,
    record_id: *const c_char,
    label: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let tbl = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid table_name");
            return -1;
        }
    };
    let rid = match CStr::from_ptr(record_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid record_id");
            return -1;
        }
    };
    let lbl = match CStr::from_ptr(label).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid label");
            return -1;
        }
    };
    match h.db.labels().untag(tbl, rid, lbl) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get all labels for a record as a JSON array.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_label_get(
    handle: *mut AgentDbHandle,
    table_name: *const c_char,
    record_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let tbl = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid table_name");
            return std::ptr::null_mut();
        }
    };
    let rid = match CStr::from_ptr(record_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid record_id");
            return std::ptr::null_mut();
        }
    };
    match h.db.labels().get_labels(tbl, rid) {
        Ok(labels) => {
            let json: Vec<Value> = labels
                .iter()
                .map(|l| {
                    serde_json::json!({
                        "table_name": l.table_name,
                        "record_id": l.record_id,
                        "label": l.label,
                        "tagged_by": l.tagged_by,
                        "tagged_at": l.tagged_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Check if a record has a specific label.
///
/// Returns 1 if true, 0 if false, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn agentdb_label_has(
    handle: *mut AgentDbHandle,
    table_name: *const c_char,
    record_id: *const c_char,
    label: *const c_char,
) -> i32 {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return -1;
        }
    };
    let tbl = match CStr::from_ptr(table_name).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid table_name");
            return -1;
        }
    };
    let rid = match CStr::from_ptr(record_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid record_id");
            return -1;
        }
    };
    let lbl = match CStr::from_ptr(label).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid label");
            return -1;
        }
    };
    match h.db.labels().has_label(tbl, rid, lbl) {
        Ok(has) => {
            if has {
                1
            } else {
                0
            }
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get a trace subtree as a JSON array.
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_trace_get_tree(
    handle: *mut AgentDbHandle,
    root_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => {
            set_last_error("null handle");
            return std::ptr::null_mut();
        }
    };
    let rid = match CStr::from_ptr(root_id).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_last_error("invalid root_id");
            return std::ptr::null_mut();
        }
    };
    match h.db.traces().get_trace_tree(rid) {
        Ok(traces) => {
            let json: Vec<Value> = traces
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "session_id": t.session_id,
                        "parent_id": t.parent_id,
                        "trace_type": t.trace_type,
                        "content": t.content,
                        "metadata": t.metadata,
                        "created_at": t.created_at
                    })
                })
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s)
                .map(|c| c.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}
