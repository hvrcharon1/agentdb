//! Comprehensive integration tests for `src/async_api.rs`.
//!
//! Covers every async store type across the full async API surface.
//! Run with: `cargo test --features async --test test_async_api`

#![cfg(feature = "async")]

use agentdb::{AsyncAgentDB, BatchEntry, DistanceMetric, SearchOptions, VectorEntry};
use serde_json::json;
use std::collections::HashMap;

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn open() -> AsyncAgentDB {
    AsyncAgentDB::open(":memory:").await.expect("failed to open async in-memory db")
}

// ── AsyncAgentDB open / close ─────────────────────────────────────────────────

#[tokio::test]
async fn async_open_returns_zeroed_stats() {
    let db = open().await;
    let s = db.stats().await.unwrap();
    assert_eq!(s.collections, 0);
    assert_eq!(s.vectors, 0);
    assert_eq!(s.nodes, 0);
    assert_eq!(s.conversations, 0);
}

#[tokio::test]
async fn async_close_sole_owner_succeeds() {
    let db = open().await;
    db.close().await.unwrap();
}

#[tokio::test]
async fn async_close_with_clone_errors() {
    let db = open().await;
    let _clone = db.clone();
    let result = db.close().await;
    assert!(result.is_err(), "close must fail when a clone still exists");
}

// ── AsyncVectorStore — collection creation ────────────────────────────────────

#[tokio::test]
async fn async_vector_collection_create_and_list() {
    let db = open().await;
    let vs = db.vectors();
    vs.collection("alpha", 4).await.unwrap();
    vs.collection("beta", 8).await.unwrap();
    let cols = vs.list_collections().await.unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.iter().any(|(n, _, _)| n == "alpha"));
    assert!(cols.iter().any(|(n, _, _)| n == "beta"));
}

#[tokio::test]
async fn async_vector_collection_with_metric() {
    let db = open().await;
    let vs = db.vectors();
    vs.collection_with_metric("dot_col", 4, DistanceMetric::DotProduct)
        .await
        .unwrap();
    let cols = vs.list_collections().await.unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].0, "dot_col");
}

#[tokio::test]
async fn async_vector_drop_collection() {
    let db = open().await;
    let vs = db.vectors();
    vs.collection("temp", 4).await.unwrap();
    assert_eq!(vs.list_collections().await.unwrap().len(), 1);
    vs.drop_collection("temp").await.unwrap();
    assert!(vs.list_collections().await.unwrap().is_empty());
}

// ── AsyncCollection — upsert / search / count ─────────────────────────────────

#[tokio::test]
async fn async_vector_upsert_and_count() {
    let db = open().await;
    let col = db.vectors().collection("thoughts", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "v1".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: None,
    })
    .await
    .unwrap();
    col.upsert(VectorEntry {
        id: "v2".into(),
        vector: vec![0.0, 1.0, 0.0, 0.0],
        metadata: None,
    })
    .await
    .unwrap();
    assert_eq!(col.count().await.unwrap(), 2);
}

#[tokio::test]
async fn async_vector_search_top_k() {
    let db = open().await;
    let col = db.vectors().collection("mem", 4).await.unwrap();
    for i in 0..5u32 {
        col.upsert(VectorEntry {
            id: format!("v{i}"),
            vector: vec![i as f32 / 4.0, 0.0, 0.0, 0.0],
            metadata: None,
        })
        .await
        .unwrap();
    }
    let results = col
        .search(
            vec![1.0, 0.0, 0.0, 0.0],
            SearchOptions {
                top_k: 2,
                metric: DistanceMetric::Cosine,
                filter: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn async_vector_search_with_metadata_filter() {
    let db = open().await;
    let col = db.vectors().collection("filtered", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "user".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: Some(json!({"role": "user"})),
    })
    .await
    .unwrap();
    col.upsert(VectorEntry {
        id: "assistant".into(),
        vector: vec![0.99, 0.0, 0.0, 0.0],
        metadata: Some(json!({"role": "assistant"})),
    })
    .await
    .unwrap();
    let results = col
        .search(
            vec![1.0, 0.0, 0.0, 0.0],
            SearchOptions {
                top_k: 10,
                metric: DistanceMetric::Cosine,
                filter: Some(json!({"role": "user"})),
            },
        )
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "user");
}

#[tokio::test]
async fn async_vector_upsert_updates_existing() {
    let db = open().await;
    let col = db.vectors().collection("col", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "v1".into(),
        vector: vec![0.1, 0.0, 0.0, 0.0],
        metadata: Some(json!({"ver": 1})),
    })
    .await
    .unwrap();
    col.upsert(VectorEntry {
        id: "v1".into(),
        vector: vec![0.9, 0.0, 0.0, 0.0],
        metadata: Some(json!({"ver": 2})),
    })
    .await
    .unwrap();
    assert_eq!(col.count().await.unwrap(), 1, "re-upsert must not inflate count");
}

