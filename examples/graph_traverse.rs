//! Memory Graph Traversal Example
//!
//! Demonstrates building and traversing a typed knowledge graph
//! inside AgentDB. Models an AI agent's episodic memory across
//! multiple sessions, with concepts and entities as nodes.
//!
//! Run with: cargo run --example graph_traverse

use agentdb::{AgentDB, TraversalOptions};
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:")?;
    let graph = db.memory();

    println!("=== AgentDB Memory Graph Traversal Demo ===\n");

    // ── Build the graph ──────────────────────────────────────────────────
    println!("1. Building memory graph...");

    // Sessions
    graph.add_node("session_01", "session", Some(json!({
        "user": "harshal", "date": "2025-01-01", "topic": "Rust basics"
    })))?;
    graph.add_node("session_02", "session", Some(json!({
        "user": "harshal", "date": "2025-01-02", "topic": "Databases"
    })))?;
    graph.add_node("session_03", "session", Some(json!({
        "user": "harshal", "date": "2025-01-03", "topic": "AI agents"
    })))?;

    // Concepts
    graph.add_node("concept_rust",       "concept", Some(json!({ "label": "Rust" })))?;
    graph.add_node("concept_memory",     "concept", Some(json!({ "label": "Memory safety" })))?;
    graph.add_node("concept_perf",       "concept", Some(json!({ "label": "Performance" })))?;
    graph.add_node("concept_db",         "concept", Some(json!({ "label": "Databases" })))?;
    graph.add_node("concept_vectors",    "concept", Some(json!({ "label": "Vector search" })))?;
    graph.add_node("concept_agents",     "concept", Some(json!({ "label": "AI agents" })))?;
    graph.add_node("concept_rag",        "concept", Some(json!({ "label": "RAG" })))?;

    // Entities
    graph.add_node("entity_agentdb",     "entity",  Some(json!({ "label": "AgentDB", "type": "software" })))?;
    graph.add_node("entity_openai",      "entity",  Some(json!({ "label": "OpenAI",  "type": "company" })))?;

    // ── Edges: sessions → concepts ───────────────────────────────────────
    graph.add_edge("session_01", "concept_rust",    "discussed", 0.95)?;
    graph.add_edge("session_01", "concept_memory",  "discussed", 0.85)?;
    graph.add_edge("session_01", "concept_perf",    "discussed", 0.75)?;

    graph.add_edge("session_02", "concept_db",      "discussed", 0.90)?;
    graph.add_edge("session_02", "concept_vectors", "discussed", 0.80)?;
    graph.add_edge("session_02", "entity_agentdb",  "mentioned", 0.95)?;

    graph.add_edge("session_03", "concept_agents",  "discussed", 0.95)?;
    graph.add_edge("session_03", "concept_rag",     "discussed", 0.85)?;
    graph.add_edge("session_03", "entity_openai",   "mentioned", 0.70)?;
    graph.add_edge("session_03", "entity_agentdb",  "mentioned", 0.90)?;

    // ── Edges: concept → concept ─────────────────────────────────────────
    graph.add_edge("concept_rust",    "concept_memory",  "relates_to", 0.90)?;
    graph.add_edge("concept_rust",    "concept_perf",    "relates_to", 0.85)?;
    graph.add_edge("concept_db",      "concept_vectors", "relates_to", 0.80)?;
    graph.add_edge("concept_vectors", "concept_rag",     "enables",    0.90)?;
    graph.add_edge("concept_rag",     "concept_agents",  "used_by",    0.85)?;
    graph.add_edge("concept_agents",  "concept_memory",  "requires",   0.80)?;

    // ── Edges: entity → concept ───────────────────────────────────────────
    graph.add_edge("entity_agentdb", "concept_db",      "implements", 0.95)?;
    graph.add_edge("entity_agentdb", "concept_vectors", "implements", 0.95)?;
    graph.add_edge("entity_agentdb", "concept_rust",    "built_with", 0.95)?;
    graph.add_edge("entity_openai",  "concept_rag",     "popularized", 0.85)?;

    let (nodes, edges) = graph.stats()?;
    println!("   Nodes: {}  Edges: {}", nodes, edges);

    // ── Traversal 1: depth-1 from session_01 ────────────────────────────
    println!("\n2. Depth-1 traversal from session_01 (discussed only):");
    let neighbors = graph.neighbors("session_01", TraversalOptions {
        relation:   Some("discussed".into()),
        max_depth:  1,
        min_weight: None,
    })?;
    for n in &neighbors {
        println!("   → {} ({})  weight={:.2}", n.node.id, n.node.kind, n.weight);
    }

    // ── Traversal 2: depth-2, all relations, min weight 0.8 ─────────────
    println!("\n3. Depth-2 traversal from session_02 (all relations, weight ≥ 0.8):");
    let deep = graph.neighbors("session_02", TraversalOptions {
        relation:   None,
        max_depth:  2,
        min_weight: Some(0.8),
    })?;
    for n in &deep {
        println!(
            "   depth={}  → {} ({})  weight={:.2}",
            n.depth, n.node.id, n.node.kind, n.weight
        );
    }

    // ── Traversal 3: what does AgentDB connect to? ───────────────────────
    println!("\n4. What does entity_agentdb connect to (depth=2)?");
    let agentdb_neighbors = graph.neighbors("entity_agentdb", TraversalOptions {
        relation:   None,
        max_depth:  2,
        min_weight: Some(0.7),
    })?;
    for n in &agentdb_neighbors {
        println!(
            "   depth={}  → {} ({})  weight={:.2}",
            n.depth, n.node.id, n.node.kind, n.weight
        );
    }

    // ── Node kind query ───────────────────────────────────────────────────
    println!("\n5. All concept nodes:");
    let concepts = graph.nodes_by_kind("concept")?;
    for c in &concepts {
        let label = c.data.as_ref()
            .and_then(|d| d["label"].as_str())
            .unwrap_or("?");
        println!("   {} — {}", c.id, label);
    }

    println!("\n✓ Graph traversal demo complete.");
    Ok(())
}
