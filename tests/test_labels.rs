#[cfg(test)]
mod tests {
    use agentdb::AgentDB;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── tag ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_tag_succeeds() {
        let db = open();
        db.labels()
            .tag("_adb_messages", "msg-1", "pii", Some("agent-1"))
            .unwrap();
    }

    #[test]
    fn test_tag_without_tagged_by() {
        let db = open();
        db.labels()
            .tag("_adb_nodes", "n-1", "sensitive", None)
            .unwrap();
        let labels = db.labels().get_labels("_adb_nodes", "n-1").unwrap();
        assert_eq!(labels[0].tagged_by, None);
    }

    #[test]
    fn test_tag_upsert_on_duplicate() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "rec", "secret", Some("alice")).unwrap();
        store.tag("tbl", "rec", "secret", Some("bob")).unwrap();
        let labels = store.get_labels("tbl", "rec").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].tagged_by.as_deref(), Some("bob"));
    }

    #[test]
    fn test_multiple_labels_on_same_record() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "rec", "pii", None).unwrap();
        store.tag("tbl", "rec", "sensitive", None).unwrap();
        store.tag("tbl", "rec", "internal", None).unwrap();
        let labels = store.get_labels("tbl", "rec").unwrap();
        assert_eq!(labels.len(), 3);
    }

    // ── untag ────────────────────────────────────────────────────────────────

    #[test]
    fn test_untag_removes_specific_label() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "rec", "pii", None).unwrap();
        store.tag("tbl", "rec", "internal", None).unwrap();
        store.untag("tbl", "rec", "pii").unwrap();
        let labels = store.get_labels("tbl", "rec").unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].label, "internal");
    }

    #[test]
    fn test_untag_nonexistent_no_error() {
        let db = open();
        db.labels().untag("tbl", "rec", "ghost").unwrap();
    }

    // ── get_labels ───────────────────────────────────────────────────────────

    #[test]
    fn test_get_labels_empty() {
        let db = open();
        let labels = db.labels().get_labels("tbl", "no-such-record").unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn test_get_labels_field_correctness() {
        let db = open();
        db.labels()
            .tag("my_table", "record-42", "classified", Some("admin"))
            .unwrap();
        let labels = db.labels().get_labels("my_table", "record-42").unwrap();
        let l = &labels[0];
        assert_eq!(l.table_name, "my_table");
        assert_eq!(l.record_id, "record-42");
        assert_eq!(l.label, "classified");
        assert_eq!(l.tagged_by.as_deref(), Some("admin"));
        assert!(l.tagged_at > 0);
    }

    // ── find_by_label ────────────────────────────────────────────────────────

    #[test]
    fn test_find_by_label() {
        let db = open();
        let store = db.labels();
        store.tag("tbl_a", "r1", "pii", None).unwrap();
        store.tag("tbl_a", "r2", "pii", None).unwrap();
        store.tag("tbl_b", "r3", "pii", None).unwrap();
        store.tag("tbl_a", "r1", "internal", None).unwrap();
        let pii_labels = store.find_by_label("pii", None).unwrap();
        assert_eq!(pii_labels.len(), 3);
        for l in &pii_labels {
            assert_eq!(l.label, "pii");
        }
    }

    #[test]
    fn test_find_by_label_with_limit() {
        let db = open();
        let store = db.labels();
        for i in 0..10 {
            store.tag("tbl", &format!("r{i}"), "tag", None).unwrap();
        }
        let results = store.find_by_label("tag", Some(3)).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_by_label_not_found() {
        let db = open();
        let results = db.labels().find_by_label("nonexistent", None).unwrap();
        assert!(results.is_empty());
    }

    // ── has_label ────────────────────────────────────────────────────────────

    #[test]
    fn test_has_label_true() {
        let db = open();
        db.labels().tag("tbl", "rec", "secret", None).unwrap();
        assert!(db.labels().has_label("tbl", "rec", "secret").unwrap());
    }

    #[test]
    fn test_has_label_false() {
        let db = open();
        assert!(!db.labels().has_label("tbl", "rec", "secret").unwrap());
    }

    #[test]
    fn test_has_label_after_untag() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "rec", "pii", None).unwrap();
        store.untag("tbl", "rec", "pii").unwrap();
        assert!(!store.has_label("tbl", "rec", "pii").unwrap());
    }

    // ── clear_record ─────────────────────────────────────────────────────────

    #[test]
    fn test_clear_record_removes_all_labels() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "rec", "a", None).unwrap();
        store.tag("tbl", "rec", "b", None).unwrap();
        store.tag("tbl", "rec", "c", None).unwrap();
        store.clear_record("tbl", "rec").unwrap();
        let labels = store.get_labels("tbl", "rec").unwrap();
        assert!(labels.is_empty());
    }

    #[test]
    fn test_clear_record_does_not_affect_other_records() {
        let db = open();
        let store = db.labels();
        store.tag("tbl", "r1", "tag", None).unwrap();
        store.tag("tbl", "r2", "tag", None).unwrap();
        store.clear_record("tbl", "r1").unwrap();
        assert!(store.has_label("tbl", "r2", "tag").unwrap());
    }

    #[test]
    fn test_clear_nonexistent_record_no_error() {
        let db = open();
        db.labels().clear_record("tbl", "ghost").unwrap();
    }
}