#[tokio::test]
async fn async_vector_batch_upsert() {
    let db = open().await;
    let col = db.vectors().collection("batch", 4).await.unwrap();
    let entries = vec![
        BatchEntry { id: "b1".into(), vector: vec![1.0, 0.0, 0.0, 0.0], metadata: None },
        BatchEntry { id: "b2".into(), vector: vec![0.0, 1.0, 0.0, 0.0], metadata: None },
        BatchEntry { id: "b3".into(), vector: vec![0.0, 0.0, 1.0, 0.0], metadata: None },
    ];
    let inserted = col.upsert_batch(entries).await.unwrap();
    assert_eq!(inserted, 3);
    assert_eq!(col.count().await.unwrap(), 3);
}

#[tokio::test]
async fn async_vector_delete() {
    let db = open().await;
    let col = db.vectors().collection("del_col", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "gone".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: None,
    })
    .await
    .unwrap();
    assert_eq!(col.count().await.unwrap(), 1);
    col.delete("gone").await.unwrap();
    assert_eq!(col.count().await.unwrap(), 0);
}

#[tokio::test]
async fn async_vector_reindex() {
    let db = open().await;
    let col = db.vectors().collection("reindex", 4).await.unwrap();
    col.upsert(VectorEntry {
        id: "r1".into(),
        vector: vec![1.0, 0.0, 0.0, 0.0],
        metadata: None,
    })
    .await
    .unwrap();
    col.reindex().await.unwrap();
    assert_eq!(col.count().await.unwrap(), 1);
}

#[tokio::test]
async fn async_vector_upsert_with_text() {
    let db = open().await;
    let col = db.vectors().collection("with_text", 4).await.unwrap();
    col.upsert_with_text(
        VectorEntry {
            id: "doc1".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: Some(json!({"title": "Rust async"})),
        },
        "Rust async programming with tokio".to_string(),
    )
    .await
    .unwrap();
    assert_eq!(col.count().await.unwrap(), 1);
}

// ── AsyncConversationStore ────────────────────────────────────────────────────

#[tokio::test]
async fn async_conversation_create_and_list() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", Some("First"), None).await.unwrap();
    convs.create_conversation("c2", None, Some(json!({"agent": "bot"}))).await.unwrap();
    let list = convs.list_conversations().await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|c| c.id == "c1"));
    assert!(list.iter().any(|c| c.id == "c2"));
}

#[tokio::test]
async fn async_conversation_add_message_returns_id() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    let id = convs.add_message("c1", "user", "Hello!", None).await.unwrap();
    assert!(!id.is_empty());
}

#[tokio::test]
async fn async_conversation_get_messages_ordered() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    convs.add_message("c1", "system", "You are helpful.", None).await.unwrap();
    convs.add_message("c1", "user", "What is 2+2?", None).await.unwrap();
    convs.add_message("c1", "assistant", "4.", None).await.unwrap();
    let msgs = convs.get_messages("c1", None).await.unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].role, "system");
    assert_eq!(msgs[1].role, "user");
    assert_eq!(msgs[2].role, "assistant");
    assert_eq!(msgs[2].content, "4.");
}

#[tokio::test]
async fn async_conversation_get_messages_with_limit() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    for i in 0..6u32 {
        convs.add_message("c1", "user", &format!("msg {i}"), None).await.unwrap();
    }
    let msgs = convs.get_messages("c1", Some(3)).await.unwrap();
    assert_eq!(msgs.len(), 3);
}

#[tokio::test]
async fn async_conversation_delete_cascades_messages() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    convs.add_message("c1", "user", "Hi", None).await.unwrap();
    convs.delete_conversation("c1").await.unwrap();
    assert!(convs.list_conversations().await.unwrap().is_empty());
    assert!(convs.get_messages("c1", None).await.unwrap().is_empty());
}

#[tokio::test]
async fn async_conversation_search_messages() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    convs.add_message("c1", "user", "The quick brown fox", None).await.unwrap();
    convs.add_message("c1", "assistant", "It jumps over the lazy dog", None).await.unwrap();
    let results = convs.search_messages("fox", 10, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].conversation_id, "c1");
}

