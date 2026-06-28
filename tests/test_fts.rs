use agentdb::AgentDB;

#[test]
fn fts_index_and_search() {
    let db = AgentDB::open(":memory:").unwrap();
    let fts = db.fts();
    fts.index_text(
        "docs",
        "d1",
        "col1",
        "The quick brown fox jumps over the lazy dog",
    )
    .unwrap();
    fts.index_text(
        "docs",
        "d2",
        "col1",
        "A fast red car drove past the sleeping cat",
    )
    .unwrap();

    let results = fts.search("docs", "quick fox", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "d1");
    assert!(results[0].snippet.contains("quick"));
}

#[test]
fn fts_search_empty_collection() {
    let db = AgentDB::open(":memory:").unwrap();
    let results = db.fts().search("nonexistent", "hello", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn fts_delete_text() {
    let db = AgentDB::open(":memory:").unwrap();
    let fts = db.fts();
    fts.index_text("notes", "n1", "col1", "Important meeting tomorrow")
        .unwrap();
    fts.index_text("notes", "n2", "col1", "Meeting cancelled")
        .unwrap();

    let before = fts.search("notes", "meeting", 10).unwrap();
    assert_eq!(before.len(), 2);

    fts.delete_text("notes", "n1").unwrap();

    let after = fts.search("notes", "meeting", 10).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, "n2");
}

#[test]
fn fts_optimize() {
    let db = AgentDB::open(":memory:").unwrap();
    let fts = db.fts();
    fts.index_text("opt", "o1", "col1", "Optimization test document")
        .unwrap();
    fts.optimize("opt").unwrap();

    let results = fts.search("opt", "optimization", 10).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn fts_upsert_replaces_existing() {
    let db = AgentDB::open(":memory:").unwrap();
    let fts = db.fts();
    fts.index_text("docs", "d1", "col1", "Original content here")
        .unwrap();
    fts.index_text("docs", "d1", "col1", "Updated replacement content")
        .unwrap();

    let old = fts.search("docs", "original", 10).unwrap();
    assert!(old.is_empty());

    let new = fts.search("docs", "replacement", 10).unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].id, "d1");
}

#[test]
fn fts_porter_stemming() {
    let db = AgentDB::open(":memory:").unwrap();
    let fts = db.fts();
    fts.index_text("stem", "s1", "col1", "The runners are running in the race")
        .unwrap();

    let results = fts.search("stem", "run", 10).unwrap();
    assert_eq!(results.len(), 1);
}
