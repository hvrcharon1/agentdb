#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── log ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_audit_log_returns_id() {
        let db = open();
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
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_audit_log_with_old_and_new_values() {
        let db = open();
        db.audit()
            .log(
                Some("agent-1"),
                "update",
                "_adb_nodes",
                "node-1",
                Some(json!({"status": "active"})),
                Some(json!({"status": "archived"})),
                Some("user requested archive"),
            )
            .unwrap();
        let entries = db
            .audit()
            .query_by_record("_adb_nodes", "node-1", None)
            .unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.actor.as_deref(), Some("agent-1"));
        assert_eq!(e.action, "update");
        assert_eq!(e.old_value.as_ref().unwrap()["status"], "active");
        assert_eq!(e.new_value.as_ref().unwrap()["status"], "archived");
        assert_eq!(e.reason.as_deref(), Some("user requested archive"));
    }

    #[test]
    fn test_audit_log_null_actor() {
        let db = open();
        db.audit()
            .log(None, "delete", "_adb_edges", "edge-1", None, None, None)
            .unwrap();
        let entries = db.audit().query_recent(None).unwrap();
        assert_eq!(entries[0].actor, None);
    }

    // ── query_by_record ──────────────────────────────────────────────────────

    #[test]
    fn test_query_by_record_filters_correctly() {
        let db = open();
        let audit = db.audit();
        audit
            .log(None, "insert", "table_a", "r1", None, None, None)
            .unwrap();
        audit
            .log(None, "insert", "table_a", "r2", None, None, None)
            .unwrap();
        audit
            .log(None, "insert", "table_b", "r1", None, None, None)
            .unwrap();
        let results = audit.query_by_record("table_a", "r1", None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].table_name, "table_a");
        assert_eq!(results[0].record_id, "r1");
    }

    #[test]
    fn test_query_by_record_with_limit() {
        let db = open();
        let audit = db.audit();
        for _ in 0..5 {
            audit
                .log(None, "touch", "tbl", "rec", None, None, None)
                .unwrap();
        }
        let results = audit.query_by_record("tbl", "rec", Some(2)).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_by_record_desc_order() {
        let db = open();
        let audit = db.audit();
        audit
            .log(None, "first", "tbl", "rec", None, None, None)
            .unwrap();
        audit
            .log(None, "second", "tbl", "rec", None, None, None)
            .unwrap();
        let results = audit.query_by_record("tbl", "rec", None).unwrap();
        assert!(results[0].timestamp >= results[1].timestamp);
    }

    // ── query_by_actor ───────────────────────────────────────────────────────

    #[test]
    fn test_query_by_actor() {
        let db = open();
        let audit = db.audit();
        audit
            .log(Some("alice"), "insert", "t", "r1", None, None, None)
            .unwrap();
        audit
            .log(Some("bob"), "insert", "t", "r2", None, None, None)
            .unwrap();
        audit
            .log(Some("alice"), "update", "t", "r1", None, None, None)
            .unwrap();
        let alice_entries = audit.query_by_actor("alice", None).unwrap();
        assert_eq!(alice_entries.len(), 2);
        for e in &alice_entries {
            assert_eq!(e.actor.as_deref(), Some("alice"));
        }
    }

    #[test]
    fn test_query_by_actor_with_limit() {
        let db = open();
        let audit = db.audit();
        for _ in 0..10 {
            audit
                .log(Some("bot"), "ping", "t", "r", None, None, None)
                .unwrap();
        }
        let results = audit.query_by_actor("bot", Some(3)).unwrap();
        assert_eq!(results.len(), 3);
    }

    // ── query_recent ─────────────────────────────────────────────────────────

    #[test]
    fn test_query_recent() {
        let db = open();
        let audit = db.audit();
        audit
            .log(Some("a"), "insert", "t", "1", None, None, None)
            .unwrap();
        audit
            .log(Some("b"), "update", "t", "2", None, None, None)
            .unwrap();
        audit
            .log(Some("c"), "delete", "t", "3", None, None, None)
            .unwrap();
        let recent = audit.query_recent(Some(2)).unwrap();
        assert_eq!(recent.len(), 2);
        assert!(recent[0].timestamp >= recent[1].timestamp);
    }

    #[test]
    fn test_query_recent_empty() {
        let db = open();
        let recent = db.audit().query_recent(None).unwrap();
        assert!(recent.is_empty());
    }

    // ── stats integration ────────────────────────────────────────────────────

    #[test]
    fn test_stats_counts_audit_entries() {
        let db = open();
        let audit = db.audit();
        audit.log(None, "a", "t", "1", None, None, None).unwrap();
        audit.log(None, "b", "t", "2", None, None, None).unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.audit_entries, 2);
    }
}
