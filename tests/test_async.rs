//! Integration tests for the async API (`src/async_api.rs`).
//!
//! Run with: `cargo test --features async`

#![cfg(feature = "async")]

use agentdb::AsyncAgentDB;
use serde_json::json;

// ── Open / close ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_open_and_stats() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let s = db.stats().await.unwrap();
    assert_eq!(s.collections, 0);
    assert_eq!(s.vectors, 0);
    assert_eq!(s.nodes, 0);
}

#[tokio::test]
async fn async_close_succeeds_when_sole_owner() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    db.close().await.unwrap();
}

#[tokio::test]
async fn async_close_errors_when_cloned() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let _clone = db.clone();
    let result = db.close().await;
    assert!(result.is_err(), "close should fail when a clone exists");
}

// ── SQL execute / query ───────────────────────────────────────────────────────

#[tokio::test]
async fn async_execute_and_query() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    db.execute("CREATE TABLE t (id TEXT PRIMARY KEY, v INTEGER)")
        .await
        .unwrap();
    let n = db.execute("INSERT INTO t VALUES ('x', 42)").await.unwrap();
    assert_eq!(n, 1);
    let rows = db.query_json("SELECT * FROM t").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["v"], 42);
}

// ── Vectors ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_vector_upsert_and_search() {
    use agentdb::{SearchOptions, VectorEntry};
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let col = db.vectors().collection("thoughts", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "v1".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: Some(json!({"tag": "a"})),
    })
    .await
    .unwrap();
    let results = col
        .search(vec![1.0, 0.0, 0.0, 0.0], SearchOptions::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "v1");
}

#[tokio::test]
async fn async_vector_delete() {
    use agentdb::VectorEntry;
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let col = db.vectors().collection("col", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "to_delete".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: None,
    })
    .await
    .unwrap();
    assert_eq!(col.count().await.unwrap(), 1);
    col.delete("to_delete").await.unwrap();
    assert_eq!(col.count().await.unwrap(), 0);
}

#[tokio::test]
async fn async_list_and_drop_collections() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let vs = db.vectors();
    vs.collection("alpha", 4).await.unwrap();
    vs.collection("beta", 8).await.unwrap();
    let cols = vs.list_collections().await.unwrap();
    assert_eq!(cols.len(), 2);
    vs.drop_collection("alpha").await.unwrap();
    let cols = vs.list_collections().await.unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].0, "beta");
}

// ── Memory graph ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_memory_graph() {
    use agentdb::TraversalOptions;
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let mem = db.memory();
    mem.add_node("n1", "concept", None).await.unwrap();
    mem.add_node("n2", "concept", None).await.unwrap();
    mem.add_edge("n1", "n2", "relates_to", 1.0).await.unwrap();
    let nbrs = mem
        .neighbors(
            "n1",
            TraversalOptions {
                relation: None,
                max_depth: 2,
                min_weight: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(nbrs.len(), 1);
    assert_eq!(nbrs[0].node.id, "n2");
}

// ── Conversations ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_conversations() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let convs = db.conversations();
    convs
        .create_conversation("c1", Some("Thread"), None)
        .await
        .unwrap();
    let msg_id = convs
        .add_message("c1", "user", "Hello", None)
        .await
        .unwrap();
    assert!(!msg_id.is_empty());
    let msgs = convs.get_messages("c1", None).await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "Hello");
}

// ── Workflows ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_workflow_lifecycle() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let wf = db.workflows();
    wf.create_workflow("wf-1", "Pipeline", None, None)
        .await
        .unwrap();
    let step_id = wf.add_step("wf-1", "Fetch", None).await.unwrap();
    wf.update_step(&step_id, "running", None, None)
        .await
        .unwrap();
    wf.update_step(&step_id, "completed", Some(json!({"ok": true})), None)
        .await
        .unwrap();
    wf.complete_workflow("wf-1", Some(json!({"result": "done"})))
        .await
        .unwrap();
    let workflow = wf.get_workflow("wf-1").await.unwrap();
    assert_eq!(workflow.status, "completed");
    assert_eq!(workflow.steps.len(), 1);
    assert_eq!(workflow.steps[0].status, "completed");
}

// ── Traces ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_traces() {
    let db = AsyncAgentDB::open(":memory:").await.unwrap();
    let tr = db.traces();
    // add_trace(session_id, parent_id, trace_type, content, metadata)
    let root = tr
        .add_trace(Some("session-1"), None, "plan", "plan step", None)
        .await
        .unwrap();
    let _child = tr
        .add_trace(Some("session-1"), Some(&root), "tool_call", "search", None)
        .await
        .unwrap();
    let traces = tr.get_traces("session-1", None, None).await.unwrap();
    assert_eq!(traces.len(), 2);
    let tree = tr.get_trace_tree(&root).await.unwrap();
    assert!(!tree.is_empty());
}
