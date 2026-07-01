#[cfg(test)]
mod tests {
    use agentdb::AgentDB;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── add_entry ────────────────────────────────────────────────────────────

    #[test]
    fn test_add_entry_returns_id() {
        let db = open();
        let id = db
            .context()
            .add_entry(
                "session-1",
                "message",
                "msg-001",
                Some("Hello world"),
                5,
                0.9,
                10,
            )
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_add_entry_without_preview() {
        let db = open();
        let id = db
            .context()
            .add_entry("session-1", "vector", "vec-001", None, 100, 0.5, 5)
            .unwrap();
        assert!(!id.is_empty());
    }

    // ── get_entries ──────────────────────────────────────────────────────────

    #[test]
    fn test_get_entries_returns_session_entries() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "msg", "m1", Some("hi"), 10, 0.8, 5)
            .unwrap();
        ctx.add_entry("s1", "msg", "m2", Some("bye"), 10, 0.7, 3)
            .unwrap();
        ctx.add_entry("s2", "msg", "m3", Some("other"), 10, 0.9, 1)
            .unwrap();
        let entries = ctx.get_entries("s1").unwrap();
        assert_eq!(entries.len(), 2);
        for e in &entries {
            assert_eq!(e.session_id, "s1");
        }
    }

    #[test]
    fn test_get_entries_ordered_by_priority_then_relevance() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).unwrap();
        ctx.add_entry("s1", "b", "2", None, 10, 0.9, 10).unwrap();
        ctx.add_entry("s1", "c", "3", None, 10, 0.8, 10).unwrap();
        let entries = ctx.get_entries("s1").unwrap();
        assert_eq!(entries[0].priority, 10);
        assert!(entries[0].relevance_score >= entries[1].relevance_score);
        assert_eq!(entries[2].priority, 1);
    }

    #[test]
    fn test_get_entries_nonexistent_session_empty() {
        let db = open();
        let entries = db.context().get_entries("ghost").unwrap();
        assert!(entries.is_empty());
    }

    // ── build_window (token-budgeted) ────────────────────────────────────────

    #[test]
    fn test_build_window_respects_max_tokens() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "a", "1", None, 50, 0.9, 10).unwrap();
        ctx.add_entry("s1", "b", "2", None, 50, 0.8, 9).unwrap();
        ctx.add_entry("s1", "c", "3", None, 50, 0.7, 8).unwrap();
        let window = ctx.build_window("s1", 100).unwrap();
        assert_eq!(window.len(), 2);
        let total_tokens: i64 = window.iter().map(|e| e.token_count).sum();
        assert!(total_tokens <= 100);
    }

    #[test]
    fn test_build_window_prioritizes_high_priority() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "low", "1", None, 30, 0.9, 1).unwrap();
        ctx.add_entry("s1", "high", "2", None, 30, 0.5, 100)
            .unwrap();
        let window = ctx.build_window("s1", 40).unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].source_type, "high");
    }

    #[test]
    fn test_build_window_skips_large_entries() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "big", "1", None, 1000, 0.9, 10)
            .unwrap();
        ctx.add_entry("s1", "small", "2", None, 10, 0.8, 5).unwrap();
        let window = ctx.build_window("s1", 50).unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0].source_type, "small");
    }

    #[test]
    fn test_build_window_empty_session() {
        let db = open();
        let window = db.context().build_window("empty", 1000).unwrap();
        assert!(window.is_empty());
    }

    #[test]
    fn test_build_window_zero_budget() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "a", "1", None, 1, 0.9, 10).unwrap();
        let window = ctx.build_window("s1", 0).unwrap();
        assert!(window.is_empty());
    }

    // ── clear_session ────────────────────────────────────────────────────────

    #[test]
    fn test_clear_session() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).unwrap();
        ctx.add_entry("s1", "b", "2", None, 10, 0.5, 1).unwrap();
        ctx.clear_session("s1").unwrap();
        let entries = ctx.get_entries("s1").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_clear_session_does_not_affect_other_sessions() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).unwrap();
        ctx.add_entry("s2", "b", "2", None, 10, 0.5, 1).unwrap();
        ctx.clear_session("s1").unwrap();
        let entries = ctx.get_entries("s2").unwrap();
        assert_eq!(entries.len(), 1);
    }

    // ── remove_entry ─────────────────────────────────────────────────────────

    #[test]
    fn test_remove_entry() {
        let db = open();
        let ctx = db.context();
        let id = ctx.add_entry("s1", "a", "1", None, 10, 0.5, 1).unwrap();
        ctx.add_entry("s1", "b", "2", None, 10, 0.5, 1).unwrap();
        ctx.remove_entry(&id).unwrap();
        let entries = ctx.get_entries("s1").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_type, "b");
    }

    #[test]
    fn test_remove_nonexistent_entry_no_error() {
        let db = open();
        db.context().remove_entry("ghost-id").unwrap();
    }

    // ── field correctness ────────────────────────────────────────────────────

    #[test]
    fn test_entry_fields_stored_correctly() {
        let db = open();
        let ctx = db.context();
        ctx.add_entry(
            "sess-x",
            "vector_result",
            "vec-42",
            Some("preview text"),
            256,
            0.87,
            7,
        )
        .unwrap();
        let entries = ctx.get_entries("sess-x").unwrap();
        let e = &entries[0];
        assert_eq!(e.session_id, "sess-x");
        assert_eq!(e.source_type, "vector_result");
        assert_eq!(e.source_id, "vec-42");
        assert_eq!(e.content_preview.as_deref(), Some("preview text"));
        assert_eq!(e.token_count, 256);
        assert!((e.relevance_score - 0.87).abs() < 0.001);
        assert_eq!(e.priority, 7);
        assert!(e.included_at > 0);
    }
}