#[tokio::test]
async fn async_conversation_search_messages_filter_by_conversation() {
    let db = open().await;
    let convs = db.conversations();
    convs.create_conversation("c1", None, None).await.unwrap();
    convs.create_conversation("c2", None, None).await.unwrap();
    convs.add_message("c1", "user", "apple banana", None).await.unwrap();
    convs.add_message("c2", "user", "apple orange", None).await.unwrap();
    let all = convs.search_messages("apple", 10, None).await.unwrap();
    assert_eq!(all.len(), 2);
    let c1_only = convs.search_messages("apple", 10, Some("c1")).await.unwrap();
    assert_eq!(c1_only.len(), 1);
    assert_eq!(c1_only[0].conversation_id, "c1");
}

// ── AsyncWorkflowStore ────────────────────────────────────────────────────────

#[tokio::test]
async fn async_workflow_full_lifecycle() {
    let db = open().await;
    let wf = db.workflows();
    wf.create_workflow("wf-1", "Pipeline", Some(json!({"mode": "fast"})), None)
        .await
        .unwrap();
    let step1 = wf.add_step("wf-1", "Fetch", None).await.unwrap();
    let step2 = wf.add_step("wf-1", "Process", None).await.unwrap();
    assert!(!step1.is_empty());
    assert!(!step2.is_empty());
    wf.update_step(&step1, "completed", Some(json!({"rows": 10})), None)
        .await
        .unwrap();
    wf.update_step(&step2, "completed", Some(json!({"processed": 10})), None)
        .await
        .unwrap();
    wf.complete_workflow("wf-1", Some(json!({"result": "ok"})))
        .await
        .unwrap();
    let workflow = wf.get_workflow("wf-1").await.unwrap();
    assert_eq!(workflow.status, "completed");
    assert_eq!(workflow.steps.len(), 2);
}

#[tokio::test]
async fn async_workflow_fail() {
    let db = open().await;
    let wf = db.workflows();
    wf.create_workflow("wf-fail", "Failing", None, None).await.unwrap();
    wf.add_step("wf-fail", "Step1", None).await.unwrap();
    wf.fail_workflow("wf-fail", Some("network timeout")).await.unwrap();
    let workflow = wf.get_workflow("wf-fail").await.unwrap();
    assert_eq!(workflow.status, "failed");
}

#[tokio::test]
async fn async_workflow_list_with_status_filter() {
    let db = open().await;
    let wf = db.workflows();
    wf.create_workflow("wf-a", "WA", None, None).await.unwrap();
    wf.create_workflow("wf-b", "WB", None, None).await.unwrap();
    wf.complete_workflow("wf-a", None).await.unwrap();
    let completed = wf.list_workflows(Some("completed")).await.unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, "wf-a");
    let all = wf.list_workflows(None).await.unwrap();
    assert_eq!(all.len(), 2);
}

// ── AsyncTraceStore ───────────────────────────────────────────────────────────

#[tokio::test]
async fn async_trace_add_and_get() {
    let db = open().await;
    let tr = db.traces();
    let root = tr
        .add_trace(Some("sess-1"), None, "plan", "planning step", None)
        .await
        .unwrap();
    let child = tr
        .add_trace(Some("sess-1"), Some(&root), "tool_call", "web_search", None)
        .await
        .unwrap();
    assert!(!root.is_empty());
    assert!(!child.is_empty());
    let traces = tr.get_traces("sess-1", None, None).await.unwrap();
    assert_eq!(traces.len(), 2);
}

#[tokio::test]
async fn async_trace_get_with_limit_and_offset() {
    let db = open().await;
    let tr = db.traces();
    for i in 0..5u32 {
        tr.add_trace(Some("sess-2"), None, "step", &format!("step {i}"), None)
            .await
            .unwrap();
    }
    let limited = tr.get_traces("sess-2", Some(2), None).await.unwrap();
    assert_eq!(limited.len(), 2);
    let offset = tr.get_traces("sess-2", Some(2), Some(2)).await.unwrap();
    assert_eq!(offset.len(), 2);
}

#[tokio::test]
async fn async_trace_tree() {
    let db = open().await;
    let tr = db.traces();
    let root = tr
        .add_trace(Some("sess-3"), None, "root", "root trace", None)
        .await
        .unwrap();
    tr.add_trace(Some("sess-3"), Some(&root), "child_a", "child A", None)
        .await
        .unwrap();
    tr.add_trace(Some("sess-3"), Some(&root), "child_b", "child B", None)
        .await
        .unwrap();
    let tree = tr.get_trace_tree(&root).await.unwrap();
    assert!(!tree.is_empty());
    // Root plus two children
    assert!(tree.len() >= 3);
}

