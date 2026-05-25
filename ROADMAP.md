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

## v0.4.0 — WASM Persistence + Go/Ruby Bindings
- [ ] OPFS (Origin Private File System) VFS adapter for SQLite — persistent browser storage
- [ ] Cloudflare Workers target (Durable Objects storage backend)
- [ ] Go bindings via cgo wrapping the C FFI layer
- [ ] Ruby gem via `ffi` wrapping the C FFI layer
- [ ] `BENCHMARKS.md` — baseline numbers (100k vectors, 10k nodes, all platforms)
- [ ] Schema migration runner for future schema upgrades

## v0.5.0 — Ecosystem Integrations
- [ ] `langchain-agentdb` Python package — implements `VectorStore` + `Memory` base classes
- [ ] LlamaIndex storage adapter
- [ ] MCP (Model Context Protocol) server wrapping all five layers
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution: last-write-wins + custom strategies
- [ ] CLI sync commands: `agentdb sync push/pull/watch`

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
