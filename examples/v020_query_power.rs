use agentdb::{AgentDB, BatchEntry, DistanceMetric, HybridQuery, SearchOptions, VectorEntry};
use serde_json::json;

fn embed(seed: f32, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|i| ((seed + i as f32) * 0.1).sin().abs())
        .collect()
}

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:");
    let db = db?;
    println!("=== AgentDB v0.2.0 \u2014 Query Power Demo ===\n");

    // 1. BATCH UPSERT
    println!("1. Batch upsert (single transaction)");
    let col = db.vectors().collection("docs", 8)?;

    let batch: Vec<BatchEntry> = vec![
        BatchEntry {
            id: "doc_rust".into(),
            vector: embed(1.0, 8),
            metadata: Some(
                json!({"text": "Rust systems programming language", "lang": "en", "score": 9, "ts": 1700000100}),
            ),
        },
        BatchEntry {
            id: "doc_agents".into(),
            vector: embed(2.0, 8),
            metadata: Some(
                json!({"text": "AI agents and autonomous systems", "lang": "en", "score": 8, "ts": 1700000200}),
            ),
        },
        BatchEntry {
            id: "doc_db".into(),
            vector: embed(3.0, 8),
            metadata: Some(
                json!({"text": "Database design and vector search", "lang": "en", "score": 7, "ts": 1700000300}),
            ),
        },
        BatchEntry {
            id: "doc_rag".into(),
            vector: embed(4.0, 8),
            metadata: Some(
                json!({"text": "Retrieval augmented generation RAG", "lang": "en", "score": 8, "ts": 1700000400}),
            ),
        },
        BatchEntry {
            id: "doc_memory".into(),
            vector: embed(5.0, 8),
            metadata: Some(
                json!({"text": "Memory graphs for episodic recall", "lang": "en", "score": 9, "ts": 1700000500}),
            ),
        },
        BatchEntry {
            id: "doc_fr".into(),
            vector: embed(6.0, 8),
            metadata: Some(
                json!({"text": "Apprentissage automatique avanc\u00e9", "lang": "fr", "score": 6, "ts": 1700000600}),
            ),
        },
    ];

    let inserted = col.upsert_batch(batch)?;
    println!("   Inserted {} documents in one transaction", inserted);
    println!("   Collection size: {}", col.count()?);

    // 2. ADVANCED METADATA FILTERING
    println!("\n2. Advanced metadata filtering");

    let high_score = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "score": { "$gt": 7 } })),
        },
    )?;
    println!("   score > 7: {} results", high_score.len());
    for r in &high_score {
        println!(
            "     {} score={}",
            r.id,
            r.metadata.as_ref().unwrap()["score"]
        );
    }

    let english = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "lang": { "$in": ["en"] } })),
        },
    )?;
    println!("   lang $in [en]: {} results", english.len());

    let has_score = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "score": { "$exists": true } })),
        },
    )?;
    println!("   score $exists true: {} results", has_score.len());

    let top_en = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "lang": "en", "score": { "$gte": 8 } })),
        },
    )?;
    println!("   lang=en AND score >= 8: {} results", top_en.len());

    // 3. FULL-TEXT SEARCH
    println!("\n3. Full-text search (FTS5 + BM25)");
    let fts = db.fts();

    let docs_text = vec![
        (
            "doc_rust",
            "Rust systems programming language memory safety performance",
        ),
        (
            "doc_agents",
            "AI agents autonomous systems planning reasoning memory",
        ),
        (
            "doc_db",
            "database design vector search embeddings storage retrieval",
        ),
        (
            "doc_rag",
            "retrieval augmented generation language model context",
        ),
        (
            "doc_memory",
            "memory graphs episodic recall knowledge representation",
        ),
        (
            "doc_fr",
            "apprentissage automatique intelligence artificielle",
        ),
    ];

    let col_id = col.id.clone();
    for (id, text) in &docs_text {
        fts.index_text("docs", id, &col_id, text)?;
    }
    fts.optimize("docs")?;

    let kw_results = fts.search("docs", "memory", 5)?;
    println!("   FTS 'memory': {} results", kw_results.len());
    for r in &kw_results {
        println!("     {} | snippet: {}", r.id, r.snippet);
    }

    let kw2 = fts.search("docs", "vector search", 5)?;
    println!("   FTS 'vector search': {} results", kw2.len());
    for r in &kw2 {
        println!("     {} | rank: {:.4}", r.id, r.rank);
    }

    // 4. HYBRID QUERY
    println!("\n4. Hybrid query (graph traversal + vector search)");
    let graph = db.memory();

    graph.add_node("session_x", "session", Some(json!({"user": "harshal"})))?;
    graph.add_node("doc_rust", "doc", Some(json!({"title": "Rust"})))?;
    graph.add_node("doc_agents", "doc", Some(json!({"title": "Agents"})))?;
    graph.add_node("doc_memory", "doc", Some(json!({"title": "Memory"})))?;
    graph.add_edge("session_x", "doc_rust", "read", 0.95)?;
    graph.add_edge("session_x", "doc_agents", "read", 0.80)?;
    graph.add_edge("session_x", "doc_memory", "read", 0.70)?;

    let hybrid_results = db.hybrid_query(HybridQuery {
        anchor_node: "session_x",
        embedding: &embed(1.0, 8),
        collection: "docs",
        graph_depth: 1,
        top_k: 3,
        alpha: 0.6,
        filter: None,
    })?;

    println!(
        "   Top {} hybrid results (alpha=0.6):",
        hybrid_results.len()
    );
    for r in &hybrid_results {
        println!(
            "     {} | rank={:.4}  vec={:.4}  graph={:.2}",
            r.id, r.rank_score, r.vector_score, r.graph_weight
        );
    }

    let graph_only = db.hybrid_query(HybridQuery {
        anchor_node: "session_x",
        embedding: &embed(1.0, 8),
        collection: "docs",
        graph_depth: 1,
        top_k: 3,
        alpha: 0.0,
        filter: None,
    })?;
    println!(
        "   Top {} results (alpha=0.0, pure graph):",
        graph_only.len()
    );
    for r in &graph_only {
        println!("     {} | graph_weight={:.2}", r.id, r.graph_weight);
    }

    let _ = VectorEntry {
        id: "test".into(),
        vector: vec![0.0; 8],
        metadata: None,
    };

    println!("\n\u2713 v0.2.0 demo complete.");
    Ok(())
}
