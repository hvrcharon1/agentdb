//! # agentdb CLI
//!
//! Inspect, query, and manage AgentDB database files from the command line.
//!
//! ## Commands
//!
//! ```text
//! agentdb version                             — print version and build target
//! agentdb stats       <path>                  — print database statistics
//! agentdb collections <path>                  — list all vector collections
//! agentdb sql         <path> <query>          — run a SQL query, print JSON
//! agentdb search      <path> <col> <vec...>   — ANN vector search
//! agentdb reindex     <path>                  — rebuild all dirty HNSW indexes
//! agentdb inspect     <path>                  — full database summary
//! agentdb migrate     <path>                  — migrate schema to current version
//! agentdb export      <path>                  — dump database as SQL statements
//! agentdb completions --shell <shell>         — generate shell completion script
//! agentdb shell       <path>                  — interactive SQL/dot-command REPL
//! agentdb mcp         <path>                  — start MCP server over stdio
//! ```
//!
//! ## Examples
//!
//! ```bash
//! # Install from crates.io
//! cargo install agentdb
//!
//! # Version
//! agentdb version
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
//! # Export as SQL dump
//! agentdb export agent.agentdb
//! agentdb export agent.agentdb --table _adb_nodes
//!
//! # Generate shell completions
//! agentdb completions --shell bash
//!
//! # Interactive shell
//! agentdb shell agent.agentdb
//! agentdb -i agent.agentdb
//! ```