// ── AsyncMemoryGraph ──────────────────────────────────────────────────────────

#[tokio::test]
async fn async_memory_add_node_and_get() {
    let db = open().await;
    let mem = db.memory();
    mem.add_node("n1", "concept", Some(json!({"value": 42}))).await.unwrap();
    let node = mem.get_node("n1").await.unwrap();
    assert_eq!(node.id, "n1");
    assert_eq!(node.kind, "concept");
    assert_eq!(node.data.as_ref().unwrap()["value"], 42);
}

#[tokio::test]
async fn async_memory_add_edge_and_neighbors() {
    use agentdb::TraversalOptions;
    let db = open().await;
    let mem = db.memory();
    mem.add_node("a", "entity", None).await.unwrap();
    mem.add_node("b", "entity", None).await.unwrap();
    mem.add_node("c", "entity", None).await.unwrap();
    mem.add_edge("a", "b", "knows", 1.0).await.unwrap();
    mem.add_edge("a", "c", "knows", 0.5).await.unwrap();
    let nbrs = mem
        .neighbors("a", TraversalOptions { relation: None, max_depth: 1, min_weight: None })
        .await
        .unwrap();
    assert_eq!(nbrs.len(), 2);
}

#[tokio::test]
async fn async_memory_neighbors_with_relation_filter() {
    use agentdb::TraversalOptions;
    let db = open().await;
    let mem = db.memory();
    mem.add_node("x", "node", None).await.unwrap();
    mem.add_node("y", "node", None).await.unwrap();
    mem.add_node("z", "node", None).await.unwrap();
    mem.add_edge("x", "y", "friend", 1.0).await.unwrap();
    mem.add_edge("x", "z", "colleague", 1.0).await.unwrap();
    let friends = mem
        .neighbors(
            "x",
            TraversalOptions {
                relation: Some("friend".to_string()),
                max_depth: 1,
                min_weight: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(friends.len(), 1);
    assert_eq!(friends[0].node.id, "y");
}

#[tokio::test]
async fn async_memory_delete_node_and_edge() {
    use agentdb::TraversalOptions;
    let db = open().await;
    let mem = db.memory();
    mem.add_node("p", "entity", None).await.unwrap();
    mem.add_node("q", "entity", None).await.unwrap();
    mem.add_edge("p", "q", "linked", 1.0).await.unwrap();

    // Delete the edge first, then verify it's gone
    mem.delete_edge("p", "q", "linked").await.unwrap();
    let nbrs = mem
        .neighbors("p", TraversalOptions { relation: None, max_depth: 1, min_weight: None })
        .await
        .unwrap();
    assert!(nbrs.is_empty());

    // Delete the node
    mem.delete_node("p").await.unwrap();
    let result = mem.get_node("p").await;
    assert!(result.is_err(), "get_node on deleted node should return an error");
}

#[tokio::test]
async fn async_memory_nodes_by_kind() {
    let db = open().await;
    let mem = db.memory();
    mem.add_node("doc1", "document", None).await.unwrap();
    mem.add_node("doc2", "document", None).await.unwrap();
    mem.add_node("sess1", "session", None).await.unwrap();
    let docs = mem.nodes_by_kind("document").await.unwrap();
    assert_eq!(docs.len(), 2);
    let sessions = mem.nodes_by_kind("session").await.unwrap();
    assert_eq!(sessions.len(), 1);
}

// ── AsyncFullTextStore ────────────────────────────────────────────────────────

#[tokio::test]
async fn async_fts_index_and_search() {
    let db = open().await;
    let fts = db.fts();
    fts.index_text("docs", "d1", "col1", "The quick brown fox jumps over the lazy dog")
        .await
        .unwrap();
    fts.index_text("docs", "d2", "col1", "A fast red car drove past the sleeping cat")
        .await
        .unwrap();
    let results = fts.search("docs", "quick fox", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "d1");
}

#[tokio::test]
async fn async_fts_search_empty_collection() {
    let db = open().await;
    let results = db.fts().search("nonexistent", "anything", 10).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn async_fts_delete_text() {
    let db = open().await;
    let fts = db.fts();
    fts.index_text("notes", "n1", "col1", "Important meeting tomorrow").await.unwrap();
    fts.index_text("notes", "n2", "col1", "Meeting cancelled").await.unwrap();
    fts.delete_text("notes", "n1").await.unwrap();
    let results = fts.search("notes", "meeting", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "n2");
}

#[tokio::test]
async fn async_fts_upsert_replaces_existing() {
    let db = open().await;
    let fts = db.fts();
    fts.index_text("docs", "d1", "col1", "Original content here").await.unwrap();
    fts.index_text("docs", "d1", "col1", "Updated replacement content").await.unwrap();
    let old_results = fts.search("docs", "original", 10).await.unwrap();
    assert!(old_results.is_empty());
    let new_results = fts.search("docs", "replacement", 10).await.unwrap();
    assert_eq!(new_results.len(), 1);
    assert_eq!(new_results[0].id, "d1");
}

#[tokio::test]
async fn async_fts_optimize() {
    let db = open().await;
    let fts = db.fts();
    fts.index_text("opt", "o1", "col1", "Optimization test document").await.unwrap();
    fts.optimize("opt").await.unwrap();
    let results = fts.search("opt", "optimization", 10).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn async_fts_top_k_limits_results() {
    let db = open().await;
    let fts = db.fts();
    for i in 0..8u32 {
        fts.index_text("many", &format!("m{i}"), "col1", &format!("document with query word {i}"))
            .await
            .unwrap();
    }
    let results = fts.search("many", "query", 3).await.unwrap();
    assert!(results.len() <= 3);
}

// ── AsyncToolStore ────────────────────────────────────────────────────────────

#[tokio::test]
async fn async_tool_register_and_get() {
    let db = open().await;
    let tools = db.tools();
    let id = tools
        .register_tool("web_search", Some("Search the web"), None, Some("1.0.0"))
        .await
        .unwrap();
    assert!(!id.is_empty());
    let tool = tools.get_tool("web_search").await.unwrap();
    assert_eq!(tool.name, "web_search");
    assert_eq!(tool.description.as_deref(), Some("Search the web"));
    assert_eq!(tool.version, "1.0.0");
}

#[tokio::test]
async fn async_tool_register_with_schema() {
    let db = open().await;
    let schema = json!({
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "required": ["query"]
    });
    db.tools()
        .register_tool("search", Some("Search"), Some(schema), Some("2.0.0"))
        .await
        .unwrap();
    let tool = db.tools().get_tool("search").await.unwrap();
    assert_eq!(tool.parameters_schema.unwrap()["type"], "object");
}

#[tokio::test]
async fn async_tool_upsert_on_conflict() {
    let db = open().await;
    let tools = db.tools();
    tools.register_tool("calc", Some("v1"), None, Some("1.0")).await.unwrap();
    tools.register_tool("calc", Some("v2"), None, Some("2.0")).await.unwrap();
    let tool = tools.get_tool("calc").await.unwrap();
    assert_eq!(tool.description.as_deref(), Some("v2"));
    assert_eq!(tools.list_tools().await.unwrap().len(), 1);
}

#[tokio::test]
async fn async_tool_list_all() {
    let db = open().await;
    let tools = db.tools();
    tools.register_tool("tool_a", None, None, None).await.unwrap();
    tools.register_tool("tool_b", None, None, None).await.unwrap();
    tools.register_tool("tool_c", None, None, None).await.unwrap();
    let list = tools.list_tools().await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn async_tool_delete() {
    let db = open().await;
    let tools = db.tools();
    tools.register_tool("tmp", None, None, None).await.unwrap();
    tools.delete_tool("tmp").await.unwrap();
    assert!(tools.get_tool("tmp").await.is_err());
}

#[tokio::test]
async fn async_tool_log_call_and_get() {
    let db = open().await;
    let tools = db.tools();
    let call_id = tools
        .log_tool_call(
            Some("sess-1"),
            "web_search",
            Some(json!({"query": "rust"})),
            Some(json!({"results": []})),
            None,
            Some(50),
        )
        .await
        .unwrap();
    assert!(!call_id.is_empty());
    let calls = tools.get_tool_calls(Some("sess-1"), None, None).await.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "web_search");
    assert_eq!(calls[0].latency_ms, Some(50));
}

#[tokio::test]
async fn async_tool_log_call_with_error() {
    let db = open().await;
    let tools = db.tools();
    tools
        .log_tool_call(Some("sess-2"), "api_call", None, None, Some("timeout"), Some(30000))
        .await
        .unwrap();
    let calls = tools.get_tool_calls(Some("sess-2"), None, None).await.unwrap();
    assert_eq!(calls[0].error.as_deref(), Some("timeout"));
}

#[tokio::test]
async fn async_tool_get_calls_filter_by_tool_name() {
    let db = open().await;
    let tools = db.tools();
    tools.log_tool_call(Some("s1"), "search", None, None, None, None).await.unwrap();
    tools.log_tool_call(Some("s1"), "calc", None, None, None, None).await.unwrap();
    let calc_calls = tools.get_tool_calls(None, Some("calc"), None).await.unwrap();
    assert_eq!(calc_calls.len(), 1);
    assert_eq!(calc_calls[0].tool_name, "calc");
}

#[tokio::test]
async fn async_tool_get_calls_with_limit() {
    let db = open().await;
    let tools = db.tools();
    for _ in 0..6u32 {
        tools.log_tool_call(Some("s1"), "ping", None, None, None, None).await.unwrap();
    }
    let limited = tools.get_tool_calls(Some("s1"), None, Some(3)).await.unwrap();
    assert_eq!(limited.len(), 3);
}

// ── AsyncAuditStore ───────────────────────────────────────────────────────────

#[tokio::test]
async fn async_audit_log_returns_id() {
    let db = open().await;
    let id = db
        .audit()
        .log(
            Some("agent-1"),
            "insert",
            "_adb_nodes",
            "node-abc",
            None,
            Some(json!({"kind": "session"})),
            None,
        )
        .await
        .unwrap();
    assert!(!id.is_empty());
}

#[tokio::test]
async fn async_audit_log_with_old_and_new_values() {
    let db = open().await;
    let audit = db.audit();
    audit
        .log(
            Some("agent-1"),
            "update",
            "table_a",
            "rec-1",
            Some(json!({"status": "active"})),
            Some(json!({"status": "archived"})),
            Some("user requested archive"),
        )
        .await
        .unwrap();
    let entries = audit.query_by_record("table_a", "rec-1", None).await.unwrap();
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.actor.as_deref(), Some("agent-1"));
    assert_eq!(e.action, "update");
    assert_eq!(e.old_value.as_ref().unwrap()["status"], "active");
    assert_eq!(e.new_value.as_ref().unwrap()["status"], "archived");
    assert_eq!(e.reason.as_deref(), Some("user requested archive"));
}

#[tokio::test]
async fn async_audit_query_by_actor() {
    let db = open().await;
    let audit = db.audit();
    audit.log(Some("alice"), "insert", "t", "r1", None, None, None).await.unwrap();
    audit.log(Some("bob"), "insert", "t", "r2", None, None, None).await.unwrap();
    audit.log(Some("alice"), "update", "t", "r1", None, None, None).await.unwrap();
    let alice_entries = audit.query_by_actor("alice", None).await.unwrap();
    assert_eq!(alice_entries.len(), 2);
    for e in &alice_entries {
        assert_eq!(e.actor.as_deref(), Some("alice"));
    }
}

#[tokio::test]
async fn async_audit_query_recent_with_limit() {
    let db = open().await;
    let audit = db.audit();
    for i in 0..5u32 {
        audit.log(None, "ping", "t", &format!("r{i}"), None, None, None).await.unwrap();
    }
    let recent = audit.query_recent(Some(2)).await.unwrap();
    assert_eq!(recent.len(), 2);
    assert!(recent[0].timestamp >= recent[1].timestamp);
}

#[tokio::test]
async fn async_audit_query_by_record_with_limit() {
    let db = open().await;
    let audit = db.audit();
    for _ in 0..4u32 {
        audit.log(None, "touch", "tbl", "rec", None, None, None).await.unwrap();
    }
    let results = audit.query_by_record("tbl", "rec", Some(2)).await.unwrap();
    assert_eq!(results.len(), 2);
}

// ── AsyncContextStore ─────────────────────────────────────────────────────────

#[tokio::test]
async fn async_context_add_entry_and_get() {
    let db = open().await;
    let ctx = db.context();
    let id = ctx
        .add_entry("sess-1", "message", "msg-001", Some("Hello world"), 5, 0.9, 10)
        .await
        .unwrap();
    assert!(!id.is_empty());
    let entries = ctx.get_entries("sess-1").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].session_id, "sess-1");
    assert_eq!(entries[0].source_type, "message");
    assert_eq!(entries[0].content_preview.as_deref(), Some("Hello world"));
}

#[tokio::test]
async fn async_context_build_window_respects_token_budget() {
    let db = open().await;
    let ctx = db.context();
    ctx.add_entry("s1", "a", "1", None, 50, 0.9, 10).await.unwrap();
    ctx.add_entry("s1", "b", "2", None, 50, 0.8, 9).await.unwrap();
    ctx.add_entry("s1", "c", "3", None, 50, 0.7, 8).await.unwrap();
    let window = ctx.build_window("s1", 100).await.unwrap();
    assert_eq!(window.len(), 2);
    let total: i64 = window.iter().map(|e| e.token_count).sum();
    assert!(total <= 100);
}

#[tokio::test]
async fn async_context_build_window_prioritises_high_priority() {
    let db = open().await;
    let ctx = db.context();
    ctx.add_entry("s1", "low", "1", None, 30, 0.9, 1).await.unwrap();
    ctx.add_entry("s1", "high", "2", None, 30, 0.5, 100).await.unwrap();
    let window = ctx.build_window("s1", 40).await.unwrap();
    assert_eq!(window.len(), 1);
    assert_eq!(window[0].source_type, "high");
}

#[tokio::test]
async fn async_context_clear_session() {
    let db = open().await;
    let ctx = db.context();
    ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).await.unwrap();
    ctx.add_entry("s1", "b", "2", None, 10, 0.5, 1).await.unwrap();
    ctx.clear_session("s1").await.unwrap();
    assert!(ctx.get_entries("s1").await.unwrap().is_empty());
}

