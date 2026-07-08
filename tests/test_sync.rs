//! Integration tests for the sync module (HLC + SyncEngine).

#[cfg(test)]
mod tests {
    use agentdb::sync::{ConflictStrategy, HybridClock, OpType, SyncEngine, SyncOp};
    use agentdb::AgentDB;
    use serde_json::json;

    fn open_db() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── HybridClock ───────────────────────────────────────────────────────────

    #[test]
    fn test_hlc_monotonicity() {
        let mut clock = HybridClock::new("node-a");
        let t0 = clock.now();
        let t1 = clock.now();
        let t2 = clock.now();
        assert!(t1 > t0, "successive HLC calls must be strictly increasing");
        assert!(t2 > t1, "successive HLC calls must be strictly increasing");
    }

    #[test]
    fn test_hlc_update_advances_past_remote() {
        let mut clock = HybridClock::new("node-b");
        // Seed our clock.
        let local_ts = clock.now();

        // Simulate a remote timestamp far in the future.
        let future_ms: i64 = 9_999_999_999_999_i64; // huge physical ms
        let remote_ts = future_ms << 16; // logical = 0

        clock.update(remote_ts);

        // After update, the next timestamp must be > remote_ts.
        let after = clock.now();
        assert!(
            after > remote_ts,
            "clock should advance past remote ts: after={after} remote={remote_ts}"
        );
        // Sanity: the original local ts was much smaller.
        assert!(remote_ts > local_ts);
    }

    #[test]
    fn test_hlc_update_with_older_remote_does_not_regress() {
        let mut clock = HybridClock::new("node-c");
        let t0 = clock.now();
        // Feed a remote timestamp that's older than our current physical time.
        clock.update(0); // epoch — always in the past
        let t1 = clock.now();
        assert!(
            t1 > t0,
            "clock must not regress after receiving an old remote ts"
        );
    }

    // ── record_mutation / get_ops_since ───────────────────────────────────────

    #[test]
    fn test_record_mutation_is_retrievable() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-1").unwrap();

        let op_id = engine
            .record_mutation(
                "my_table",
                "row-1",
                OpType::Insert,
                Some(json!({"name": "Alice"})),
            )
            .unwrap();

