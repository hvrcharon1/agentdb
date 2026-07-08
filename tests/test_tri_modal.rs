use agentdb::{AgentDB, TriModalQuery, VectorEntry};
use serde_json::json;

/// Helper: open an in-memory database.
fn open() -> AgentDB {
    AgentDB::open(":memory:").expect("failed to open in-memory db")
}

/// Helper: build a 4-dimensional unit-ish vector biased toward dimension `idx`.
fn bias_vec(idx: usize, dim: usize) -> Vec<f32> {
    let mut v = vec![0.1f32; dim];
    v[idx % dim] = 0.9;
    v
}

// ── Utility: seed a collection with text + vectors + graph edges ───────────

fn seed_db(db: &AgentDB) {
    // Create collection with vectors and FTS text.
    let col = db.vectors().collection("docs", 4).unwrap();
    let entries = vec![
        ("d1", bias_vec(0, 4), "rust programming language systems"),
        ("d2", bias_vec(1, 4), "python machine learning data science"),
        ("d3", bias_vec(2, 4), "rust async tokio runtime"),
        ("d4", bias_vec(3, 4), "sql database query optimization"),
    ];
    for (id, vec, text) in &entries {
        col.upsert_with_text(
            VectorEntry {
                id: id.to_string(),
                vector: vec.clone(),
                metadata: Some(json!({ "tag": id })),
            },
            text,
        )
        .unwrap();
    }

    // Build a small memory graph: anchor → d1 → d3
    let graph = db.memory();
    graph.add_node("anchor", "session", None).unwrap();
    graph.add_node("d1", "doc", None).unwrap();
    graph.add_node("d2", "doc", None).unwrap();
    graph.add_node("d3", "doc", None).unwrap();
    graph.add_node("d4", "doc", None).unwrap();
    graph.add_edge("anchor", "d1", "related", 0.9).unwrap();
    graph.add_edge("anchor", "d3", "related", 0.7).unwrap();
    graph.add_edge("d1", "d2", "linked", 0.5).unwrap();
}

// ── Test 1: Basic tri-modal query ──────────────────────────────────────────

#[test]
fn tri_modal_basic_all_channels() {
    let db = open();
    seed_db(&db);

    let query = TriModalQuery {
        anchor_node: "anchor".to_string(),
        embedding: bias_vec(0, 4),          // similar to d1
        text_query: "rust".to_string(),      // matches d1 and d3
        collection: "docs".to_string(),
        graph_depth: 2,
        top_k: 4,
        alpha: 0.4,
        beta: 0.3,
        gamma: 0.3,
        filter: None,
    };

    let results = db.tri_modal_query(&query).unwrap();
    assert!(!results.is_empty(), "should return results");
    assert!(results.len() <= 4, "should respect top_k");

    // Results must be sorted descending by rank_score
    for w in results.windows(2) {
        assert!(
            w[0].rank_score >= w[1].rank_score,
            "results must be sorted by rank_score desc"
        );
    }

    // d1 is strongly connected via graph AND matched by FTS AND vector-similar → should rank highly
    let d1 = results.iter().find(|r| r.id == "d1");
    assert!(d1.is_some(), "d1 should appear in results");
}

// ── Test 2: Pure vector mode (alpha=1, beta=0, gamma=0) ────────────────────

#[test]
fn tri_modal_pure_vector_mode() {
    let db = open();
    seed_db(&db);

    let query_vector = bias_vec(1, 4); // biased toward d2

    let tri_results = db
        .tri_modal_query(&TriModalQuery {
            anchor_node: "anchor".to_string(),
            embedding: query_vector.clone(),
            text_query: String::new(),
            collection: "docs".to_string(),
            graph_depth: 2,
            top_k: 4,
            alpha: 1.0,
            beta: 0.0,
            gamma: 0.0,
            filter: None,
        })
        .unwrap();

    assert!(!tri_results.is_empty(), "pure vector mode should return results");

    // All graph_weight components should be None (beta=0, graph not traversed)
    // and all fts_rank components should be None (gamma=0, FTS not queried)
    for r in &tri_results {
        assert!(r.graph_weight.is_none(), "graph_weight should be None in pure vector mode");
        assert!(r.fts_rank.is_none(), "fts_rank should be None in pure vector mode");
        assert!(r.vector_score.is_some(), "vector_score should be present");
    }

    // Top result should be d2 (most similar to bias_vec(1,4))
    let top = &tri_results[0];
    assert_eq!(top.id, "d2", "d2 should be the top result in pure vector mode");
}

// ── Test 3: Pure FTS mode (alpha=0, beta=0, gamma=1) ──────────────────────

