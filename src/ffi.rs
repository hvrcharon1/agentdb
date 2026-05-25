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
use crate::hybrid::HybridQuery;
use crate::vectors::{DistanceMetric, SearchOptions, VectorEntry};
use serde_json::Value;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ── Thread-local last error ───────────────────────────────────────────

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
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
        Some(msg) => CString::new(msg).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut()),
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
pub extern "C" fn agentdb_open(path: *const c_char) -> *mut AgentDbHandle {
    clear_last_error();
    let path_str = unsafe {
        match path.as_ref().and_then(|p| CStr::from_ptr(p).to_str().ok()) {
            Some(s) => s,
            None => {
                set_last_error("agentdb_open: invalid path string");
                return std::ptr::null_mut();
            }
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
        None => { set_last_error("agentdb_execute: null handle"); return -1; }
    };
    let sql_str = match CStr::from_ptr(sql).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("agentdb_execute: invalid SQL string"); return -1; }
    };
    match h.db.execute(sql_str) {
        Ok(n) => n as i64,
        Err(e) => { set_last_error(e.to_string()); -1 }
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
        None => { set_last_error("agentdb_query_json: null handle"); return std::ptr::null_mut(); }
    };
    let sql_str = match CStr::from_ptr(sql).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("agentdb_query_json: invalid SQL"); return std::ptr::null_mut(); }
    };
    match h.db.query_json(sql_str) {
        Ok(rows) => {
            let json = Value::Array(rows).to_string();
            CString::new(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
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
        None => { set_last_error("null handle"); return -1; }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("invalid collection name"); return -1; }
    };
    let id_str = match CStr::from_ptr(id).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("invalid id"); return -1; }
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
        Err(e) => { set_last_error(e.to_string()); return -1; }
    };
    match col.upsert(VectorEntry { id: id_str.to_string(), vector: vec, metadata: meta }) {
        Ok(()) => 0,
        Err(e) => { set_last_error(e.to_string()); -1 }
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
        None => { set_last_error("null handle"); return std::ptr::null_mut(); }
    };
    let col_name = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("invalid collection name"); return std::ptr::null_mut(); }
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
        Err(e) => { set_last_error(e.to_string()); return std::ptr::null_mut(); }
    };
    match col.search(&q, SearchOptions { top_k, metric: DistanceMetric::Cosine, filter }) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({ "id": r.id, "score": r.score, "metadata": r.metadata }))
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
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
        None => { set_last_error("null handle"); return -1; }
    };
    let id_str   = match CStr::from_ptr(id).to_str()   { Ok(s) => s, Err(_) => { set_last_error("invalid id");   return -1; } };
    let kind_str = match CStr::from_ptr(kind).to_str() { Ok(s) => s, Err(_) => { set_last_error("invalid kind"); return -1; } };
    let data: Option<Value> = if data_json.is_null() {
        None
    } else {
        CStr::from_ptr(data_json).to_str().ok().and_then(|s| serde_json::from_str(s).ok())
    };
    match h.db.memory().add_node(id_str, kind_str, data) {
        Ok(()) => 0,
        Err(e) => { set_last_error(e.to_string()); -1 }
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
        None => { set_last_error("null handle"); return -1; }
    };
    let src_str      = match CStr::from_ptr(src).to_str()      { Ok(s) => s, Err(_) => { set_last_error("invalid src");      return -1; } };
    let dst_str      = match CStr::from_ptr(dst).to_str()      { Ok(s) => s, Err(_) => { set_last_error("invalid dst");      return -1; } };
    let relation_str = match CStr::from_ptr(relation).to_str() { Ok(s) => s, Err(_) => { set_last_error("invalid relation"); return -1; } };
    match h.db.memory().add_edge(src_str, dst_str, relation_str, weight) {
        Ok(()) => 0,
        Err(e) => { set_last_error(e.to_string()); -1 }
    }
}

