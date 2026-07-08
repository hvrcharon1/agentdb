# AgentDB Roadmap

## v0.1.0 — Core ✅
- [x] Schema bootstrap + WAL mode + integrity check
- [x] Relational SQL layer (full SQL, transactions, indexes, JSON)
- [x] Pure Rust HNSW index (cosine, euclidean, dot product)
- [x] Vector collection API — upsert, search, delete, reindex
- [x] Memory graph — typed nodes, weighted edges, recursive CTE traversal
- [x] Examples: agent_memory, rag_pipeline, graph_traverse
- [x] Test suite — 29 tests across relational, vector, and graph layers
- [x] Criterion benchmarks — vector search + graph traversal
- [x] GitHub Actions CI — lint, test (ubuntu/macos/windows), audit, coverage, release

## v0.2.0 — Query Power ✅
- [x] Advanced metadata filtering — `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists`
- [x] Hybrid query — graph traversal + ANN vector search with alpha blending
- [x] Full-text search — FTS5 virtual tables, BM25 ranking, Porter stemmer, snippet extraction
- [x] Batch upsert — single-transaction bulk insert with rollback on failure
- [x] Example: v020_query_power (all four features demonstrated)
- [x] Test suite — 20 new tests (filter operators, batch, hybrid ranking)
- [x] CI fixes — resolved self-referencing imports, missing VectorStore, fmt compliance
- [x] Cargo.lock committed — reproducible builds, working CI cache keys
- [x] cdylib gated behind `ffi` feature — rlib builds work everywhere without a C linker

## v0.3.0 — Universal Availability ✅
- [x] C FFI flat API (`src/ffi.rs`) — open, close, SQL, vector, graph, FTS, hybrid, stats
- [x] `cbindgen.toml` + `ffi-header.yml` CI — auto-generate `agentdb.h` on every FFI change
- [x] CLI binary (`src/bin/agentdb.rs`) — stats, collections, sql, search, reindex, inspect
- [x] Python bindings (`python/`) — PyO3 + maturin, PyPI-ready, manylinux + macOS + Windows wheels
- [x] Node.js bindings (`nodejs/`) — napi-rs, TypeScript types, platform-aware loader
- [x] WASM stub (`src/wasm.rs`) — in-memory databases work today via wasm-pack
- [x] `publish.yml` — crates.io publish on tag
- [x] `python-publish.yml` — PyPI OIDC publish on tag
- [x] `nodejs-publish.yml` — npm publish on tag
- [x] `wasm.yml` — wasm-pack build + binary smoke test on every push
- [x] FFI integration tests (`tests/test_ffi.rs`) — 8 tests
- [x] CLI integration tests (`tests/test_cli.rs`) — 7 tests
- [x] CHANGELOG.md — full history from v0.1.0 through v0.3.0

## v0.4.0 — AI-Native Features + Multi-Language SDKs ✅
- [x] Conversation/message threading layer (`_adb_conversations`, `_adb_messages`)
- [x] Workflow persistence layer (`_adb_workflows`, `_adb_workflow_steps`)
- [x] Reasoning traces layer (`_adb_traces`) — tree-structured chain-of-thought
- [x] Transaction API (`db.transaction()`, `db.execute_batch()`)
- [x] Interactive CLI shell (`agentdb shell <path>`, `agentdb -i <path>`)
- [x] Go SDK via cgo wrapping the C FFI layer (`go/`)
- [x] Java SDK via JNI wrapping the C FFI layer (`java/`)
- [x] C# / .NET SDK via P/Invoke wrapping the C FFI layer (`dotnet/`)
- [x] Dockerfile — multi-stage build, minimal runtime image
- [x] Docker support (Linux, macOS, Windows, WSL)

## v0.4.1 — WASM Persistence + Ruby
- [ ] OPFS (Origin Private File System) adapter — persistent browser storage
- [ ] Cloudflare Workers target (Durable Objects storage backend)
- [ ] Ruby gem via `ffi` wrapping the C FFI layer
- [ ] `BENCHMARKS.md` — baseline numbers (100k vectors, 10k nodes, all platforms)
- [ ] Schema migration runner for future schema upgrades

