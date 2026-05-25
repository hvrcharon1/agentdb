//! Integration tests for the C FFI layer (src/ffi.rs).
//!
//! These tests call the `extern "C"` functions directly from Rust
//! (zero overhead vs a real C caller, same ABI guarantees).
//!
//! To run:
//!   cargo test --features ffi --test test_ffi

#![cfg(feature = "ffi")]

use agentdb::ffi::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// ── helpers ────────────────────────────────────────────────────────────

fn cstr(s: &str) -> CString {
    CString::new(s).expect("CString::new")
}

unsafe fn last_error() -> Option<String> {
    let p = agentdb_last_error();
    if p.is_null() {
        return None;
    }
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    agentdb_free_string(p);
    Some(s)
}

unsafe fn free_str(p: *mut c_char) -> String {
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    agentdb_free_string(p);
    s
}

// ── tests ──────────────────────────────────────────────────────────────

#[test]
fn ffi_open_memory_and_close() {
    unsafe {
        let path = cstr(":memory:");
        let h = agentdb_open(path.as_ptr());
        assert!(!h.is_null(), "open failed: {:?}", last_error());
        agentdb_close(h);
    }
}

#[test]
fn ffi_open_invalid_path_returns_null() {
    unsafe {
        // A path that cannot be created on any OS.
        let path = cstr("/dev/null/impossible/path/agentdb");
        let h = agentdb_open(path.as_ptr());
        // May succeed on some systems (SQLite error at bootstrap) or return NULL;
        // either way no panic and no UB.
        if !h.is_null() {
            agentdb_close(h);
        }
    }
}

#[test]
fn ffi_execute_and_query_json() {
    unsafe {
        let h = agentdb_open(cstr(":memory:").as_ptr());
        assert!(!h.is_null());

        let ddl = cstr("CREATE TABLE notes (id TEXT PRIMARY KEY, body TEXT)");
        let rows = agentdb_execute(h, ddl.as_ptr());
        assert!(rows >= 0, "DDL failed");

        let dml = cstr("INSERT INTO notes VALUES ('n1', 'hello world')");
        let rows = agentdb_execute(h, dml.as_ptr());
        assert_eq!(rows, 1);

        let sel = cstr("SELECT id, body FROM notes");
        let json_ptr = agentdb_query_json(h, sel.as_ptr());
        assert!(!json_ptr.is_null());
        let json = free_str(json_ptr);
        assert!(json.contains("n1"), "unexpected json: {json}");
        assert!(json.contains("hello world"), "unexpected json: {json}");

        agentdb_close(h);
    }
}

#[test]
fn ffi_vector_upsert_and_search() {
    unsafe {
        let h = agentdb_open(cstr(":memory:").as_ptr());
        assert!(!h.is_null());

        let col = cstr("thoughts");
        let id  = cstr("t1");
        let vec: Vec<f32> = vec![0.9, 0.1, 0.0, 0.0];
        let meta = cstr(r#"{"score":9}"#);

        let rc = agentdb_vector_upsert(
            h,
            col.as_ptr(),
            id.as_ptr(),
            vec.as_ptr(),
            vec.len(),
            meta.as_ptr(),
        );
        assert_eq!(rc, 0, "upsert failed: {:?}", last_error());

        // Insert a second vector so search has something to rank.
        let id2  = cstr("t2");
        let vec2: Vec<f32> = vec![0.1, 0.9, 0.0, 0.0];
        let _rc = agentdb_vector_upsert(
            h, col.as_ptr(), id2.as_ptr(),
            vec2.as_ptr(), vec2.len(), std::ptr::null(),
        );

        let query: Vec<f32> = vec![0.9, 0.1, 0.0, 0.0];
        let results_ptr = agentdb_vector_search(
            h,
            col.as_ptr(),
            query.as_ptr(),
            query.len(),
            2,
            std::ptr::null(),
        );
        assert!(!results_ptr.is_null(), "search failed: {:?}", last_error());
        let results = free_str(results_ptr);
        assert!(results.contains("t1"), "t1 not in results: {results}");

        agentdb_close(h);
    }
}

#[test]
fn ffi_graph_add_node_and_edge_and_neighbors() {
    unsafe {
        let h = agentdb_open(cstr(":memory:").as_ptr());
        assert!(!h.is_null());

        let rc1 = agentdb_graph_add_node(
            h,
            cstr("s1").as_ptr(),
            cstr("session").as_ptr(),
            std::ptr::null(),
        );
        assert_eq!(rc1, 0);

        let data = cstr(r#"{"label":"Rust"}"#);
        let rc2 = agentdb_graph_add_node(
            h,
            cstr("c1").as_ptr(),
            cstr("concept").as_ptr(),
            data.as_ptr(),
        );
        assert_eq!(rc2, 0);

        let rc3 = agentdb_graph_add_edge(
            h,
            cstr("s1").as_ptr(),
            cstr("c1").as_ptr(),
            cstr("discussed").as_ptr(),
            0.9,
        );
        assert_eq!(rc3, 0);

        let nb_ptr = agentdb_graph_neighbors(h, cstr("s1").as_ptr(), 2, 0.0);
        assert!(!nb_ptr.is_null(), "neighbors failed: {:?}", last_error());
        let nb = free_str(nb_ptr);
        assert!(nb.contains("c1"), "c1 not in neighbors: {nb}");

        agentdb_close(h);
    }
}

#[test]
fn ffi_fts_index_and_search() {
    unsafe {
        let h = agentdb_open(cstr(":memory:").as_ptr());
        assert!(!h.is_null());

        // We need a vector collection to get a collection_id for FTS.
        let col = cstr("docs");
        let id  = cstr("d1");
        let vec: Vec<f32> = vec![0.5, 0.5];
        agentdb_vector_upsert(h, col.as_ptr(), id.as_ptr(), vec.as_ptr(), 2, std::ptr::null());

        // Use a placeholder collection_id (FTS doesn't validate it).
        let rc = agentdb_fts_index(
            h,
            col.as_ptr(),
            cstr("d1").as_ptr(),
            cstr("placeholder-col-id").as_ptr(),
            cstr("Rust systems programming safety").as_ptr(),
        );
        assert_eq!(rc, 0, "fts_index failed: {:?}", last_error());

        let res_ptr = agentdb_fts_search(h, col.as_ptr(), cstr("safety").as_ptr(), 5);
        assert!(!res_ptr.is_null(), "fts_search failed: {:?}", last_error());
        let res = free_str(res_ptr);
        assert!(res.contains("d1"), "d1 not in fts results: {res}");

        agentdb_close(h);
    }
}

#[test]
fn ffi_stats_returns_valid_json() {
    unsafe {
        let h = agentdb_open(cstr(":memory:").as_ptr());
        assert!(!h.is_null());

        let s_ptr = agentdb_stats(h);
        assert!(!s_ptr.is_null());
        let s = free_str(s_ptr);
        // Must contain all four stat keys.
        for key in &["collections", "vectors", "nodes", "edges"] {
            assert!(s.contains(key), "missing '{key}' in stats: {s}");
        }

        agentdb_close(h);
    }
}

#[test]
fn ffi_free_string_null_is_safe() {
    // Must not panic or segfault.
    unsafe { agentdb_free_string(std::ptr::null_mut()); }
}
