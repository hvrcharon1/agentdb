//! v0.2.0 Feature Demo
//!
//! Demonstrates all four v0.2.0 features:
//!   1. Advanced metadata filtering ($gt, $in, $exists, $ne, ...)
//!   2. Batch upsert
//!   3. Hybrid query (graph + vector blended ranking)
//!   4. Full-text search (FTS5)
//!
//! Run with: cargo run --example v020_query_power

use agentdb::{
    AgentDB, BatchEntry, DistanceMetric, FtsResult, HybridQuery,
    SearchOptions, VectorEntry,
};
use serde_json::json;

/// Simple deterministic fake embedding
fn embed(seed: f32, dim: usize) -> Vec<f32> {
    (0..dim).map(|i| ((seed + i as f32) * 0.1).sin().abs()).collect()
}

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:")?;
    println!("=== AgentDB v0.2.0 — Query Power Demo ===\n");

    // ──────────────────────────────────────────────────────────────
    // 1. BATCH UPSERT
    // ──────────────────────────────────────────────────────────────
    println!("1. Batch upsert (single transaction)");
    let col = db.vectors().collection("docs", 8)?;

    let batch: Vec<BatchEntry> = vec![
        BatchEntry { id: "doc_rust".into(),    vector: embed(1.0, 8), metadata: Some(json!({ "text": "Rust systems programming language",   "lang": "en", "score": 9, "ts": 1700000100 })) },
        BatchEntry { id: "doc_agents".into(),  vector: embed(2.0, 8), metadata: Some(json!({ "text": "AI agents and autonomous systems",     "lang": "en", "score": 8, "ts": 1700000200 })) },
        BatchEntry { id: "doc_db".into(),      vector: embed(3.0, 8), metadata: Some(json!({ "text": "Database design and vector search",    "lang": "en", "score": 7, "ts": 1700000300 })) },
        BatchEntry { id: "doc_rag".into(),     vector: embed(4.0, 8), metadata: Some(json!({ "text": "Retrieval augmented generation RAG",   "lang": "en", "score": 8, "ts": 1700000400 })) },
        BatchEntry { id: "doc_memory".into(),  vector: embed(5.0, 8), metadata: Some(json!({ "text": "Memory graphs for episodic recall",    "lang": "en", "score": 9, "ts": 1700000500 })) },
        BatchEntry { id: "doc_fr".into(),      vector: embed(6.0, 8), metadata: Some(json!({ "text": "Apprentissage automatique avancé",     "lang": "fr", "score": 6, "ts": 1700000600 })) },
    ];

    let inserted = col.upsert_batch(batch)?;
    println!("   Inserted {} documents in one transaction", inserted);
    println!("   Collection size: {}", col.count()?);

    // ──────────────────────────────────────────────────────────────
    // 2. ADVANCED METADATA FILTERING
    // ──────────────────────────────────────────────────────────────
    println!("\n2. Advanced metadata filtering");

    // $gt — score greater than 7
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
        println!("     {} score={}", r.id, r.metadata.as_ref().unwrap()["score"]);
    }

    // $in — lang in ["en"]
    let english = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "lang": { "$in": ["en"] } })),
        },
    )?;
    println!("   lang $in [en]: {} results", english.len());

    // $exists — has 'score' field
    let has_score = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "score": { "$exists": true } })),
        },
    )?;
    println!("   score $exists true: {} results", has_score.len());

    // Combined: lang=en AND score >= 8
    let top_en = col.search(
        &embed(1.0, 8),
        SearchOptions {
            top_k: 10,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "lang": "en", "score": { "$gte": 8 } })),
        },
    )?;
    println!("   lang=en AND score >= 8: {} results", top_en.len());

    // ──────────────────────────────────────────────────────────────
    // 3. FULL-TEXT SEARCH (FTS5)
    // ──────────────────────────────────────────────────────────────
    println!("\n3. Full-text search (FTS5 + BM25)");
    let fts = db.fts();

    // Index text content for each doc
    let docs_text = vec![
        ("doc_rust",   "Rust systems programming language memory safety performance"),
        ("doc_agents", "AI agents autonomous systems planning reasoning memory"),
        ("doc_db",     "database design vector search embeddings storage retrieval"),
        ("doc_rag",    "retrieval augmented generation language model context"),
        ("doc_memory", "memory graphs episodic recall knowledge representation"),
        ("doc_fr",     "apprentissage automatique intelligence artificielle"),
    ];

    // Get the collection id for FTS indexing
    let col_id = &col.id;
    for (id, text) in &docs_text {
        fts.index_text("docs", id, col_id, text)?;
    }
    fts.optimize("docs")?;

    // Keyword search
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

    // ──────────────────────────────────────────────────────────────
    // 4. HYBRID QUERY (graph + vector)
    // ──────────────────────────────────────────────────────────────
    println!("\n4. Hybrid query (graph traversal + vector search)");
    let graph = db.memory();

    // Build a small knowledge graph linking sessions to doc IDs
    graph.add_node("session_x",   "session", Some(json!({ "user": "harshal" })))?;
    graph.add_node("doc_rust",    "doc",     Some(json!({ "title": "Rust" })))?;
    graph.add_node("doc_agents",  "doc",     Some(json!({ "title": "Agents" })))?;
    graph.add_node("doc_memory",  "doc",     Some(json!({ "title": "Memory" })))?;
    graph.add_edge("session_x", "doc_rust",   "read", 0.95)?;
    graph.add_edge("session_x", "doc_agents", "read", 0.80)?;
    graph.add_edge("session_x", "doc_memory", "read", 0.70)?;

    // Hybrid: anchor on session_x, query vector ~= doc_rust
    let hybrid_results = db.hybrid_query(HybridQuery {
        anchor_node: "session_x",
        embedding:   &embed(1.0, 8),
        collection:  "docs",
        graph_depth: 1,
        top_k:       3,
        alpha:       0.6,   // 60% vector, 40% graph weight
        filter:      None,
    })?;

    println!("   Top {} hybrid results (alpha=0.6):", hybrid_results.len());
    for r in &hybrid_results {
        println!(
            "     {} | rank={:.4}  vec={:.4}  graph={:.2}",
            r.id, r.rank_score, r.vector_score, r.graph_weight
        );
    }

    // Pure graph (alpha=0.0)
    let graph_only = db.hybrid_query(HybridQuery {
        anchor_node: "session_x",
        embedding:   &embed(1.0, 8),
        collection:  "docs",
        graph_depth: 1,
        top_k:       3,
        alpha:       0.0,
        filter:      None,
    })?;
    println!("   Top {} results (alpha=0.0, pure graph):", graph_only.len());
    for r in &graph_only {
        println!("     {} | graph_weight={:.2}", r.id, r.graph_weight);
    }

    println!("\n✓ v0.2.0 demo complete.");
    Ok(())
}