#[tokio::test]
async fn async_context_remove_entry() {
    let db = open().await;
    let ctx = db.context();
    let id = ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).await.unwrap();
    ctx.add_entry("s1", "b", "2", None, 10, 0.5, 1).await.unwrap();
    ctx.remove_entry(&id).await.unwrap();
    let entries = ctx.get_entries("s1").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source_type, "b");
}

// ── AsyncPromptStore ──────────────────────────────────────────────────────────

#[tokio::test]
async fn async_prompt_create_template_and_get() {
    let db = open().await;
    let prompts = db.prompts();
    let id = prompts
        .create_template("greeting", "Hello, {{name}}!", Some("claude-3-opus"), Some(4096), None)
        .await
        .unwrap();
    assert!(!id.is_empty());
    let tmpl = prompts.get_template("greeting").await.unwrap();
    assert_eq!(tmpl.name, "greeting");
    assert_eq!(tmpl.template, "Hello, {{name}}!");
    assert_eq!(tmpl.model_hint.as_deref(), Some("claude-3-opus"));
    assert_eq!(tmpl.max_tokens, Some(4096));
}

#[tokio::test]
async fn async_prompt_versioning() {
    let db = open().await;
    let prompts = db.prompts();
    prompts.create_template("sys", "You are v1", None, None, None).await.unwrap();
    prompts.create_template("sys", "You are v2", None, None, None).await.unwrap();
    prompts.create_template("sys", "You are v3", None, None, None).await.unwrap();
    let latest = prompts.get_template("sys").await.unwrap();
    assert_eq!(latest.version, 3);
    assert_eq!(latest.template, "You are v3");
}

