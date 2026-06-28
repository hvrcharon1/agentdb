use agentdb::AgentDB;

#[test]
fn transaction_commits_on_success() {
    let db = AgentDB::open(":memory:").unwrap();
    db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .unwrap();

    db.transaction(|tx| {
        tx.execute("INSERT INTO t (id, val) VALUES (1, 'a')", [])?;
        tx.execute("INSERT INTO t (id, val) VALUES (2, 'b')", [])?;
        Ok(())
    })
    .unwrap();

    let rows = db.query_json("SELECT * FROM t ORDER BY id").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["val"], "a");
    assert_eq!(rows[1]["val"], "b");
}

#[test]
fn transaction_rolls_back_on_error() {
    let db = AgentDB::open(":memory:").unwrap();
    db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .unwrap();
    db.execute("INSERT INTO t (id, val) VALUES (1, 'existing')")
        .unwrap();

    let result = db.transaction(|tx| {
        tx.execute("INSERT INTO t (id, val) VALUES (2, 'new')", [])?;
        // This will fail: duplicate primary key
        tx.execute("INSERT INTO t (id, val) VALUES (1, 'dup')", [])?;
        Ok(())
    });

    assert!(result.is_err());

    let rows = db.query_json("SELECT * FROM t").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["val"], "existing");
}

#[test]
fn transaction_returns_value() {
    let db = AgentDB::open(":memory:").unwrap();
    db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .unwrap();

    let count: usize = db
        .transaction(|tx| {
            tx.execute("INSERT INTO t (id, val) VALUES (1, 'x')", [])?;
            tx.execute("INSERT INTO t (id, val) VALUES (2, 'y')", [])?;
            let n: i64 = tx.query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))?;
            Ok(n as usize)
        })
        .unwrap();

    assert_eq!(count, 2);
}

#[test]
fn execute_batch_atomic() {
    let db = AgentDB::open(":memory:").unwrap();
    db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .unwrap();

    db.execute_batch(
        "INSERT INTO t (id, val) VALUES (1, 'first');
         INSERT INTO t (id, val) VALUES (2, 'second');
         INSERT INTO t (id, val) VALUES (3, 'third');",
    )
    .unwrap();

    let rows = db.query_json("SELECT * FROM t ORDER BY id").unwrap();
    assert_eq!(rows.len(), 3);
}

#[test]
fn execute_batch_rolls_back_on_error() {
    let db = AgentDB::open(":memory:").unwrap();
    db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, val TEXT)")
        .unwrap();
    db.execute("INSERT INTO t (id, val) VALUES (1, 'existing')")
        .unwrap();

    let result = db.execute_batch(
        "INSERT INTO t (id, val) VALUES (2, 'new');
         INSERT INTO t (id, val) VALUES (1, 'duplicate');",
    );

    assert!(result.is_err());

    let rows = db.query_json("SELECT * FROM t").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["val"], "existing");
}
