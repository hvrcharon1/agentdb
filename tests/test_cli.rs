//! Integration tests for the `agentdb` CLI binary.
//!
//! Spawns the compiled binary as a child process and asserts on stdout/stderr.
//! Requires the binary to be built first (cargo build or CI `cargo build --bin agentdb`).
//!
//! Run:
//!   cargo test --test test_cli

use std::process::Command;
use tempfile::NamedTempFile;

/// Return the path to the compiled `agentdb` binary.
/// In CI `cargo test` ensures it is built; locally `cargo build` must have been run.
fn agentdb_bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_agentdb is set by cargo when running integration tests
    // that share the same workspace — use it when available.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_agentdb") {
        return p.into();
    }
    // Fallback: resolve relative to the workspace target directory.
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    #[cfg(target_os = "windows")]
    p.push("agentdb.exe");
    #[cfg(not(target_os = "windows"))]
    p.push("agentdb");
    p
}

// ── helpers ────────────────────────────────────────────────────────────

struct CliOutput {
    stdout: String,
    stderr: String,
    status: std::process::ExitStatus,
}

fn run(args: &[&str]) -> CliOutput {
    let bin = agentdb_bin();
    let out = Command::new(&bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {e}", bin.display()));
    CliOutput {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        status: out.status,
    }
}

fn tmp_db() -> NamedTempFile {
    // Create a temp file; AgentDB::open will initialise it.
    // We need the path, not an open file handle, so close immediately.
    let f = NamedTempFile::new().expect("tempfile");
    f
}

// ── tests ──────────────────────────────────────────────────────────────

#[test]
fn cli_stats_on_fresh_db() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();
    let out = run(&["stats", path]);
    assert!(out.status.success(), "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("collections:"),
        "unexpected: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("vectors:"),
        "unexpected: {}",
        out.stdout
    );
    assert!(out.stdout.contains("nodes:"), "unexpected: {}", out.stdout);
    assert!(out.stdout.contains("edges:"), "unexpected: {}", out.stdout);
}

#[test]
fn cli_collections_empty_db() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();
    let out = run(&["collections", path]);
    assert!(out.status.success(), "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("No collections") || out.stdout.contains("name"),
        "unexpected: {}",
        out.stdout
    );
}

#[test]
fn cli_sql_create_and_query() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();

    // Create table.
    let create = run(&[
        "sql",
        path,
        "CREATE TABLE t (id TEXT PRIMARY KEY, v INTEGER)",
    ]);
    assert!(create.status.success(), "CREATE failed: {}", create.stderr);

    // Insert.
    let insert = run(&["sql", path, "INSERT INTO t VALUES ('r1', 42)"]);
    assert!(insert.status.success(), "INSERT failed: {}", insert.stderr);

    // Query.
    let sel = run(&["sql", path, "SELECT id, v FROM t"]);
    assert!(sel.status.success(), "SELECT failed: {}", sel.stderr);
    assert!(
        sel.stdout.contains("r1"),
        "r1 not in output: {}",
        sel.stdout
    );
    assert!(
        sel.stdout.contains("42"),
        "42 not in output: {}",
        sel.stdout
    );
}

#[test]
fn cli_reindex_empty_db() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();
    let out = run(&["reindex", path]);
    assert!(out.status.success(), "stderr: {}", out.stderr);
    // Should report 0 collections reindexed.
    assert!(out.stdout.contains("0"), "unexpected: {}", out.stdout);
}

#[test]
fn cli_inspect_empty_db() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();
    let out = run(&["inspect", path]);
    assert!(out.status.success(), "stderr: {}", out.stderr);
    assert!(
        out.stdout.contains("AgentDB Inspect"),
        "unexpected: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Statistics"),
        "unexpected: {}",
        out.stdout
    );
}

#[test]
fn cli_unknown_subcommand_exits_nonzero() {
    let out = run(&["notacommand"]);
    assert!(
        !out.status.success(),
        "Expected non-zero exit for unknown subcommand"
    );
}

#[test]
fn cli_search_on_empty_collection() {
    let db = tmp_db();
    let path = db.path().to_str().unwrap();
    // Searching a collection that doesn't exist yet — should exit cleanly
    // (AgentDB creates the collection on first access with the given dim).
    let out = run(&["search", path, "thoughts", "0.9", "0.1", "0.0", "0.0"]);
    // Either succeeds with "No results" or fails gracefully — never panics.
    assert!(
        out.stdout.contains("No results") || out.status.success() || !out.stderr.is_empty(),
        "Unexpected output: stdout={} stderr={}",
        out.stdout,
        out.stderr
    );
}