use agentdb::AgentDB;
use clap::{CommandFactory, Parser, Subcommand};

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
    /// Print the agentdb version and build target triple.
    Version,

    /// Print database-wide statistics.
    Stats {
        /// Path to the .agentdb file (or :memory: for a blank in-memory DB).
        path: String,
    },

    /// List all vector collections with their dimension and vector count.
    Collections {
        /// Path to the .agentdb file.
        path: String,
    },

    /// Run a SQL query and print results as pretty-printed JSON.
    Sql {
        /// Path to the .agentdb file.
        path: String,
        /// SQL query to execute.
        query: String,
    },

    /// Approximate nearest-neighbor search in a vector collection.
    Search {
        /// Path to the .agentdb file.
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
    Reindex {
        /// Path to the .agentdb file.
        path: String,
    },

    /// Print a full summary: stats + collections + recent nodes.
    Inspect {
        /// Path to the .agentdb file.
        path: String,
    },

    /// Migrate a database to the current schema version.
    ///
    /// Re-runs the schema bootstrap to add any missing tables or indexes
    /// introduced in newer versions of AgentDB. Existing data is preserved.
    Migrate {
        /// Path to the .agentdb file.
        path: String,
    },

    /// Export a database's contents as SQL statements (like SQLite `.dump`).
    ///
    /// Outputs CREATE TABLE statements followed by INSERT statements for every
    /// row in each user table.  Pipe the output into `sqlite3` to restore a
    /// database, or use it for backups and audits.
    Export {
        /// Path to the .agentdb file.
        path: String,
        /// Export only this table (default: all user tables).
        #[arg(short, long, value_name = "TABLE")]
        table: Option<String>,
    },

    /// Generate shell completion scripts for agentdb.
    ///
    /// Pipe the output to the appropriate location for your shell:
    ///
    ///   agentdb completions --shell bash   >> ~/.bash_completion
    ///   agentdb completions --shell zsh    > ~/.zfunc/_agentdb
    ///   agentdb completions --shell fish   > ~/.config/fish/completions/agentdb.fish
    Completions {
        /// Shell to generate completions for.
        #[arg(long, value_name = "SHELL")]
        shell: clap_complete::Shell,
    },

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

    /// Start an MCP (Model Context Protocol) server over stdio.
    ///
    /// Reads newline-delimited JSON-RPC messages from stdin, processes them
    /// against the given database, and writes responses to stdout. This allows
    /// AgentDB to be launched as a subprocess by any MCP host (Claude Desktop,
    /// VS Code, etc.).
    Mcp {
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
        Some(Commands::Version) => cmd_version(),
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
        Some(Commands::Export { path, table }) => cmd_export(&path, table.as_deref()),
        Some(Commands::Completions { shell }) => cmd_completions(shell),
        Some(Commands::Shell { path }) => cmd_shell(&path),
        Some(Commands::Mcp { path }) => cmd_mcp(&path),
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

fn cmd_version() -> agentdb::Result<()> {
    // CARGO_CFG_TARGET_ARCH / _OS / _ENV are set by Cargo during the *build
    // script* phase, not for normal compilation units.  The cleanest way to
    // expose the full target triple at runtime without a build script is to
    // compose it from std::env::consts, which is always present.
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let family = std::env::consts::FAMILY;
    println!("agentdb {}", env!("CARGO_PKG_VERSION"));
    println!("target:  {arch}-{family}-{os}");
    Ok(())
}

fn cmd_stats(path: &str) -> agentdb::Result<()> {
    let db = AgentDB::open(path)?;
    let s = db.stats()?;
    println!("path:           {path}");
    println!("collections:    {}", s.collections);
    println!("vectors:        {}", s.vectors);
    println!("nodes:          {}", s.nodes);
    println!("edges:          {}", s.edges);
    println!("conversations:  {}", s.conversations);
    println!("messages:       {}", s.messages);
    println!("workflows:      {}", s.workflows);
    println!("workflow_steps: {}", s.workflow_steps);
    println!("traces:         {}", s.traces);
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
    println!("  collections    : {}", s.collections);
    println!("  vectors        : {}", s.vectors);
    println!("  nodes          : {}", s.nodes);
    println!("  edges          : {}", s.edges);
    println!("  conversations  : {}", s.conversations);
    println!("  messages       : {}", s.messages);
    println!("  workflows      : {}", s.workflows);
    println!("  workflow_steps : {}", s.workflow_steps);
    println!("  traces         : {}", s.traces);
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

    agentdb::schema::migrate(&conn)?;

    println!(
        "  migrated to schema version: {}",
        agentdb::schema::SCHEMA_VERSION
    );
    println!("done.");
    Ok(())
}

/// Export all (or one) user tables from the database as SQL statements.
///
/// Writes `CREATE TABLE` DDL followed by `INSERT INTO` statements for every
/// row in each matching table.  Internal SQLite tables (`sqlite_*`) are always
/// excluded.
fn cmd_export(path: &str, only_table: Option<&str>) -> agentdb::Result<()> {
    use rusqlite::{Connection, params};

    let conn = Connection::open(path).map_err(agentdb::AgentDbError::Sqlite)?;

    // ── Collect table names + DDL ─────────────────────────────────────
    let mut stmt = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master \
             WHERE type = 'table' \
               AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .map_err(agentdb::AgentDbError::Sqlite)?;

    // Collect into a Vec so we can close `stmt` before issuing per-table queries.
    let tables: Vec<(String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(agentdb::AgentDbError::Sqlite)?
        .filter_map(|r| r.ok())
        .filter(|(name, _)| {
            // If --table was specified, only include that table.
            only_table.map_or(true, |t| name == t)
        })
        .collect();

    if tables.is_empty() {
        if let Some(t) = only_table {
            eprintln!("warning: table '{t}' not found in {path}");
        } else {
            eprintln!("warning: no user tables found in {path}");
        }
        return Ok(());
    }

    println!("-- AgentDB SQL dump");
    println!("-- Source: {path}");
    println!("-- Generated by agentdb {}", env!("CARGO_PKG_VERSION"));
    println!("PRAGMA foreign_keys = OFF;");
    println!("BEGIN TRANSACTION;");
    println!();

    for (table_name, ddl) in &tables {
        // ── DDL ───────────────────────────────────────────────────────
        println!("-- Table: {table_name}");
        println!("{ddl};");
        println!();

        // ── Rows ──────────────────────────────────────────────────────
        let select_sql = format!("SELECT * FROM \"{table_name}\"");
        let mut row_stmt = match conn.prepare(&select_sql) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("warning: could not query '{table_name}': {e}");
                continue;
            }
        };

        let col_count = row_stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| row_stmt.column_name(i).unwrap_or("?").to_string())
            .collect();

        let col_list = col_names
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");

        let mut rows = row_stmt
            .query(params![])
            .map_err(agentdb::AgentDbError::Sqlite)?;

        let mut row_count = 0u64;
        loop {
            let row = match rows.next().map_err(agentdb::AgentDbError::Sqlite)? {
                Some(r) => r,
                None => break,
            };

            let values: Vec<String> = (0..col_count)
                .map(|i| {
                    use rusqlite::types::ValueRef;
                    match row.get_ref_unwrap(i) {
                        ValueRef::Null => "NULL".to_string(),
                        ValueRef::Integer(n) => n.to_string(),
                        ValueRef::Real(f) => {
                            // Use repr-style to round-trip f64 exactly.
                            format!("{f:?}")
                        }
                        ValueRef::Text(s) => {
                            // Escape single-quotes by doubling them (SQL standard).
                            let escaped = String::from_utf8_lossy(s).replace('\'', "''");
                            format!("'{escaped}'")
                        }
                        ValueRef::Blob(b) => {
                            // Encode blobs as SQLite X'...' hex literals.
                            let hex: String = b.iter().map(|byte| format!("{byte:02X}")).collect();
                            format!("X'{hex}'")
                        }
                    }
                })
                .collect();

            println!(
                "INSERT INTO \"{table_name}\" ({col_list}) VALUES ({});",
                values.join(", ")
            );
            row_count += 1;
        }

        if row_count == 0 {
            println!("-- (no rows)");
        }
        println!();
    }

    println!("COMMIT;");
    Ok(())
}

/// Generate shell completion scripts and write them to stdout.
fn cmd_completions(shell: clap_complete::Shell) -> agentdb::Result<()> {
    use clap_complete::generate;
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}

// ── MCP server ────────────────────────────────────────────────────────

fn cmd_mcp(path: &str) -> agentdb::Result<()> {
    use agentdb::mcp::McpServer;
    use std::io::{self, BufRead, Write};

    let db = AgentDB::open(path)?;
    let server = McpServer::new(db);

    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            agentdb::AgentDbError::InvalidArgument(format!("stdin read error: {e}"))
        })?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_message(&line) {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        }
    }

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

            // Route non-SELECT statements to execute() so rows-affected is shown.
            let upper = query.trim_start().to_ascii_uppercase();
            let is_select = upper.starts_with("SELECT")
                || upper.starts_with("WITH")
                || upper.starts_with("PRAGMA")
                || upper.starts_with("EXPLAIN");
            if is_select {
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
            } else {
                match db.execute(&query) {
                    Ok(n) => println!("({n} row{} affected)", if n == 1 { "" } else { "s" }),
                    Err(e) => eprintln!("error: {e}"),
                }
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
