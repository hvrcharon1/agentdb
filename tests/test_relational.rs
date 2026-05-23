#[cfg(test)]
mod tests {
    use agentdb::AgentDB;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    #[test]
    fn test_open_in_memory() {
        let db = open();
        let stats = db.stats().unwrap();
        assert_eq!(stats.collections, 0);
        assert_eq!(stats.nodes, 0);
        assert_eq!(stats.edges, 0);
    }

    #[test]
    fn test_create_table_and_insert() {
        let db = open();
        db.execute("CREATE TABLE test (id TEXT PRIMARY KEY, val TEXT)")
            .unwrap();
        let changed = db
            .execute_params("INSERT INTO test VALUES (?1, ?2)", &[&"row_1", &"hello"])
            .unwrap();
        assert_eq!(changed, 1);
    }

    #[test]
    fn test_query_json_returns_rows() {
        let db = open();
        db.execute("CREATE TABLE items (id TEXT PRIMARY KEY, name TEXT)")
            .unwrap();
        db.execute_params("INSERT INTO items VALUES (?1, ?2)", &[&"a", &"alpha"])
            .unwrap();
        db.execute_params("INSERT INTO items VALUES (?1, ?2)", &[&"b", &"beta"])
            .unwrap();
        let rows = db.query_json("SELECT * FROM items ORDER BY id").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "a");
        assert_eq!(rows[1]["name"], "beta");
    }

    #[test]
    fn test_multiple_tables_coexist() {
        let db = open();
        db.execute("CREATE TABLE events (id TEXT PRIMARY KEY, kind TEXT)")
            .unwrap();
        db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, user TEXT)")
            .unwrap();
        db.execute_params("INSERT INTO events VALUES (?1, ?2)", &[&"e1", &"msg"])
            .unwrap();
        db.execute_params("INSERT INTO sessions VALUES (?1, ?2)", &[&"s1", &"harshal"])
            .unwrap();
        let events = db.query_json("SELECT * FROM events").unwrap();
        let sessions = db.query_json("SELECT * FROM sessions").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_query_empty_table_returns_empty() {
        let db = open();
        db.execute("CREATE TABLE empty_table (id TEXT PRIMARY KEY)")
            .unwrap();
        let rows = db.query_json("SELECT * FROM empty_table").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_update_and_requery() {
        let db = open();
        db.execute("CREATE TABLE kv (key TEXT PRIMARY KEY, val TEXT)")
            .unwrap();
        db.execute_params("INSERT INTO kv VALUES (?1, ?2)", &[&"x", &"old"])
            .unwrap();
        db.execute_params("UPDATE kv SET val = ?1 WHERE key = ?2", &[&"new", &"x"])
            .unwrap();
        let rows = db.query_json("SELECT val FROM kv WHERE key = 'x'").unwrap();
        assert_eq!(rows[0]["val"], "new");
    }

    #[test]
    fn test_user_tables_dont_conflict_with_internal() {
        let db = open();
        db.execute("CREATE TABLE my_app_data (id TEXT PRIMARY KEY)")
            .unwrap();
        let rows = db.query_json("SELECT * FROM my_app_data").unwrap();
        assert!(rows.is_empty());
    }
}