#[tokio::test]
async fn async_prompt_list_templates() {
    let db = open().await;
    let prompts = db.prompts();
    prompts.create_template("a", "body a", None, None, None).await.unwrap();
    prompts.create_template("a", "body a v2", None, None, None).await.unwrap();
    prompts.create_template("b", "body b", None, None, None).await.unwrap();
    let list = prompts.list_templates().await.unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn async_prompt_render_with_vars() {
    let db = open().await;
    let prompts = db.prompts();
    prompts
        .create_template("greet", "Hello {{name}}, welcome to {{place}}!", None, None, None)
        .await
        .unwrap();
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), "Alice".to_string());
    vars.insert("place".to_string(), "Wonderland".to_string());
    let rendered = prompts.render("greet", vars).await.unwrap();
    assert_eq!(rendered, "Hello Alice, welcome to Wonderland!");
}

#[tokio::test]
async fn async_prompt_render_missing_var_left_intact() {
    let db = open().await;
    let prompts = db.prompts();
    prompts
        .create_template("partial", "Hi {{name}}, id is {{id}}", None, None, None)
        .await
        .unwrap();
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), "Bob".to_string());
    let rendered = prompts.render("partial", vars).await.unwrap();
    assert_eq!(rendered, "Hi Bob, id is {{id}}");
}

