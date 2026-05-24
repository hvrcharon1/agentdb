use agentdb::{AgentDB, DistanceMetric, SearchOptions, VectorEntry};
use serde_json::json;

fn fake_embed(text: &str) -> Vec<f32> {
    let seed = text.len() as f32 / 100.0;
    let first = text.chars().next().unwrap_or('a') as u8 as f32 / 255.0;
    vec![
        seed,
        first,
        1.0 - seed,
        (seed + first) / 2.0,
        seed * first,
        1.0 - first,
        seed / 2.0,
        first / 2.0,
    ]
}

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:");
    let db = db?;

    println!("=== AgentDB RAG Pipeline Demo ===\n");

    println!("1. Ingesting document chunks into vector store...");

    let col = db
        .vectors()
        .collection_with_metric("docs", 8, DistanceMetric::Cosine)?;

    let documents = vec![
        (
            "chunk_001",
            "Rust is a systems programming language focused on safety and performance.",
        ),
        (
            "chunk_002",
            "The borrow checker prevents data races and memory errors at compile time.",
        ),
        (
            "chunk_003",
            "Cargo is Rust's package manager and build system.",
        ),
        (
            "chunk_004",
            "AgentDB stores vectors, graphs, and relational data in one file.",
        ),
        (
            "chunk_005",
            "HNSW is an algorithm for approximate nearest neighbor search.",
        ),
        (
            "chunk_006",
            "Embeddings are dense vector representations of text or other data.",
        ),
        (
            "chunk_007",
            "RAG combines retrieval with generation for more accurate LLM responses.",
        ),
        (
            "chunk_008",
            "Memory graphs help AI agents recall and relate past concepts.",
        ),
    ];

    for (id, text) in &documents {
        col.upsert(VectorEntry {
            id: id.to_string(),
            vector: fake_embed(text),
            metadata: Some(json!({
                "text": text,
                "source": "docs_v1",
                "char_count": text.len()
            })),
        })?;
    }

    println!("   Ingested {} document chunks", documents.len());
    println!("   Collection size: {} vectors", col.count()?);

    let query = "How does AgentDB handle vector search?";
    println!("\n2. User query: \"{}\"", query);

    println!("\n3. Retrieving top-3 relevant chunks...");

    let query_vec = fake_embed(query);
    let results = col.search(
        &query_vec,
        SearchOptions {
            top_k: 3,
            metric: DistanceMetric::Cosine,
            filter: None,
        },
    )?;

    println!("\n4. Retrieved context (pass to LLM):");
    println!("{}", "\u2500".repeat(60));

    for (i, result) in results.iter().enumerate() {
        let text = result
            .metadata
            .as_ref()
            .and_then(|m| m["text"].as_str())
            .unwrap_or("(no text)");
        println!(
            "  [{}] score={:.4}  id={}\n       {}",
            i + 1,
            result.score,
            result.id,
            text
        );
    }

    println!("{}", "\u2500".repeat(60));
    println!("\n\u2713 RAG retrieval complete \u2014 feed context above into your LLM.");

    println!("\n5. Filtered search (source = docs_v1):");
    let filtered = col.search(
        &query_vec,
        SearchOptions {
            top_k: 2,
            metric: DistanceMetric::Cosine,
            filter: Some(json!({ "source": "docs_v1" })),
        },
    )?;
    println!("   Returned {} results with source filter", filtered.len());

    Ok(())
}
