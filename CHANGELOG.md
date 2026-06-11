# Changelog

All notable changes to AgentDB are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.3.0] — 2026-06-11 — Universal Availability

### Added
- **C FFI** (`src/ffi.rs`): flat `extern "C"` API covering open/close, SQL execute/query, vector upsert/search, graph add\_node/add\_edge/neighbors, FTS index/search, hybrid query, stats. Every function sets a thread-local last-error readable via `agentdb_last_error()`.
- **cbindgen** (`cbindgen.toml`): auto-generates `agentdb.h` with full Doxygen comments; driven by `ffi-header.yml` CI workflow.
- **Python bindings** (`python/`): PyO3 + maturin. `AgentDB`, `Collection`, `SearchResult`, `FtsResult`, `HybridResult` classes. `pyproject.toml` ready for `maturin publish` to PyPI. Wheels for CPython 3.9+, manylinux, macOS, Windows.
- **Node.js bindings** (`nodejs/`): napi-rs. Full TypeScript type definitions (`index.d.ts`). Platform-aware binary loader (`index.js`). `build.rs` required by napi-build.
- **WASM** (`src/wasm.rs`): `WasmAgentDB` class via wasm-bindgen. In-memory databases work today; OPFS persistence tracked for v0.4.0.
- **CLI binary** (`src/bin/agentdb.rs`): subcommands `stats`, `collections`, `sql`, `search`, `reindex`, `inspect`.
- **`ffi` feature flag**: gates `cdylib` output and `src/ffi.rs`. rlib-only builds (default) work everywhere without a C linker.
- **`python` feature flag**: enables PyO3 + ffi.
- **`wasm` feature flag**: enables wasm-bindgen.
- **`clap` dependency**: CLI argument parsing (only pulled in for the binary target).
- **Python quick-start example** (`python/examples/agent_memory.py`): covers SQL, vector upsert/search, graph, FTS, hybrid query.
- **Node.js/TypeScript quick-start example** (`nodejs/examples/agent_memory.ts`): full API walkthrough in TypeScript.
- **BENCHMARKS.md**: baseline Criterion results for 100k-vector ANN, graph traversal, hybrid query, and FTS on GitHub Actions runners.
- **CONTRIBUTING.md**: development setup, PR process, code standards.
- **SECURITY.md**: vulnerability reporting process and supported versions.
- **Issue templates** (`.github/ISSUE_TEMPLATE/`): bug report and feature request forms.
- **Pull request template** (`.github/PULL_REQUEST_TEMPLATE.md`).
- **Dependabot config** (`.github/dependabot.yml`): weekly updates for Cargo, npm, pip, and GitHub Actions.

### CI Workflows added
- `publish.yml`: publish to crates.io on `vX.Y.Z` tag.
- `python-publish.yml`: build manylinux + macOS + Windows wheels, publish to PyPI via OIDC trusted publishing.
- `nodejs-publish.yml`: build native addons for all platforms, publish to npm.
- `ffi-header.yml`: regenerate `agentdb.h` via cbindgen on every `src/ffi.rs` change.
- `wasm.yml`: `wasm-pack build --features wasm` + binary smoke test on every push.

### Integration tests added
- `tests/test_ffi.rs`: 8 tests covering all FFI entry points (requires `--features ffi`).
- `tests/test_cli.rs`: 7 tests driving the compiled `agentdb` binary as a subprocess.

### Changed
- `Cargo.toml`: version bumped to `0.3.0`; license corrected to `Unlicense` (matching LICENSE file and README); `documentation` field removed until crates.io publish establishes the docs.rs page.
- `python/pyproject.toml`: version `0.3.0`; license corrected to The Unlicense.
- `nodejs/package.json`: version `0.3.0`; license corrected to `Unlicense`; `test` script added.
- `README.md`: version badge updated to v0.3.0; CI and Codecov badges added; cargo.toml snippet updated to `agentdb = "0.3"`.

---

## [0.2.0] — 2026-05-22 — Query Power

### Added
- **Advanced metadata filtering** (`src/filter.rs`): `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists` operators against vector metadata JSON. Applied post-ANN with a 10× over-fetch.
- **Hybrid queries** (`src/hybrid.rs`): single call combining recursive graph traversal (up to N hops from an anchor node) with ANN vector search, blended by alpha. Ranking formula: `rank = alpha × vector_similarity + (1 - alpha) × graph_weight`.
- **Full-text search** (`src/fts.rs`): FTS5 virtual tables (one per collection), BM25 ranking, Porter stemmer, `snippet()` extraction, `optimize()` for segment merging.
- **Batch upsert** (`col.upsert_batch()`): single SQLite transaction, full rollback on any failure, returns rows inserted.
- **Example** `examples/v020_query_power.rs`: demonstrates all four new features end-to-end.
- **Tests** `tests/test_v020.rs`: 20 new integration tests.
- `col.delete(id)` — delete a single vector by ID.
- `VectorStore::drop_collection(name)` — delete a collection and all its vectors.
- `AgentDB::close()` — flush dirty HNSW indexes before dropping.
- `AgentDB::execute_params()` — parameterized SQL execution.

### Fixed
- Self-referencing import in `src/vectors/mod.rs`.
- Missing `VectorStore` re-export in `src/lib.rs`.
- Persistent `rustfmt` CI failures (pinned toolchain to 1.75.0, added `rustfmt.toml`).
- `Cargo.lock` removed from `.gitignore`; bootstrap-lockfile CI job auto-generates and commits it.
- `cargo build --all-targets` replaced with `--lib --tests --examples --benches` to avoid `cdylib` link failures on Windows/macOS.

---

## [0.1.0] — 2026-05-18 — Core

### Added
- `AgentDB::open(path)` — open or create a single-file database.
- **Relational SQL layer**: full SQLite SQL, transactions, indexes, JSON payloads, user-defined tables.
- **Vector store** (`src/vectors/`): pure-Rust HNSW (M=16, ef\_construction=200), cosine / euclidean / dot product, lazy index build serialized to `_adb_hnsw_index`.
- **Memory graph** (`src/memory/`): typed nodes, weighted directed edges, recursive CTE traversal with depth and weight filters.
- **Schema bootstrap** (`src/schema.rs`): `_adb_meta`, `_adb_collections`, `_adb_vectors`, `_adb_hnsw_index`, `_adb_nodes`, `_adb_edges`; WAL mode, foreign keys, `PRAGMA synchronous=NORMAL`.
- **Error type** (`src/error.rs`): `AgentDbError` covering SQLite, serialization, dimension mismatch, node/edge not found, schema migration, corruption, invalid argument.
- **Examples**: `agent_memory`, `rag_pipeline`, `graph_traverse`.
- **Tests**: 29 integration tests across `test_relational.rs`, `test_vectors.rs`, `test_memory_graph.rs`.
- **Benchmarks**: `vector_search` and `graph_traverse` via Criterion.
- **GitHub Actions CI**: lint (rustfmt + clippy), test matrix (ubuntu/macos/windows), security audit, coverage (tarpaulin → Codecov), release binaries on tag, Criterion benchmarks.
- `assets/logo.svg`, `README.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `LICENSE` (public domain), `NOTICE`.

---

[Unreleased]: https://github.com/hvrcharon1/agentdb/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/hvrcharon1/agentdb/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/hvrcharon1/agentdb/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hvrcharon1/agentdb/releases/tag/v0.1.0