/// Traverse the memory graph from `node_id` and return results as JSON.
///
/// `max_depth`  — maximum hops from the anchor node
/// `min_weight` — minimum edge weight to traverse (0.0 = all edges)
///
/// Returns heap-allocated JSON string — free with `agentdb_free_string`.
#[no_mangle]
pub unsafe extern "C" fn agentdb_graph_neighbors(
    handle: *mut AgentDbHandle,
    node_id: *const c_char,
    max_depth: usize,
    min_weight: f64,
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => { set_last_error("null handle"); return std::ptr::null_mut(); }
    };
    let id_str = match CStr::from_ptr(node_id).to_str() {
        Ok(s) => s,
        Err(_) => { set_last_error("invalid node_id"); return std::ptr::null_mut(); }
    };
    let opts = crate::memory::TraversalOptions {
        relation:   None,
        max_depth,
        min_weight: Some(min_weight),
    };
    match h.db.memory().neighbors(id_str, opts) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({
                    "id":    r.node.id,
                    "kind":  r.node.kind,
                    "depth": r.depth,
                    "weight": r.weight,
                    "data":  r.node.data
                }))
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
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
        None => { set_last_error("null handle"); return -1; }
    };
    let col  = match CStr::from_ptr(collection).to_str()    { Ok(s) => s, Err(_) => { set_last_error("invalid collection");    return -1; } };
    let vid  = match CStr::from_ptr(vec_id).to_str()        { Ok(s) => s, Err(_) => { set_last_error("invalid vec_id");        return -1; } };
    let cid  = match CStr::from_ptr(collection_id).to_str() { Ok(s) => s, Err(_) => { set_last_error("invalid collection_id"); return -1; } };
    let txt  = match CStr::from_ptr(text).to_str()          { Ok(s) => s, Err(_) => { set_last_error("invalid text");          return -1; } };
    match h.db.fts().index_text(col, vid, cid, txt) {
        Ok(()) => 0,
        Err(e) => { set_last_error(e.to_string()); -1 }
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
        None => { set_last_error("null handle"); return std::ptr::null_mut(); }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s, Err(_) => { set_last_error("invalid collection"); return std::ptr::null_mut(); }
    };
    let q = match CStr::from_ptr(query).to_str() {
        Ok(s) => s, Err(_) => { set_last_error("invalid query"); return std::ptr::null_mut(); }
    };
    match h.db.fts().search(col, q, top_k) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({ "id": r.id, "snippet": r.snippet, "rank": r.rank }))
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
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
) -> *mut c_char {
    clear_last_error();
    let h = match handle.as_ref() {
        Some(h) => h,
        None => { set_last_error("null handle"); return std::ptr::null_mut(); }
    };
    let anchor = match CStr::from_ptr(anchor_node).to_str() {
        Ok(s) => s, Err(_) => { set_last_error("invalid anchor_node"); return std::ptr::null_mut(); }
    };
    let col = match CStr::from_ptr(collection).to_str() {
        Ok(s) => s, Err(_) => { set_last_error("invalid collection"); return std::ptr::null_mut(); }
    };
    let emb: Vec<f32> = std::slice::from_raw_parts(embedding, dim).to_vec();
    let q = HybridQuery {
        anchor_node: anchor,
        embedding:   &emb,
        collection:  col,
        graph_depth,
        top_k,
        alpha,
        filter: None,
    };
    match h.db.hybrid_query(q) {
        Ok(results) => {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({
                    "id":           r.id,
                    "rank_score":   r.rank_score,
                    "vector_score": r.vector_score,
                    "graph_weight": r.graph_weight
                }))
                .collect();
            let s = Value::Array(json).to_string();
            CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
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
        None => { set_last_error("null handle"); return std::ptr::null_mut(); }
    };
    match h.db.stats() {
        Ok(s) => {
            let json = serde_json::json!({
                "collections": s.collections,
                "vectors":     s.vectors,
                "nodes":       s.nodes,
                "edges":       s.edges
            });
            CString::new(json.to_string()).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
        }
        Err(e) => { set_last_error(e.to_string()); std::ptr::null_mut() }
    }
}
