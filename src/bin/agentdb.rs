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
//! agentdb shell       <path>                  — interactive SQL/dot-command REPL
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
//!
//! # Interactive shell
//! agentdb shell agent.agentdb
//! agentdb -i agent.agentdb
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
    /// Open the database at PATH in an interactive shell (alias for `agentdb shell <PATH>`).
    #[arg(short = 'i', long = "interactive", value_name = "PATH")]
    interactive: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
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

    /// Migrate a database to the current schema version.
    ///
    /// Re-runs the schema bootstrap to add any missing tables or indexes
    /// introduced in newer versions of AgentDB. Existing data is preserved.
    Migrate { path: String },

    /// Open an interactive SQL / dot-command REPL.
    ///
    /// SQL statements are terminated by a semicolon and may span multiple lines.
    /// Dot-commands are single-line helpers:
    ///
    ///   .help          — show this help text
    ///   .stats         — database statistics
    ///   .collections   — list all vector collections
    ///   .inspect       — full database summary
    ///   .quit / .exit  — leave the shell
    Shell {
        /// Path to the .agentdb file (or :memory: for a blank in-memory DB).
        path: String,
    },
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
    // --interactive / -i flag takes precedence when no subcommand is given.
    if let Some(path) = cli.interactive {
        return cmd_shell(&path);
    }

    match cli.command {
        Some(Commands::Stats { path }) => cmd_stats(&path),
        Some(Commands::Collections { path }) => cmd_collections(&path),
        Some(Commands::Sql { path, query }) => cmd_sql(&path, &query),
        Some(Commands::Search {
            path,
            collection,
            vector,
            top_k,
        }) => cmd_search(&path, &collection, &vector, top_k),
        Some(Commands::Reindex { path }) => cmd_reindex(&path),
        Some(Commands::Inspect { path }) => cmd_inspect(&path),
        Some(Commands::Migrate { path }) => cmd_migrate(&path),
        Some(Commands::Shell { path }) => cmd_shell(&path),
        None => {
            // Neither a subcommand nor -i was supplied — print help.
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "No command specified. Run `agentdb --help` for usage."
            );
            std::process::exit(2);
        }
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

fn cmd_migrate(path: &str) -> agentdb::Result<()> {
    use rusqlite::Connection;

    println!("Migrating: {path}");

    let conn = Connection::open(path).map_err(agentdb::AgentDbError::Sqlite)?;

    // Read current schema version before migration
    let old_version: String = conn
        .query_row(
            "SELECT COALESCE(
                (SELECT value FROM _adb_meta WHERE key = 'schema_version'),
                '0'
            )",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "0".to_string());

    println!("  current schema version: {old_version}");

    // Re-run bootstrap — all CREATE TABLE/INDEX statements use IF NOT EXISTS,
    // so this safely adds any new tables without affecting existing data.
    agentdb::schema::bootstrap(&conn)?;

    // Update the schema version to current
    conn.execute_batch(&format!(
        "UPDATE _adb_meta SET value = '{}' WHERE key = 'schema_version'",
        agentdb::schema::SCHEMA_VERSION
    ))?;

    println!(
        "  migrated to schema version: {}",
        agentdb::schema::SCHEMA_VERSION
    );
    println!("done.");
    Ok(())
}

// ── Interactive shell ─────────────────────────────────────────────────

