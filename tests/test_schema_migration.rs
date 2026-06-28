use agentdb::AgentDB;

#[test]
fn schema_version_matches_current() {
    let db = AgentDB::open(":memory:").unwrap();
    let rows = db
        .query_json("SELECT value FROM _adb_meta WHERE key = 'schema_version'")
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["value"], "2");
}

#[test]
fn schema_version_mismatch_errors() {
    // Create a database, then tamper with the schema version to simulate
    // opening an older/newer schema.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.agentdb");
    let path_str = path.to_str().unwrap();

    // First, create a valid database
    {
        let _db = AgentDB::open(path_str).unwrap();
    }

    // Tamper with the schema version
    {
        let conn = rusqlite::Connection::open(path_str).unwrap();
        conn.execute_batch("UPDATE _adb_meta SET value = '999' WHERE key = 'schema_version'")
            .unwrap();
    }

    // Re-opening should fail with SchemaMigration error
    let result = AgentDB::open(path_str);
    match result {
        Ok(_) => panic!("Expected SchemaMigration error, got Ok"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("Schema version mismatch"),
                "Expected SchemaMigration error, got: {msg}"
            );
        }
    }
}

#[test]
fn schema_created_at_is_set() {
    let db = AgentDB::open(":memory:").unwrap();
    let rows = db
        .query_json("SELECT value FROM _adb_meta WHERE key = 'created_at'")
        .unwrap();
    assert_eq!(rows.len(), 1);
    let ts = rows[0]["value"].as_str().unwrap();
    let ms: i64 = ts.parse().unwrap();
    assert!(ms > 1_700_000_000_000); // sanity: after 2023
}

#[test]
fn schema_all_tables_exist() {
    let db = AgentDB::open(":memory:").unwrap();
    let expected_tables = [
        "_adb_meta",
        "_adb_collections",
        "_adb_vectors",
        "_adb_hnsw_index",
        "_adb_nodes",
        "_adb_edges",
        "_adb_conversations",
        "_adb_messages",
        "_adb_workflows",
        "_adb_workflow_steps",
        "_adb_traces",
    ];

    for table in expected_tables {
        let rows = db
            .query_json(&format!(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'"
            ))
            .unwrap();
        assert_eq!(rows.len(), 1, "Table {table} should exist");
    }
}
