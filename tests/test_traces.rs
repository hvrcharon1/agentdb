#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── add_trace ─────────────────────────────────────────────────────────────

    #[test]
    fn test_add_trace_returns_id() {
        let db = open();
        let trace_id = db
            .traces()
            .add_trace(
                Some("session-1"),
                None,
                "thought",
                "Initial reasoning.",
                None,
            )
            .unwrap();
        assert!(!trace_id.is_empty());
    }

    #[test]
    fn test_add_trace_without_session() {
        let db = open();
        let trace_id = db
            .traces()
            .add_trace(None, None, "thought", "No session.", None)
            .unwrap();
        assert!(!trace_id.is_empty());
    }

    #[test]
    fn test_add_trace_with_metadata() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(
                Some("session-1"),
                None,
                "tool_call",
                "Calling search.",
                Some(json!({ "tool": "search", "query": "rust" })),
            )
            .unwrap();
        let results = traces.get_traces("session-1", None, None).unwrap();
        assert_eq!(results[0].metadata.as_ref().unwrap()["tool"], "search");
    }

    // ── get_traces ────────────────────────────────────────────────────────────

    #[test]
    fn test_get_traces_returns_session_traces() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(Some("session-1"), None, "thought", "Step 1", None)
            .unwrap();
        traces
            .add_trace(Some("session-1"), None, "observation", "Step 2", None)
            .unwrap();
        let results = traces.get_traces("session-1", None, None).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_traces_correct_session_only() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(Some("session-A"), None, "thought", "A", None)
            .unwrap();
        traces
            .add_trace(Some("session-B"), None, "thought", "B", None)
            .unwrap();
        let results_a = traces.get_traces("session-A", None, None).unwrap();
        assert_eq!(results_a.len(), 1);
        assert_eq!(results_a[0].content, "A");
        let results_b = traces.get_traces("session-B", None, None).unwrap();
        assert_eq!(results_b.len(), 1);
        assert_eq!(results_b[0].content, "B");
    }

    #[test]
    fn test_get_traces_chronological_order() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(Some("session-1"), None, "thought", "first", None)
            .unwrap();
        traces
            .add_trace(Some("session-1"), None, "thought", "second", None)
            .unwrap();
        let results = traces.get_traces("session-1", None, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].created_at <= results[1].created_at);
    }

    #[test]
    fn test_get_traces_includes_correct_fields() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(Some("session-1"), None, "tool_call", "Use hammer.", None)
            .unwrap();
        let results = traces.get_traces("session-1", None, None).unwrap();
        let t = &results[0];
        assert_eq!(t.session_id.as_deref(), Some("session-1"));
        assert_eq!(t.trace_type, "tool_call");
        assert_eq!(t.content, "Use hammer.");
        assert!(t.parent_id.is_none());
    }

    // ── edge cases: nonexistent session ──────────────────────────────────────

    #[test]
    fn test_get_traces_nonexistent_session_returns_empty() {
        let db = open();
        let results = db
            .traces()
            .get_traces("no-such-session", None, None)
            .unwrap();
        assert!(results.is_empty());
    }

    // ── pagination ────────────────────────────────────────────────────────────

    #[test]
    fn test_get_traces_limit() {
        let db = open();
        let tr = db.traces();
        for i in 0..5 {
            tr.add_trace(Some("s"), None, "thought", &format!("msg {i}"), None)
                .unwrap();
        }
        let page = tr.get_traces("s", Some(3), None).unwrap();
        assert_eq!(page.len(), 3);
    }

    #[test]
    fn test_get_traces_offset() {
        let db = open();
        let tr = db.traces();
        for i in 0..5 {
            tr.add_trace(Some("s"), None, "thought", &format!("msg {i}"), None)
                .unwrap();
        }
        let all = tr.get_traces("s", None, None).unwrap();
        let page = tr.get_traces("s", None, Some(2)).unwrap();
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].content, all[2].content);
    }

    #[test]
    fn test_get_traces_limit_and_offset() {
        let db = open();
        let tr = db.traces();
        for i in 0..5 {
            tr.add_trace(Some("s"), None, "thought", &format!("msg {i}"), None)
                .unwrap();
        }
        let page = tr.get_traces("s", Some(2), Some(2)).unwrap();
        assert_eq!(page.len(), 2);
    }

    // ── get_trace_tree ────────────────────────────────────────────────────────

    #[test]
    fn test_get_trace_tree_root_only() {
        let db = open();
        let traces = db.traces();
        let root_id = traces
            .add_trace(Some("session-1"), None, "thought", "Root thought.", None)
            .unwrap();
        let tree = traces.get_trace_tree(&root_id).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, root_id);
    }

    #[test]
    fn test_get_trace_tree_with_children() {
        let db = open();
        let traces = db.traces();
        // Build: root -> child_1, root -> child_2
        let root_id = traces
            .add_trace(Some("session-1"), None, "thought", "Root", None)
            .unwrap();
        traces
            .add_trace(
                Some("session-1"),
                Some(&root_id),
                "tool_call",
                "Child 1",
                None,
            )
            .unwrap();
        traces
            .add_trace(
                Some("session-1"),
                Some(&root_id),
                "observation",
                "Child 2",
                None,
            )
            .unwrap();
        let tree = traces.get_trace_tree(&root_id).unwrap();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn test_get_trace_tree_deep_nesting() {
        let db = open();
        let traces = db.traces();
        // Build: root -> child -> grandchild
        let root_id = traces
            .add_trace(Some("session-1"), None, "thought", "Root", None)
            .unwrap();
        let child_id = traces
            .add_trace(
                Some("session-1"),
                Some(&root_id),
                "tool_call",
                "Child",
                None,
            )
            .unwrap();
        let grandchild_id = traces
            .add_trace(
                Some("session-1"),
                Some(&child_id),
                "observation",
                "Grandchild",
                None,
            )
            .unwrap();
        let tree = traces.get_trace_tree(&root_id).unwrap();
        assert_eq!(tree.len(), 3);
        let ids: Vec<&str> = tree.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&root_id.as_str()));
        assert!(ids.contains(&child_id.as_str()));
        assert!(ids.contains(&grandchild_id.as_str()));
    }

    #[test]
    fn test_get_trace_tree_from_child_does_not_include_root() {
        let db = open();
        let traces = db.traces();
        // root -> child -> grandchild; querying from child should return only
        // child + grandchild, NOT the root.
        let root_id = traces
            .add_trace(Some("session-1"), None, "thought", "Root", None)
            .unwrap();
        let child_id = traces
            .add_trace(
                Some("session-1"),
                Some(&root_id),
                "tool_call",
                "Child",
                None,
            )
            .unwrap();
        traces
            .add_trace(
                Some("session-1"),
                Some(&child_id),
                "observation",
                "Grandchild",
                None,
            )
            .unwrap();
        let tree = traces.get_trace_tree(&child_id).unwrap();
        assert_eq!(tree.len(), 2);
        let ids: Vec<&str> = tree.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&child_id.as_str()));
        assert!(!ids.contains(&root_id.as_str()));
    }

    #[test]
    fn test_get_trace_tree_nonexistent_root_returns_empty() {
        let db = open();
        let tree = db.traces().get_trace_tree("no-such-root").unwrap();
        assert!(tree.is_empty());
    }

    #[test]
    fn test_get_trace_tree_parent_ids_are_set() {
        let db = open();
        let traces = db.traces();
        let root_id = traces
            .add_trace(Some("session-1"), None, "thought", "Root", None)
            .unwrap();
        let child_id = traces
            .add_trace(
                Some("session-1"),
                Some(&root_id),
                "tool_call",
                "Child",
                None,
            )
            .unwrap();
        let tree = traces.get_trace_tree(&root_id).unwrap();
        let root_trace = tree.iter().find(|t| t.id == root_id).unwrap();
        let child_trace = tree.iter().find(|t| t.id == child_id).unwrap();
        assert!(root_trace.parent_id.is_none());
        assert_eq!(child_trace.parent_id.as_deref(), Some(root_id.as_str()));
    }

    // ── multiple trace types ──────────────────────────────────────────────────

    #[test]
    fn test_add_traces_with_various_types() {
        let db = open();
        let traces = db.traces();
        traces
            .add_trace(
                Some("session-1"),
                None,
                "thought",
                "I need to search.",
                None,
            )
            .unwrap();
        traces
            .add_trace(Some("session-1"), None, "tool_call", "search(query)", None)
            .unwrap();
        traces
            .add_trace(
                Some("session-1"),
                None,
                "observation",
                "Results found.",
                None,
            )
            .unwrap();
        let results = traces.get_traces("session-1", None, None).unwrap();
        assert_eq!(results.len(), 3);
        let types: Vec<&str> = results.iter().map(|t| t.trace_type.as_str()).collect();
        assert!(types.contains(&"thought"));
        assert!(types.contains(&"tool_call"));
        assert!(types.contains(&"observation"));
    }
}