## v0.5.0 — Quality & Correctness ✅
- [x] Schema v3: `error` column on `_adb_workflows`; `updated_at` on `_adb_vectors`
- [x] `create_workflow` now accepts optional `metadata` parameter
- [x] `fail_workflow` correctly stores error message (was writing to `output` column)
- [x] `DotProduct` distance metric fixed (was identical to cosine)
- [x] `$regex` filter operator uses real regex matching (was substring match); `$contains` added for substring
- [x] `AsyncAgentDB::close()` returns error instead of panicking when other references exist
- [x] `schema::check_version` now errors on missing schema version
- [x] `schema::migrate()` public function for in-place upgrades
- [x] `DbStats` extended to all 9 fields; single-query implementation
- [x] CLI `stats` and `inspect` print all 9 stat fields
- [x] CLI shell routes INSERT/UPDATE/DELETE to `execute()` (shows rows affected)
- [x] `impl Drop for AgentDB` flushes dirty HNSW indexes on drop
- [x] WASM `stats()` returns all 9 fields

## v0.5.1 — Enhancement Pass ✅
- [x] `search_messages(query, top_k, conversation_id?)` — BM25 full-text search over messages
- [x] Schema v4: `_adb_messages_fts` FTS5 virtual table
- [x] `Workflow.step_count` populated via LEFT JOIN COUNT
- [x] `TraceStore::get_traces` pagination (limit, offset)
- [x] Python + Node.js bindings updated

## v0.5.2 — Critical Fixes ✅
- [x] DotProduct HNSW heap ordering fix (sign bit inversion)
- [x] HNSW `random_level()` now uses paper-standard formula
- [x] Node.js `addTrace` parameter order corrected
- [x] Cyclic memory graph CTE explosion fixed (INSTR-based visit tracking)
- [x] FTS insert/delete in ConversationStore now propagates errors
- [x] Python `search_messages()` build fix (removed `pythonize` dependency)

## v0.5.3 — API Ergonomics & Serde ✅
- [x] `query_json_params()` — parameterized SQL queries (core, async, FFI, Node.js)
- [x] `Clone` derive on `AgentDB`
- [x] `Serialize`/`Deserialize` on all public data types
- [x] `metadata` param on `create_workflow` (FFI, Node.js, Go)
- [x] `relation` filter on graph traversal (FFI, Node.js, Go, Java, C#)
- [x] `close()` / `Drop` double-flush eliminated (AtomicBool guard)
- [x] Vector search N+1 metadata queries → single batch query
- [x] Clippy-clean: `agentdb_open` marked `unsafe`, `const` thread_local

## v0.6.0 — AI-Native Architecture ✅
- [x] Tool Registry (`_adb_tools`, `_adb_tool_calls`) — register, list, and log tool invocations
- [x] Audit Log (`_adb_audit_log`) — actor/action/old/new/reason change trail
- [x] Context Window (`_adb_context_entries`) — token-budgeted session context builder
- [x] Prompt Templates (`_adb_prompt_templates`) — versioned templates with variable rendering
- [x] Data Labels (`_adb_data_labels`) — privacy/compliance tagging on any record
- [x] MCP Server (`src/mcp.rs`) — Model Context Protocol server wrapping all layers

## v0.7.0 — Ecosystem Integrations ✅
- [x] `langchain-agentdb` Python package — implements `VectorStore` + `Memory` base classes
- [x] LlamaIndex storage adapter
- [x] AgentDB Sync — CRDT-based replication protocol
- [x] Conflict resolution: last-write-wins + custom strategies
- [x] Tri-modal hybrid query (vector + graph + FTS weighted blending)
- [x] OPFS (Origin Private File System) persistence for WASM
- [x] Ruby gem via `ffi` wrapping the C FFI layer
- [x] Comprehensive test suites (async API, Node.js, Python)

## v0.8.0 — WASM Persistence + Ruby SDK
- [ ] OPFS (Origin Private File System) adapter — persistent browser storage
- [ ] Cloudflare Workers target (Durable Objects storage backend)
- [ ] Ruby gem via `ffi` wrapping the C FFI layer

## v1.0.0 — Production
- [ ] Published to crates.io (`cargo add agentdb`)
- [ ] Published to PyPI (`pip install agentdb`)
- [ ] Published to npm (`npm install agentdb`)
- [ ] 80%+ test coverage via cargo-tarpaulin
- [ ] ANN search < 50ms on 100k vectors
- [ ] Graph traversal < 10ms on 10k nodes
- [ ] Zero known data corruption scenarios
- [ ] Full docs.rs documentation on all public items
- [ ] `agentdb.h` published as a standalone release asset
- [ ] Announcement: crates.io, PyPI, npm, r/rust, Hacker News