/// Run an interactive REPL for the database at `path`.
///
/// Input model
/// -----------
/// * SQL statements are accumulated across lines until a semicolon is seen,
///   then executed and the JSON result printed.
/// * Dot-commands (`.help`, `.stats`, `.collections`, `.inspect`,
///   `.quit` / `.exit`) are executed immediately on a single line.
/// * An empty line is ignored.
/// * Ctrl-D (EOF from `read_line` returning `Ok(0)`) exits cleanly.
/// * An `Interrupted` IO error (Ctrl-C on Unix) discards the current buffer
///   and redisplays the prompt so the user can start over.
fn cmd_shell(path: &str) -> agentdb::Result<()> {
    use std::io::{self, BufRead, Write};

    let db = AgentDB::open(path)?;

    println!("AgentDB shell v{}", env!("CARGO_PKG_VERSION"));
    println!("Connected to: {path}");
    println!("Type .help for help, .quit to exit.");
    println!();

    let stdin = io::stdin();
    let stdout = io::stdout();

    // SQL buffer — accumulates lines until a `;` is encountered.
    let mut sql_buf = String::new();

    loop {
        // Choose the prompt: continuation lines use `   ...> ` so the user
        // can see they are still inside a multi-line statement.
        let prompt = if sql_buf.trim().is_empty() {
            "agentdb> "
        } else {
            "      -> "
        };

        // Print the prompt without a newline and flush immediately.
        {
            let mut out = stdout.lock();
            let _ = write!(out, "{prompt}");
            let _ = out.flush();
        }

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            // EOF (Ctrl-D): exit cleanly.
            Ok(0) => {
                println!();
                println!("Bye.");
                break;
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                // Ctrl-C on Unix: discard current buffer and continue.
                if !sql_buf.is_empty() {
                    println!("  (input cleared)");
                    sql_buf.clear();
                } else {
                    println!();
                }
                continue;
            }
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
        }

        let trimmed = line.trim();

        // Skip blank lines.
        if trimmed.is_empty() {
            continue;
        }

        // ── Dot-commands ──────────────────────────────────────────────
        if trimmed.starts_with('.') {
            // Dot-commands are only valid when no SQL buffer is in progress.
            if !sql_buf.trim().is_empty() {
                eprintln!(
                    "error: complete the current statement first (end with `;`), \
                     or press Ctrl-C to discard it."
                );
                continue;
            }

            match trimmed {
                ".quit" | ".exit" => {
                    println!("Bye.");
                    break;
                }
                ".help" => shell_help(),
                ".stats" => {
                    if let Err(e) = cmd_stats(path) {
                        eprintln!("error: {e}");
                    }
                }
                ".collections" => {
                    if let Err(e) = cmd_collections(path) {
                        eprintln!("error: {e}");
                    }
                }
                ".inspect" => {
                    if let Err(e) = cmd_inspect(path) {
                        eprintln!("error: {e}");
                    }
                }
                other => {
                    eprintln!("Unknown dot-command: {other}");
                    eprintln!("Type .help for a list of commands.");
                }
            }
            continue;
        }

        // ── SQL accumulation ──────────────────────────────────────────
        sql_buf.push_str(&line);

        // Execute when the accumulated buffer ends with a semicolon
        // (ignoring trailing whitespace after the semicolon).
        if sql_buf.trim_end().ends_with(';') {
            let query = sql_buf.trim().to_string();
            sql_buf.clear();

            match db.query_json(&query) {
                Ok(rows) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&rows).unwrap_or_default()
                    );
                    println!(
                        "({} row{})",
                        rows.len(),
                        if rows.len() == 1 { "" } else { "s" }
                    );
                }
                Err(e) => eprintln!("error: {e}"),
            }
        }
    }

    Ok(())
}

/// Print the shell help text.
fn shell_help() {
    println!("AgentDB interactive shell");
    println!();
    println!("SQL queries");
    println!("  Enter any SQL statement.  Statements may span multiple lines.");
    println!("  Terminate with a semicolon (;) to execute.");
    println!();
    println!("Dot-commands");
    println!("  .help          show this help text");
    println!("  .stats         print database statistics");
    println!("  .collections   list vector collections");
    println!("  .inspect       full database summary");
    println!("  .quit          exit the shell");
    println!("  .exit          exit the shell (alias for .quit)");
    println!();
    println!("Keyboard shortcuts");
    println!("  Ctrl-C         discard current input line and start fresh");
    println!("  Ctrl-D         exit the shell (EOF)");
}
