use agentdb::{AgentDB, TraversalOptions, VectorEntry};
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:");
    let db = db?;

    println!("=== AgentDB — Agent Memory Demo ===\n");

    db.execute(
        "CREATE TABLE IF NOT EXISTS events (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        data TEXT,
        ts INTEGER
    )",
    )?;

    println!("1. Storing conversation events...");
    db.execute_params(
        "INSERT OR IGNORE INTO events VALUES (?1, ?2, ?3, ?4)",
        &[
            &"evt_1",
            &"user_msg",
            &r#"{"text":"Tell me about Rust"}"#,
            &1700000001_i64,
        ],
    )?;
    db.execute_params(
        "INSERT OR IGNORE INTO events VALUES (?1, ?2, ?3, ?4)",
        &[
            &"evt_2",
            &"agent_reply",
            &r#"{"text":"Rust is a systems language focused on safety"}"#,
            &1700000002_i64,
        ],
    )?;

    let rows = db.query_json("SELECT * FROM events")?;
    println!("   Stored {} events", rows.len());

    println!("\n2. Indexing thought embeddings...");
    let col = db.vectors().collection("thoughts", 4)?;

    let thoughts = vec![
        (
            "thought_rust",
            vec![0.9_f32, 0.1, 0.05, 0.0],
            "Rust is fast and memory safe",
        ),
        (
            "thought_db",
            vec![0.1_f32, 0.9, 0.05, 0.0],
            "Databases store and retrieve data",
        ),
        (
            "thought_ai",
            vec![0.05_f32, 0.1, 0.9, 0.0],
            "AI agents need persistent memory",
        ),
        (
            "thought_embed",
            vec![0.0_f32, 0.05, 0.8, 0.9],
            "Embeddings capture semantic meaning",
        ),
    ];

    for (id, vector, text) in &thoughts {
        col.upsert(VectorEntry {
            id: id.to_string(),
            vector: vector.clone(),
            metadata: Some(json!({"text": text})),
        })?;
    }

    let query = vec![0.85_f32, 0.1, 0.1, 0.0];
    let results = col.search(&query, Default::default())?;
    println!(
        "   Top match for 'Rust performance': {:?}",
        results.first().map(|r| r.id.as_str())
    );

    println!("\n3. Building memory graph...");
    let graph = db.memory();

    graph.add_node(
        "session_42",
        "session",
        Some(json!({"user": "harshal", "date": "2025-01-01"})),
    )?;
    graph.add_node(
        "concept_rust",
        "concept",
        Some(json!({"label": "Rust programming"})),
    )?;
    graph.add_node(
        "concept_perf",
        "concept",
        Some(json!({"label": "Performance optimization"})),
    )?;
    graph.add_node(
        "concept_memory",
        "concept",
        Some(json!({"label": "Memory safety"})),
    )?;
    graph.add_node("concept_db", "concept", Some(json!({"label": "Databases"})))?;

    graph.add_edge("session_42", "concept_rust", "discussed", 0.95)?;
    graph.add_edge("session_42", "concept_db", "discussed", 0.80)?;
    graph.add_edge("concept_rust", "concept_perf", "relates_to", 0.85)?;
    graph.add_edge("concept_rust", "concept_memory", "relates_to", 0.90)?;

    let neighbors = graph.neighbors(
        "session_42",
        TraversalOptions {
            relation: None,
            max_depth: 2,
            min_weight: Some(0.7),
        },
    )?;

    println!("   session_42 memory graph (depth=2):");
    for n in &neighbors {
        println!(
            "     depth={} weight={:.2} → {} ({})",
            n.depth, n.weight, n.node.id, n.node.kind
        );
    }

    println!("\n4. Database stats:");
    let stats = db.stats()?;
    println!("   Collections: {}", stats.collections);
    println!("   Vectors:     {}", stats.vectors);
    println!("   Nodes:       {}", stats.nodes);
    println!("   Edges:       {}", stats.edges);

    println!("\n✓ AgentDB demo complete — all three layers working in one file.");
    Ok(())
}
