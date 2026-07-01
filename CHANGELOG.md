# Changelog

All notable changes to AgentDB are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.5.2] — 2026-07-01 — Critical Fixes

### Fixed
- **DotProduct ANN results were inverted.** The HNSW search heap used `f32::to_bits()` as `u32` for
  ordering — negative distances (from DotProduct's `-dot` formula) had their sign bit set, making
  them appear "largest" in the max-heap so the best matches were discarded first. Replaced with
  a proper `Ord`-implementing float wrapper that uses `f32::total_cmp()`.
- **HNSW `random_level()` formula was non-standard.** Used a geometric loop instead of the standard
  `floor(-ln(uniform) * m_l)`. This produced a biased level distribution degrading ANN recall.
  Now uses the paper-standard formula capped at level 16.
- **Node.js `addTrace` parameter order was wrong.** TypeScript declared `(parentId, kind, data,
  sessionId)` but the Rust binding takes `(traceType, content, sessionId, parentId)`. Every
  positional caller was scrambling all fields. Fixed the TypeScript declaration.
- **Node.js `Trace` interface had phantom field names.** Declared `kind`, `data`, `children` — the
  actual serialized fields are `traceType`, `content`, and no `children`. Fixed to match output.
- **Node.js `getTraces` pagination params were ignored.** TypeScript declared `limit`/`offset` but
  the binding hardcoded `None, None`. Now passes them through.
- **FTS insert/delete in `ConversationStore` silently swallowed errors** (`let _ = ...`). Messages
  could be stored but not indexed, or ghost FTS entries remained after deletion. Both now propagate
  errors.
- **Cyclic memory graphs caused exponential CTE row explosion.** The recursive traversal used
  `UNION ALL` without visit tracking — bidirectional edges at `max_depth=20` generated 2^20
  intermediate rows. Replaced with an `INSTR`-based visited-path string that prevents revisiting
  nodes. Outer query now uses `GROUP BY n.id` (min depth, max weight) instead of broken `DISTINCT`.
- **`usize::MAX as i64` in traces pagination** — on 64-bit this produced -1, relying on SQLite
  implementation detail. Replaced with `i64::MAX`.
- **Python `search_messages()` called `pythonize` crate which wasn't in Cargo.toml** — broke the
  entire Python binding build. Replaced with the existing `json_to_pyobj` helper.

### Added
- `AsyncMemoryGraph`: `get_node()`, `delete_node()`, `delete_edge()`, `nodes_by_kind()`.
- `AsyncFullTextStore`: `delete_text()`, `optimize()`.
- `nodejs/index.d.ts`: `dropCollection`, `getNode`, `deleteNode`, `deleteEdge`, `ftsDelete`,
  `ftsOptimize` declarations (methods already existed in the binding, just undeclared in types).

---

## [0.5.1] — 2026-06-29 — Enhancement Pass

### Added
- **`search_messages(query, top_k, conversation_id?)`** on `ConversationStore` and `AsyncConversationStore` —
  BM25 full-text search over all message content via a dedicated `_adb_messages_fts` FTS5 virtual table
  (schema v4). Porter stemmer enabled. Supports optional per-conversation scope.
  Exposed in Python (`search_messages`) and Node.js (`searchMessages`).
- **`Workflow.step_count`** — `list_workflows()` now populates a `step_count` field via a single
  `LEFT JOIN … COUNT` query; no extra round-trips. `get_workflow()` sets it from the fetched steps.
  Surfaced in Node.js `listWorkflows()` / `getWorkflow()` as `stepCount`.
- **`TraceStore::get_traces` pagination** — new `limit: Option<usize>` and `offset: Option<usize>`
  parameters. Async wrapper `AsyncTraceStore::get_traces` updated to match. All bindings (Python,
  Node.js, FFI) pass `None, None` for backward-compatible behaviour.
- **`MessageSearchResult`** type re-exported from `agentdb` crate root.
- **`nodejs/index.d.ts`**: `MessageSearchResult` interface, `Workflow.stepCount`, `searchMessages()`,
  updated `getTraces(sessionId, limit?, offset?)`.
- **Schema v4** — adds `_adb_messages_fts` FTS5 virtual table; `migrate()` handles upgrade.
- New tests: trace pagination (3), message FTS search (5), workflow step_count (3).

---

## [0.5.0] — 2026-06-29 — Quality & Correctness

### Fixed
- **`$regex` filter** was performing a substring match instead of real regex matching.
  Real `regex::Regex` evaluation is now used; the old substring behaviour is preserved as the new `$contains` operator.
- **`DotProduct` distance metric** was computing the same value as cosine (normalised dot product).
  Now computes the raw dot product of unnormalised vectors.
- **`fail_workflow()`** was writing the error message to the `output` column instead of `error`.
  A dedicated `error TEXT` column has been added to `_adb_workflows` (schema v3).
- **`AsyncAgentDB::close()`** was panicking when other `Arc` references existed.
  Now returns `Err(InvalidArgument)` with the reference count.
- **`schema::check_version`** was silently returning `Ok(())` when no schema version row existed.
  Now returns `Err(SchemaMigration)` for missing or corrupt metadata.

### Added
- **`$contains` filter operator** — substring match, preserving the previous `$regex` behaviour.
- **`error` column on `_adb_workflows`** (schema v3) — separates error messages from output payloads.
- **`updated_at` column on `_adb_vectors`** (schema v3) — tracks when each vector was last upserted.
- **`create_workflow` `metadata` parameter** — pass arbitrary JSON metadata at creation time.
- **`schema::migrate(conn)`** — public function for in-place schema upgrades (`agentdb migrate <path>`).
- **`impl Drop for AgentDB`** — best-effort flush of dirty HNSW indexes on drop.
- **`DbStats` extended** to all 9 fields: conversations, messages, workflows, workflow_steps, traces.
  Single-query implementation replaces 9 separate round-trips.
- **WASM `stats()`** now returns all 9 fields (was 4).
- **CLI `stats` and `inspect`** print all 9 stat fields.
- **CLI shell** routes INSERT/UPDATE/DELETE/CREATE to `execute()` (shows "N rows affected") instead of silently returning an empty array.

### Changed
- Schema version bumped from 2 → 3. Existing v2 databases must be migrated:
  `agentdb migrate <path>` or `agentdb::schema::migrate(&conn)`.
- Python `__version__` updated to `"0.5.0"`.
- `ROADMAP.md`: v0.5.0 milestone marked complete; ecosystem integrations moved to v0.6.0.
- `ARCHITECTURE.md`: corrected all internal table names (`agentdb_*` → `_adb_*`), updated
  API method names for layers 5–7, and corrected Layer 8 description.
- `MIGRATION.md`: v0.3.x→v0.4.0 section updated to use real API method signatures.

---

## [0.4.5] — 2026-06-19

### Fixed
- Chocolatey publish workflow: corrected secret name (`CHOCO_API_KEY`)
- Python publish workflow: switched to Rust stable toolchain, fixed maturin readme path, corrected module directory structure, replaced container build with `maturin-action`
- Node.js publish workflow: switched to Rust stable toolchain, added `--ignore-scripts` for publish step
- crates.io publish workflow: switched to Rust stable toolchain

### Changed
- All publish workflows now use Rust stable instead of pinned 1.75 for wheel/addon builds (transitive deps require edition2024)

---

## [0.4.0] — 2026-06-19 — AI-Native Features + Multi-Language SDKs

### Added
- **Conversation threading** (`src/conversations.rs`): first-class `_adb_conversations` and `_adb_messages` tables with `ConversationStore` API — create threads, append messages with role/content/metadata, query chronologically.
- **Workflow persistence** (`src/workflows.rs`): `_adb_workflows` and `_adb_workflow_steps` tables with `WorkflowStore` API — create durable workflows, add/update steps with status tracking, complete/fail workflows.
- **Reasoning traces** (`src/traces.rs`): `_adb_traces` table with `TraceStore` API — tree-structured chain-of-thought, tool call logs, decision traces with recursive CTE traversal.
- **Transaction API**: `db.transaction(|tx| { ... })` for multi-operation ACID closures, `db.execute_batch(sql)` for atomic multi-statement execution.
- **Interactive CLI shell** (`agentdb shell <path>` / `agentdb -i <path>`): readline-style REPL with multi-line SQL, dot-commands (`.stats`, `.collections`, `.inspect`, `.help`, `.quit`), and graceful Ctrl+C/Ctrl+D handling.
- **Dockerfile**: multi-stage build (rust:1.75-slim builder + debian:bookworm-slim runtime), OCI labels, VOLUME /data, under 30MB final image.
- **`.dockerignore`**: excludes target/, .git/, node_modules/, *.agentdb from build context.
- **Go SDK** (`go/`): cgo wrapper covering full FFI surface — Open, Execute, QueryJSON, Stats, vector upsert/search, graph, FTS, hybrid query.
- **Java SDK** (`java/`): JNI wrapper with Maven POM — AgentDB.open(), execute(), queryJson(), full vector/graph/FTS/hybrid API, try-with-resources support.
- **C# / .NET SDK** (`dotnet/`): P/Invoke wrapper with NuGet project — IDisposable pattern, full FFI coverage, .NET 8 class library.

### Changed
- Schema version bumped to 2 (adds 7 new tables + 5 indexes alongside existing schema).
- README: comparison table expanded (25 rows), project structure updated, API reference updated with new stores, Docker/Go/Java/C# install sections added.
- ROADMAP: v0.4.0 marked complete, v0.4.1 split off for WASM persistence + Ruby.

---

## [0.3.4] — 2026-06-19 — CI Stability & Logo Refresh

### Changed
- **Logo redesign**: replaced complex gradient SVG with a clean flat design for better legibility at small sizes.
- **MSRV lockfile stabilization**: re-pinned all transitive deps to Rust 1.75-compatible versions; removed bootstrap-lockfile CI job that was causing race conditions.
- **Source formatting**: reformatted all sources with Rust 1.75.0 `rustfmt` to match CI MSRV expectations.
- **`.gitattributes` added**: enforces LF line endings across all platforms; `.gitignore` cleaned up.

### Fixed
- 3 Clippy warnings resolved (unused imports, redundant clone, needless borrow).
- `Cargo.lock` checksum corruption from bootstrap-lockfile job — now committed directly, no auto-generation.

---

## [0.3.3] — 2026-06-18 — Universal Package Manager Distribution

### Added
- **Homebrew tap** (`brew install hvrcharon1/tap/agentdb`) — macOS + Linux, auto-updated on release.
- **Scoop bucket** (`scoop bucket add agentdb https://github.com/hvrcharon1/scoop-bucket && scoop install agentdb`).
- **Chocolatey** (`choco install agentdb`) — Windows, auto-published on tag.
- **Snap Store** (`snap install agentdb`) — Linux, built from source on tag.
- **WinGet** (`winget install Datacules.AgentDB`) — auto-submits PR to microsoft/winget-pkgs.
- **Nix flake** (`nix run github:hvrcharon1/agentdb`) — reproducible builds from source.
- **install.sh** — `curl -fsSL .../install.sh | sh` with SHA-256 verification (Linux/macOS).
- **install.ps1** — `irm .../install.ps1 | iex` with SHA-256 verification (Windows).
- Linux aarch64 cross-compiled binary in release matrix.

### Changed
- Release workflow now produces proper archives (`*.tar.gz`, `*.zip`) with `checksums-sha256.txt`.
- Release workflow dispatches to homebrew-tap and scoop-bucket for auto-update.
- README install section expanded to cover all 11 distribution channels.

---

## [0.3.2] — 2026-06-18 — Registry Rename + Universal Install

### Changed
- **crates.io package renamed** from `agentdb` to `datacules-agentdb` to avoid conflict with
  the pre-existing crate (cryptopatrick/agentdb). Install with:
  ```bash
  cargo add datacules-agentdb
  ```
  The library name remains `agentdb`, so all `use agentdb::*` imports are unchanged.
- **PyPI package renamed** from `agentdb` to `datacules-agentdb` to avoid conflict with the
  pre-existing package (Team Dotagent/openagent). Install with:
  ```bash
  pip install datacules-agentdb
  ```
  The module name remains `agentdb`, so `import agentdb` is unchanged.
- npm remains `@datacules/agentdb` (already resolved in v0.3.1).
- `python/Cargo.toml`: version bumped from stale `0.2.0` to `0.3.2`.
- `python/__init__.py`: `__version__` corrected from `"0.3.0"` to `"0.3.2"`.
- All manifests version-bumped to `0.3.2`.
- Publish workflows updated for new package names.

---

## [0.3.1] — 2026-06-16 — Registry Fixes + Hotfixes

### Changed
- **npm package renamed** from `agentdb` to `@datacules/agentdb` to avoid conflict with the
  pre-existing `agentdb` npm package (ruvnet/agentic-flow). Update your install and import:
  ```bash
  npm install @datacules/agentdb
  ```
  ```ts
  import { AgentDB } from '@datacules/agentdb';
  ```
- `nodejs/Cargo.toml`: version bumped from stale `0.2.0` to `0.3.1`.
- `nodejs/package.json`: `napi.package.name` set to `@datacules/agentdb` for scoped publish.

### Fixed
- **Node.js `Collection.search()` API mismatch**: The napi binding previously accepted
  separate `(topK, filter)` scalar parameters, contradicting the `index.d.ts` declaration
  of `search(query, options?: SearchOptions)`. Calls using the documented options-object
  form (e.g. `col.search(vec, { topK: 5 })`) were silently broken at the napi layer.
  The binding now correctly accepts `Option<SearchOptions>` matching the TypeScript interface.
- **Node.js `AgentDB.hybridQuery()` API mismatch**: Same issue. Previously took three
  scalar args `(graphDepth, topK, alpha)`; now accepts `Option<HybridOptions>` matching
  `index.d.ts`.
- **`DistanceMetric` silently ignored in Node.js binding**: The `metric` field was exported
  in `index.d.ts` but the napi binding always used `DistanceMetric::Cosine` regardless.
  Now maps `'cosine'` → `Cosine`, `'euclidean'` → `Euclidean`, `'dot'` → `DotProduct`.
- **CI coverage job missing OIDC permission**: `coverage` job in `ci.yml` lacked
  `permissions: id-token: write` for `codecov-action@v4` tokenless upload. Uploads were
  silently failing, preventing the Codecov badge from rendering. Permission added.
- **`nodejs/index.js` fallback scope**: Corrected `@agentdb/${key}` → `@datacules/agentdb-${key}`
  to match the new scoped package name.
- **`nodejs/index.d.ts` JSDoc**: Removed erroneous `await` from `Collection.search()` example;
  the method is synchronous and returns `SearchResult[]` directly.
- **`nodejs/examples/agent_memory.ts`**: Updated import from `'agentdb'` to `'@datacules/agentdb'`.

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
- **MIGRATION.md**: migration guide covering all versions from v0.1.0 through v0.3.0.
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
- `README.md`: version badge updated to v0.3.0; CI and Codecov badges added; cargo.toml snippet updated to `agentdb = "0.3"`; license badge corrected from `Public Domain` to `Unlicense`.

---

## [0.2.0] — 2026-05-22 — Query Power

### Added
- **Advanced metadata filtering** (`src/filter.rs`): `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists` operators against vector metadata JSON. Applied post-ANN with a 10× over-fetch.
- **Hybrid queries** (`src/hybrid.rs`): single call combining recursive graph traversal (up to N hops from an anchor node) with ANN vector search, blended by alpha. Ranking formula: `rank = alpha × vector_similarity + (1 - alpha) × graph_weight`.
- **Full-text search** (`src/fts.rs`): FTS5 virtual tables (one per collection), BM25 ranking, Porter stemmer, `snippet()` extraction, `optimize()` for segment merging.
- **Batch upsert** (`col.upsert_batch()`): single atomic transaction, full rollback on any failure, returns rows inserted.
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
- **Relational SQL layer**: full SQL, transactions, indexes, JSON payloads, user-defined tables.
- **Vector store** (`src/vectors/`): pure-Rust HNSW (M=16, ef\_construction=200), cosine / euclidean / dot product, lazy index build serialized to `_adb_hnsw_index`.
- **Memory graph** (`src/memory/`): typed nodes, weighted directed edges, recursive CTE traversal with depth and weight filters.
- **Schema bootstrap** (`src/schema.rs`): `_adb_meta`, `_adb_collections`, `_adb_vectors`, `_adb_hnsw_index`, `_adb_nodes`, `_adb_edges`; WAL mode, foreign keys, `PRAGMA synchronous=NORMAL`.
- **Error type** (`src/error.rs`): `AgentDbError` covering storage errors, serialization, dimension mismatch, node/edge not found, schema migration, corruption, invalid argument.
- **Examples**: `agent_memory`, `rag_pipeline`, `graph_traverse`.
- **Tests**: 29 integration tests across `test_relational.rs`, `test_vectors.rs`, `test_memory_graph.rs`.
- **Benchmarks**: `vector_search` and `graph_traverse` via Criterion.
- **GitHub Actions CI**: lint (rustfmt + clippy), test matrix (ubuntu/macos/windows), security audit, coverage (tarpaulin → Codecov), release binaries on tag, Criterion benchmarks.
- `assets/logo.svg`, `README.md`, `ARCHITECTURE.md`, `ROADMAP.md`, `LICENSE` (public domain), `NOTICE`.

---

[Unreleased]: https://github.com/hvrcharon1/agentdb/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/hvrcharon1/agentdb/compare/v0.4.5...v0.5.0
[0.4.5]: https://github.com/hvrcharon1/agentdb/compare/v0.4.0...v0.4.5
[0.4.4]: https://github.com/hvrcharon1/agentdb/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/hvrcharon1/agentdb/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/hvrcharon1/agentdb/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/hvrcharon1/agentdb/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/hvrcharon1/agentdb/compare/v0.3.4...v0.4.0
[0.3.4]: https://github.com/hvrcharon1/agentdb/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/hvrcharon1/agentdb/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/hvrcharon1/agentdb/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/hvrcharon1/agentdb/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/hvrcharon1/agentdb/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/hvrcharon1/agentdb/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/hvrcharon1/agentdb/releases/tag/v0.1.0