#[test]
fn tri_modal_pure_fts_mode() {
    let db = open();
    seed_db(&db);

    let results = db
        .tri_modal_query(&TriModalQuery {
            anchor_node: "anchor".to_string(),
            embedding: vec![0.0; 4],
            text_query: "python machine learning".to_string(),
            collection: "docs".to_string(),
            graph_depth: 0,
            top_k: 4,
            alpha: 0.0,
            beta: 0.0,
            gamma: 1.0,
            filter: None,
        })
        .unwrap();

    assert!(!results.is_empty(), "pure FTS mode should return results");

    // All vector_score and graph_weight components should be None
    for r in &results {
        assert!(r.vector_score.is_none(), "vector_score should be None in pure FTS mode");
        assert!(r.graph_weight.is_none(), "graph_weight should be None in pure FTS mode");
        assert!(r.fts_rank.is_some(), "fts_rank should be present");
    }

    // d2 is the only document matching "python machine learning"
    let top = &results[0];
    assert_eq!(top.id, "d2", "d2 should be the top FTS result for 'python machine learning'");
}

// ── Test 4: Weight validation ──────────────────────────────────────────────

#[test]
fn tri_modal_weight_validation_rejects_bad_weights() {
    let db = open();
    seed_db(&db);

    // Weights that don't sum to 1.0
    let result = db.tri_modal_query(&TriModalQuery {
        anchor_node: "anchor".to_string(),
        embedding: bias_vec(0, 4),
        text_query: "rust".to_string(),
        collection: "docs".to_string(),
        graph_depth: 2,
        top_k: 4,
        alpha: 0.5,
        beta: 0.5,
        gamma: 0.5, // sum = 1.5
        filter: None,
    });

    assert!(result.is_err(), "should reject weights that don't sum to ~1.0");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("alpha") || err_msg.contains("gamma") || err_msg.contains("1.0"),
        "error message should mention weight constraint"
    );
}

#[test]
fn tri_modal_weight_validation_accepts_valid_weights() {
    let db = open();
    seed_db(&db);

    // Exactly equal thirds
    let result = db.tri_modal_query(&TriModalQuery {
        anchor_node: "anchor".to_string(),
        embedding: bias_vec(0, 4),
        text_query: "rust".to_string(),
        collection: "docs".to_string(),
        graph_depth: 2,
        top_k: 4,
        alpha: 1.0 / 3.0,
        beta: 1.0 / 3.0,
        gamma: 1.0 / 3.0,
        filter: None,
    });

    // 1/3 + 1/3 + 1/3 is slightly less than 1.0 in floating point;
    // the implementation has a 0.01 tolerance so this should pass.
    assert!(result.is_ok(), "equal thirds should be accepted (within tolerance)");
}

// ── Test 5: Empty result handling ──────────────────────────────────────────

#[test]
fn tri_modal_empty_collection_returns_empty() {
    let db = open();
    // No data seeded — empty collection
    let results = db
        .tri_modal_query(&TriModalQuery {
            anchor_node: "nonexistent".to_string(),
            embedding: vec![0.1, 0.2, 0.3, 0.4],
            text_query: "anything".to_string(),
            collection: "empty_col".to_string(),
            graph_depth: 2,
            top_k: 10,
            alpha: 0.4,
            beta: 0.3,
            gamma: 0.3,
            filter: None,
        })
        .unwrap();

    assert!(results.is_empty(), "empty collection should return empty results");
}

#[test]
fn tri_modal_no_fts_text_skips_fts() {
    let db = open();
    seed_db(&db);

    // gamma > 0 but text_query is empty — FTS channel should gracefully no-op
    let results = db
        .tri_modal_query(&TriModalQuery {
            anchor_node: "anchor".to_string(),
            embedding: bias_vec(0, 4),
            text_query: String::new(), // empty — no FTS
            collection: "docs".to_string(),
            graph_depth: 2,
            top_k: 4,
            alpha: 0.5,
            beta: 0.3,
            gamma: 0.2,
            filter: None,
        })
        .unwrap();

    // Should still return results from vector + graph channels
    assert!(!results.is_empty(), "should return results even without FTS query");
    for r in &results {
        assert!(r.fts_rank.is_none(), "fts_rank should be None when text_query is empty");
    }
}

// ── Test 6: top_k is respected ─────────────────────────────────────────────

#[test]
fn tri_modal_respects_top_k() {
    let db = open();
    seed_db(&db);

    let results = db
        .tri_modal_query(&TriModalQuery {
            anchor_node: "anchor".to_string(),
            embedding: bias_vec(0, 4),
            text_query: "rust".to_string(),
            collection: "docs".to_string(),
            graph_depth: 3,
            top_k: 2,
            alpha: 0.34,
            beta: 0.33,
            gamma: 0.33,
            filter: None,
        })
        .unwrap();

    assert!(results.len() <= 2, "should return at most top_k results");
}
