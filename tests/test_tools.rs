#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── register_tool ────────────────────────────────────────────────────────

    #[test]
    fn test_register_tool_returns_id() {
        let db = open();
        let id = db
            .tools()
            .register_tool("web_search", Some("Search the web"), None, None)
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_register_tool_with_schema() {
        let db = open();
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        });
        db.tools()
            .register_tool(
                "web_search",
                Some("Search"),
                Some(schema.clone()),
                Some("2.0.0"),
            )
            .unwrap();
        let tool = db.tools().get_tool("web_search").unwrap();
        assert_eq!(tool.parameters_schema.unwrap()["type"], "object");
        assert_eq!(tool.version, "2.0.0");
    }

    #[test]
    fn test_register_tool_upsert_on_conflict() {
        let db = open();
        let tools = db.tools();
        tools
            .register_tool("calc", Some("Calculator v1"), None, Some("1.0.0"))
            .unwrap();
        tools
            .register_tool("calc", Some("Calculator v2"), None, Some("2.0.0"))
            .unwrap();
        let tool = tools.get_tool("calc").unwrap();
        assert_eq!(tool.description.as_deref(), Some("Calculator v2"));
        assert_eq!(tool.version, "2.0.0");
        let all = tools.list_tools().unwrap();
        assert_eq!(all.len(), 1);
    }

    // ── get_tool ─────────────────────────────────────────────────────────────

    #[test]
    fn test_get_tool_not_found() {
        let db = open();
        let result = db.tools().get_tool("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_tool_fields() {
        let db = open();
        db.tools()
            .register_tool("code_exec", Some("Execute code"), None, Some("1.2.3"))
            .unwrap();
        let tool = db.tools().get_tool("code_exec").unwrap();
        assert_eq!(tool.name, "code_exec");
        assert_eq!(tool.description.as_deref(), Some("Execute code"));
        assert_eq!(tool.version, "1.2.3");
        assert!(tool.created_at > 0);
        assert!(tool.updated_at > 0);
    }

    // ── list_tools ───────────────────────────────────────────────────────────

    #[test]
    fn test_list_tools_empty() {
        let db = open();
        let tools = db.tools().list_tools().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_list_tools_ordered_by_name() {
        let db = open();
        let store = db.tools();
        store.register_tool("zeta", None, None, None).unwrap();
        store.register_tool("alpha", None, None, None).unwrap();
        store.register_tool("mid", None, None, None).unwrap();
        let tools = store.list_tools().unwrap();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "alpha");
        assert_eq!(tools[1].name, "mid");
        assert_eq!(tools[2].name, "zeta");
    }

    // ── delete_tool ──────────────────────────────────────────────────────────

    #[test]
    fn test_delete_tool() {
        let db = open();
        let store = db.tools();
        store.register_tool("tmp", None, None, None).unwrap();
        store.delete_tool("tmp").unwrap();
        assert!(store.get_tool("tmp").is_err());
    }

    #[test]
    fn test_delete_nonexistent_tool_no_error() {
        let db = open();
        db.tools().delete_tool("ghost").unwrap();
    }

    // ── log_tool_call ────────────────────────────────────────────────────────

    #[test]
    fn test_log_tool_call_returns_id() {
        let db = open();
        let id = db
            .tools()
            .log_tool_call(
                Some("session-1"),
                "web_search",
                Some(json!({"query": "rust"})),
                Some(json!({"results": []})),
                None,
                Some(42),
            )
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_log_tool_call_with_error() {
        let db = open();
        db.tools()
            .log_tool_call(
                Some("session-1"),
                "api_call",
                Some(json!({"url": "https://example.com"})),
                None,
                Some("timeout after 30s"),
                Some(30000),
            )
            .unwrap();
        let calls = db
            .tools()
            .get_tool_calls(Some("session-1"), None, None)
            .unwrap();
        assert_eq!(calls[0].error.as_deref(), Some("timeout after 30s"));
        assert_eq!(calls[0].latency_ms, Some(30000));
    }

    // ── get_tool_calls ───────────────────────────────────────────────────────

    #[test]
    fn test_get_tool_calls_by_session() {
        let db = open();
        let store = db.tools();
        store
            .log_tool_call(Some("s1"), "search", None, None, None, None)
            .unwrap();
        store
            .log_tool_call(Some("s2"), "search", None, None, None, None)
            .unwrap();
        let calls = store.get_tool_calls(Some("s1"), None, None).unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn test_get_tool_calls_by_tool_name() {
        let db = open();
        let store = db.tools();
        store
            .log_tool_call(Some("s1"), "search", None, None, None, None)
            .unwrap();
        store
            .log_tool_call(Some("s1"), "calc", None, None, None, None)
            .unwrap();
        let calls = store.get_tool_calls(None, Some("calc"), None).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "calc");
    }

    #[test]
    fn test_get_tool_calls_limit() {
        let db = open();
        let store = db.tools();
        for _ in 0..10 {
            store
                .log_tool_call(Some("s1"), "ping", None, None, None, None)
                .unwrap();
        }
        let calls = store.get_tool_calls(Some("s1"), None, Some(3)).unwrap();
        assert_eq!(calls.len(), 3);
    }

    #[test]
    fn test_get_tool_calls_desc_order() {
        let db = open();
        let store = db.tools();
        store
            .log_tool_call(Some("s1"), "a", None, None, None, None)
            .unwrap();
        store
            .log_tool_call(Some("s1"), "b", None, None, None, None)
            .unwrap();
        let calls = store.get_tool_calls(Some("s1"), None, None).unwrap();
        assert!(calls[0].created_at >= calls[1].created_at);
    }

    // ── stats integration ────────────────────────────────────────────────────

    #[test]
    fn test_stats_counts_tools_and_calls() {
        let db = open();
        let store = db.tools();
        store.register_tool("a", None, None, None).unwrap();
        store.register_tool("b", None, None, None).unwrap();
        store
            .log_tool_call(None, "a", None, None, None, None)
            .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.tools, 2);
        assert_eq!(stats.tool_calls, 1);
    }
}