        assert!(!op_id.is_empty(), "op_id should be a non-empty UUID string");

        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        let op = &ops[0];
        assert_eq!(op.op_id, op_id);
        assert_eq!(op.table_name, "my_table");
        assert_eq!(op.record_id, "row-1");
        assert_eq!(op.op_type, OpType::Insert);
        assert_eq!(op.payload.as_ref().unwrap()["name"], "Alice");
        assert_eq!(op.node_id, "node-1");
    }

    #[test]
    fn test_get_ops_since_filters_by_hlc() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-1").unwrap();

        engine
            .record_mutation("t", "r1", OpType::Insert, None)
            .unwrap();
        let mid_ops = engine.get_ops_since(0).unwrap();
        let watermark = mid_ops[0].hlc_ts;

        engine
            .record_mutation("t", "r2", OpType::Insert, None)
            .unwrap();
        engine
            .record_mutation("t", "r3", OpType::Insert, None)
            .unwrap();

        let later = engine.get_ops_since(watermark).unwrap();
        assert_eq!(
            later.len(),
            2,
            "only ops after watermark should be returned"
        );
        for op in &later {
            assert!(op.hlc_ts > watermark);
        }
    }

    #[test]
    fn test_record_mutation_delete_no_payload() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-1").unwrap();
        engine
            .record_mutation("users", "u-99", OpType::Delete, None)
            .unwrap();
        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_type, OpType::Delete);
        assert!(ops[0].payload.is_none());
    }

    // ── apply_remote_ops — no conflict ────────────────────────────────────────

    #[test]
    fn test_apply_remote_ops_no_conflict() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();

        let remote_op = SyncOp {
            op_id: "op-remote-1".to_string(),
            hlc_ts: 100 << 16, // arbitrary future ts
            node_id: "node-remote".to_string(),
            table_name: "items".to_string(),
            record_id: "item-1".to_string(),
            op_type: OpType::Insert,
            payload: Some(json!({"value": 42})),
        };

        let result = engine.apply_remote_ops(vec![remote_op]).unwrap();
        assert_eq!(result.applied, 1);
        assert_eq!(result.conflicts, 0);
        assert_eq!(result.skipped, 0);

        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_id, "op-remote-1");
    }

    #[test]
    fn test_apply_multiple_remote_ops_no_conflict() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();

        let remote_ops: Vec<SyncOp> = (0..5)
            .map(|i| SyncOp {
                op_id: format!("op-{i}"),
                hlc_ts: (100 + i) << 16,
                node_id: "node-remote".to_string(),
                table_name: "tbl".to_string(),
                record_id: format!("row-{i}"),
                op_type: OpType::Insert,
                payload: None,
            })
            .collect();

        let result = engine.apply_remote_ops(remote_ops).unwrap();
        assert_eq!(result.applied, 5);
        assert_eq!(result.conflicts, 0);
        assert_eq!(result.skipped, 0);
    }

    // ── apply_remote_ops — LWW conflict resolution ────────────────────────────

    #[test]
    fn test_lww_incoming_higher_ts_wins() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();
        engine.set_strategy(ConflictStrategy::LastWriterWins);

        // Insert a local op at ts=10.
        let local_op = SyncOp {
            op_id: "op-local".to_string(),
            hlc_ts: 10 << 16,
            node_id: "node-local".to_string(),
            table_name: "docs".to_string(),
            record_id: "doc-1".to_string(),
            op_type: OpType::Insert,
            payload: Some(json!({"v": "local"})),
        };
        engine.apply_remote_ops(vec![local_op]).unwrap();

        // Incoming remote op at ts=20 (higher) — should win.
        let remote_op = SyncOp {
            op_id: "op-remote".to_string(),
            hlc_ts: 20 << 16,
            node_id: "node-remote".to_string(),
            table_name: "docs".to_string(),
            record_id: "doc-1".to_string(),
            op_type: OpType::Update,
            payload: Some(json!({"v": "remote"})),
        };
        let result = engine.apply_remote_ops(vec![remote_op]).unwrap();
        assert_eq!(result.applied, 1);
        assert_eq!(result.conflicts, 1);
        assert_eq!(result.skipped, 0);

        // Verify the winning op is the remote one.
        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_id, "op-remote");
        assert_eq!(ops[0].payload.as_ref().unwrap()["v"], "remote");
    }

    #[test]
    fn test_lww_incoming_lower_ts_loses() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();
        engine.set_strategy(ConflictStrategy::LastWriterWins);

        // Local op at ts=50.
        let local_op = SyncOp {
            op_id: "op-local-high".to_string(),
            hlc_ts: 50 << 16,
            node_id: "node-local".to_string(),
            table_name: "docs".to_string(),
            record_id: "doc-2".to_string(),
            op_type: OpType::Insert,
            payload: Some(json!({"v": "local-high"})),
        };
        engine.apply_remote_ops(vec![local_op]).unwrap();

        // Remote op at ts=5 (lower) — should lose.
        let remote_op = SyncOp {
            op_id: "op-remote-old".to_string(),
            hlc_ts: 5 << 16,
            node_id: "node-remote".to_string(),
            table_name: "docs".to_string(),
            record_id: "doc-2".to_string(),
            op_type: OpType::Update,
            payload: Some(json!({"v": "remote-old"})),
        };
        let result = engine.apply_remote_ops(vec![remote_op]).unwrap();
        assert_eq!(result.applied, 0);
        assert_eq!(result.conflicts, 1);
        assert_eq!(result.skipped, 1);

        // Existing (local-high) op should remain unchanged.
        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_id, "op-local-high");
    }

    // ── apply_remote_ops — FWW conflict resolution ────────────────────────────

    #[test]
    fn test_fww_incoming_lower_ts_wins() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();
        engine.set_strategy(ConflictStrategy::FirstWriterWins);

        // Local op at ts=50.
        let local_op = SyncOp {
            op_id: "op-local-late".to_string(),
            hlc_ts: 50 << 16,
            node_id: "node-local".to_string(),
            table_name: "notes".to_string(),
            record_id: "note-1".to_string(),
            op_type: OpType::Insert,
            payload: Some(json!({"text": "late local"})),
        };
        engine.apply_remote_ops(vec![local_op]).unwrap();

        // Remote op at ts=5 (earlier) — should win under FWW.
        let remote_op = SyncOp {
            op_id: "op-remote-early".to_string(),
            hlc_ts: 5 << 16,
            node_id: "node-remote".to_string(),
            table_name: "notes".to_string(),
            record_id: "note-1".to_string(),
            op_type: OpType::Insert,
            payload: Some(json!({"text": "early remote"})),
        };
        let result = engine.apply_remote_ops(vec![remote_op]).unwrap();
        assert_eq!(result.applied, 1);
        assert_eq!(result.conflicts, 1);
        assert_eq!(result.skipped, 0);

        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op_id, "op-remote-early");
        assert_eq!(ops[0].payload.as_ref().unwrap()["text"], "early remote");
    }

    // ── Peer management ───────────────────────────────────────────────────────

    #[test]
    fn test_add_peer_and_query_status() {
        let db = open_db();
        let engine = SyncEngine::new(&db, "node-local").unwrap();

        engine
            .add_peer("peer-alpha", Some("https://alpha.example.com/sync"))
            .unwrap();
        engine.add_peer("peer-beta", None).unwrap();

        let statuses = engine.sync_status().unwrap();
        assert_eq!(statuses.len(), 2);

        let alpha = statuses.iter().find(|s| s.peer_id == "peer-alpha").unwrap();
        assert_eq!(
            alpha.endpoint.as_deref(),
            Some("https://alpha.example.com/sync")
        );
        assert_eq!(alpha.last_synced_hlc, 0);

        let beta = statuses.iter().find(|s| s.peer_id == "peer-beta").unwrap();
        assert!(beta.endpoint.is_none());
    }

    #[test]
    fn test_ops_pending_counts_unsynced_ops() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "node-local").unwrap();

        engine.add_peer("peer-x", None).unwrap();

        // No ops yet — pending should be 0.
        let statuses = engine.sync_status().unwrap();
        assert_eq!(statuses[0].ops_pending, 0);

        // Record some mutations.
        engine
            .record_mutation("t", "r1", OpType::Insert, None)
            .unwrap();
        engine
            .record_mutation("t", "r2", OpType::Insert, None)
            .unwrap();
        engine
            .record_mutation("t", "r3", OpType::Update, None)
            .unwrap();

        let statuses = engine.sync_status().unwrap();
        assert_eq!(
            statuses[0].ops_pending, 3,
            "all three ops are pending for peer-x"
        );
    }

    #[test]
    fn test_add_peer_idempotent_update() {
        let db = open_db();
        let engine = SyncEngine::new(&db, "node-local").unwrap();

        engine
            .add_peer("peer-1", Some("http://old.endpoint"))
            .unwrap();
        engine
            .add_peer("peer-1", Some("http://new.endpoint"))
            .unwrap();

        let statuses = engine.sync_status().unwrap();
        assert_eq!(
            statuses.len(),
            1,
            "duplicate add_peer should not double the row"
        );
        assert_eq!(
            statuses[0].endpoint.as_deref(),
            Some("http://new.endpoint"),
            "endpoint should be updated on re-add"
        );
    }

    // ── SyncEngine on a real record_mutation clock ────────────────────────────

    #[test]
    fn test_ops_node_id_matches_engine_node() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "my-unique-node").unwrap();
        engine
            .record_mutation("t", "r1", OpType::Insert, None)
            .unwrap();
        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops[0].node_id, "my-unique-node");
    }

    #[test]
    fn test_hlc_ts_increases_across_record_mutation_calls() {
        let db = open_db();
        let mut engine = SyncEngine::new(&db, "n").unwrap();
        engine
            .record_mutation("t", "r1", OpType::Insert, None)
            .unwrap();
        engine
            .record_mutation("t", "r2", OpType::Insert, None)
            .unwrap();
        engine
            .record_mutation("t", "r3", OpType::Insert, None)
            .unwrap();
        let ops = engine.get_ops_since(0).unwrap();
        assert_eq!(ops.len(), 3);
        // Ops are ordered by hlc_ts ASC — verify they're strictly increasing.
        assert!(
            ops[0].hlc_ts < ops[1].hlc_ts,
            "op 0 ts {} should be < op 1 ts {}",
            ops[0].hlc_ts,
            ops[1].hlc_ts
        );
        assert!(
            ops[1].hlc_ts < ops[2].hlc_ts,
            "op 1 ts {} should be < op 2 ts {}",
            ops[1].hlc_ts,
            ops[2].hlc_ts
        );
    }
}