#[tokio::test]
async fn async_prompt_delete_template() {
    let db = open().await;
    let prompts = db.prompts();
    prompts.create_template("doomed", "v1", None, None, None).await.unwrap();
    prompts.create_template("doomed", "v2", None, None, None).await.unwrap();
    prompts.delete_template("doomed").await.unwrap();
    assert!(prompts.get_template("doomed").await.is_err());
    // Deleting a non-existent template should not error
    prompts.delete_template("ghost").await.unwrap();
}

// ── AsyncLabelStore ───────────────────────────────────────────────────────────

#[tokio::test]
async fn async_label_tag_and_get() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("_adb_messages", "msg-1", "pii", Some("agent-1")).await.unwrap();
    let result = labels.get_labels("_adb_messages", "msg-1").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].label, "pii");
    assert_eq!(result[0].tagged_by.as_deref(), Some("agent-1"));
}

#[tokio::test]
async fn async_label_multiple_labels_on_record() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "rec", "pii", None).await.unwrap();
    labels.tag("tbl", "rec", "sensitive", None).await.unwrap();
    labels.tag("tbl", "rec", "internal", None).await.unwrap();
    let result = labels.get_labels("tbl", "rec").await.unwrap();
    assert_eq!(result.len(), 3);
}

#[tokio::test]
async fn async_label_upsert_on_duplicate() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "rec", "secret", Some("alice")).await.unwrap();
    labels.tag("tbl", "rec", "secret", Some("bob")).await.unwrap();
    let result = labels.get_labels("tbl", "rec").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].tagged_by.as_deref(), Some("bob"));
}

