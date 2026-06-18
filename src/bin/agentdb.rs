//! # agentdb CLI
//!
//! Inspect, query, and manage AgentDB database files from the command line.
//!
//! ## Commands
//!
//! ```text
//! agentdb stats       <path>                  — print database statistics
//! agentdb collections <path>                  — list all vector collections
//! agentdb sql         <path> <query>          — run a SQL query, print JSON
//! agentdb search      <path> <col> <vec...>   — ANN vector search
//! agentdb reindex     <path>                  — rebuild all dirty HNSW indexes
//! agentdb inspect     <path>                  — full database summary
//! ```
//!
//! ## Examples
//!
//! ```bash
//! # Install from crates.io
//! cargo install agentdb
//!
//! # Stats
//! agentdb stats agent.agentdb
//!
//! # Run SQL
//! agentdb sql agent.agentdb "SELECT * FROM sessions LIMIT 5"
//!
//! # Full inspection
//! agentdb inspect agent.agentdb
//! ```

use agentdb::AgentDB;
use clap::{Parser, Subcommand};

// ── CLI definition ────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "agentdb",
    version = env!("CARGO_PKG_VERSION"),
    about = "Inspect and manage AgentDB database files",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print database-wide statistics.
    Stats {
        /// Path to the .agentdb file (or :memory: for a blank in-memory DB).
        path: String,
    },

    /// List all vector collections with their dimension and vector count.
    Collections { path: String },

    /// Run a SQL query and print results as pretty-printed JSON.
    Sql {
        path: String,
        /// SQL query to execute.
        query: String,
    },

    /// Approximate nearest-neighbor search in a vector collection.
    Search {
        path: String,
        /// Collection name to search.
        collection: String,
        /// Query vector values (space-separated floats).
        #[arg(num_args = 1.., value_name = "f32")]
        vector: Vec<f32>,
        /// Number of results to return.
        #[arg(short, long, default_value_t = 5)]
        top_k: usize,
    },

    /// Rebuild all dirty HNSW indexes in the database.
    Reindex { path: String },

    /// Print a full summary: stats + collections + recent nodes.
    Inspect { path: String },
}

// ── Entry point ───────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> agentdb::Result<()> {
    match cli.command {
        Commands::Stats { path } => cmd_stats(&path),
        Commands::Collections { path } => cmd_collections(&path),
        Commands::Sql { path, query } => cmd_sql(&path, &query),
        Commands::Search {
            path,
            collection,
            vector,
            top_k,
        } => cmd_search(&path, &collection, &vector, top_k),
        Commands::Reindex { path } => cmd_reindex(&path),
        Commands::Inspect { path } => cmd_inspect(&path),
    }
}

// ── Command implementations ───────────────────────────────────────────

fn cmd_stats(path: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;
    let s = db.stats()?;
    println!("path:        {path}");
    println!("collections: {}", s.collections);
    println!("vectors:     {}", s.vectors);
    println!("nodes:       {}", s.nodes);
    println!("edges:       {}", s.edges);
    Ok(())
}

fn cmd_collections(path: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;
    let cols = db.vectors().list_collections()?;
    if cols.is_empty() {
        println!("No collections found.");
        return Ok(());
    }
    println!("{:<30} {:>6} {:>10}", "name", "dim", "vectors");
    println!("{}", "-".repeat(50));
    for (name, dim, count) in &cols {
        println!("{:<30} {:>6} {:>10}", name, dim, count);
    }
    Ok(())
}

fn cmd_sql(path: &str, query: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;
    let rows = db.query_json(query)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&rows).unwrap_or_default()
    );
    Ok(())
}

fn cmd_search(path: &str, collection: &str, vector: &[f32], top_k: usize) -> agentdb::Result<()> {
    use agentdb::{DistanceMetric, SearchOptions};

    let db = AgentDB::open(path)?;
    let dim = vector.len();
    let col = db.vectors().collection(collection, dim)?;
    let results = col.search(
        vector,
        SearchOptions {
            top_k,
            metric: DistanceMetric::Cosine,
            filter: None,
        },
    )?;

    if results.is_empty() {
        println!("No results.");
        return Ok(());
    }
    println!("{:<30} {:>8}  metadata", "id", "score");
    println!("{}", "-".repeat(60));
    for r in &results {
        let meta = r
            .metadata
            .as_ref()
            .map(|m| m.to_string())
            .unwrap_or_default();
        println!("{:<30} {:>8.4}  {}", r.id, r.score, meta);
    }
    Ok(())
}

fn cmd_reindex(path: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;
    let cols = db.vectors().list_collections()?;
    let mut rebuilt = 0usize;
    for (name, dim, _) in &cols {
        let col = db.vectors().collection(name, *dim)?;
        col.reindex()?;
        rebuilt += 1;
        println!("reindexed: {name}");
    }
    println!("done — {rebuilt} collection(s) reindexed.");
    Ok(())
}

fn cmd_inspect(path: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;

    println!("=== AgentDB Inspect ===");
    println!("path: {path}");
    println!();

    let s = db.stats()?;
    println!("Statistics");
    println!("  collections : {}", s.collections);
    println!("  vectors     : {}", s.vectors);
    println!("  nodes       : {}", s.nodes);
    println!("  edges       : {}", s.edges);
    println!();

    let cols = db.vectors().list_collections()?;
    if !cols.is_empty() {
        println!("Collections");
        println!("  {:<28} {:>6} {:>10}", "name", "dim", "vectors");
        println!("  {}", "-".repeat(48));
        for (name, dim, count) in &cols {
            println!("  {:<28} {:>6} {:>10}", name, dim, count);
        }
        println!();
    }

    let rows =
        db.query_json("SELECT id, kind FROM _adb_nodes ORDER BY created_at DESC LIMIT 10")?;
    if !rows.is_empty() {
        println!("Recent nodes (up to 10)");
        for row in &rows {
            println!(
                "  {} — {}",
                row.get("id").and_then(|v| v.as_str()).unwrap_or("-"),
                row.get("kind").and_then(|v| v.as_str()).unwrap_or("-"),
            );
        }
    }

    Ok(())
}