#[tokio::test]
async fn async_label_untag() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "rec", "pii", None).await.unwrap();
    labels.tag("tbl", "rec", "internal", None).await.unwrap();
    labels.untag("tbl", "rec", "pii").await.unwrap();
    let result = labels.get_labels("tbl", "rec").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].label, "internal");
}

#[tokio::test]
async fn async_label_has_label() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "rec", "secret", None).await.unwrap();
    assert!(labels.has_label("tbl", "rec", "secret").await.unwrap());
    assert!(!labels.has_label("tbl", "rec", "public").await.unwrap());
    labels.untag("tbl", "rec", "secret").await.unwrap();
    assert!(!labels.has_label("tbl", "rec", "secret").await.unwrap());
}

#[tokio::test]
async fn async_label_find_by_label() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl_a", "r1", "pii", None).await.unwrap();
    labels.tag("tbl_a", "r2", "pii", None).await.unwrap();
    labels.tag("tbl_b", "r3", "pii", None).await.unwrap();
    labels.tag("tbl_a", "r1", "internal", None).await.unwrap();
    let pii = labels.find_by_label("pii", None).await.unwrap();
    assert_eq!(pii.len(), 3);
    for l in &pii {
        assert_eq!(l.label, "pii");
    }
}

#[tokio::test]
async fn async_label_find_by_label_with_limit() {
    let db = open().await;
    let labels = db.labels();
    for i in 0..8u32 {
        labels.tag("tbl", &format!("r{i}"), "tag", None).await.unwrap();
    }
    let results = labels.find_by_label("tag", Some(3)).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn async_label_clear_record() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "rec", "a", None).await.unwrap();
    labels.tag("tbl", "rec", "b", None).await.unwrap();
    labels.clear_record("tbl", "rec").await.unwrap();
    assert!(labels.get_labels("tbl", "rec").await.unwrap().is_empty());
}

#[tokio::test]
async fn async_label_clear_does_not_affect_other_records() {
    let db = open().await;
    let labels = db.labels();
    labels.tag("tbl", "r1", "tag", None).await.unwrap();
    labels.tag("tbl", "r2", "tag", None).await.unwrap();
    labels.clear_record("tbl", "r1").await.unwrap();
    assert!(labels.has_label("tbl", "r2", "tag").await.unwrap());
}

// ── stats integration (async) ─────────────────────────────────────────────────

#[tokio::test]
async fn async_stats_reflects_all_stores() {
    let db = open().await;

    // Vectors
    let col = db.vectors().collection("stats_col", 4).await.unwrap();
    col.upsert(VectorEntry { id: "v1".into(), vector: vec![1.0, 0.0, 0.0, 0.0], metadata: None })
        .await
        .unwrap();

    // Memory nodes
    db.memory().add_node("n1", "concept", None).await.unwrap();
    db.memory().add_node("n2", "concept", None).await.unwrap();

    // Conversation
    db.conversations().create_conversation("c1", None, None).await.unwrap();

    // Tools
    db.tools().register_tool("my_tool", None, None, None).await.unwrap();
    db.tools().log_tool_call(None, "my_tool", None, None, None, None).await.unwrap();

    // Audit
    db.audit().log(None, "op", "t", "r", None, None, None).await.unwrap();

    let s = db.stats().await.unwrap();
    assert_eq!(s.collections, 1);
    assert_eq!(s.vectors, 1);
    assert_eq!(s.nodes, 2);
    assert_eq!(s.conversations, 1);
    assert_eq!(s.tools, 1);
    assert_eq!(s.tool_calls, 1);
    assert_eq!(s.audit_entries, 1);
}
