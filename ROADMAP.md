# AgentDB Roadmap

## v0.1.0 — Core ✅
- [x] Schema bootstrap + WAL mode + integrity check
- [x] Relational SQL layer (full SQL, transactions, indexes, JSON)
- [x] Pure Rust HNSW index (cosine, euclidean, dot product)
- [x] Vector collection API — upsert, search, delete, reindex
- [x] Memory graph — typed nodes, weighted edges, recursive CTE traversal
- [x] Examples: agent\_memory, rag\_pipeline, graph\_traverse
- [x] Test suite — 29 tests across relational, vector, and graph layers
- [x] Criterion benchmarks — vector search + graph traversal
- [x] GitHub Actions CI — lint, test (ubuntu/macos/windows), audit, coverage, release

## v0.2.0 — Query Power ✅
- [x] Advanced metadata filtering — `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists`
- [x] Hybrid query — graph traversal + ANN vector search with alpha blending
- [x] Full-text search — FTS5 virtual tables, BM25 ranking, Porter stemmer, snippet extraction
- [x] Batch upsert — single-transaction bulk insert with rollback on failure
- [x] Example: v020\_query\_power (all four features demonstrated)
- [x] Test suite — 20 new tests (filter operators, batch, hybrid ranking)
- [x] CI fixes — resolved self-referencing imports, missing VectorStore, fmt compliance

## v0.3.0 — Developer Experience
- [ ] C FFI flat API + auto-generated `agentdb.h` header via cbindgen
- [ ] CLI binary: `agentdb inspect`, `agentdb stats`, `agentdb export`, `agentdb shell`, `agentdb reindex`
- [ ] Schema migration runner for future schema upgrades
- [ ] `BENCHMARKS.md` — document baseline numbers (100k vectors, 10k nodes)

## v0.4.0 — Language Bindings
- [ ] Node.js bindings via napi-rs (TypeScript types auto-generated)
- [ ] Python bindings via PyO3 + maturin (numpy array support, `.pyi` stubs)
- [ ] WASM build for browser and Cloudflare Workers

## v0.5.0 — Sync
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution: last-write-wins + custom strategies
- [ ] CLI sync commands: `agentdb sync push/pull/watch`
- [ ] Optional cloud replica endpoint

## v1.0.0 — Production
- [ ] Published to crates.io (`cargo add agentdb`)
- [ ] 80%+ test coverage via cargo-tarpaulin
- [ ] ANN search < 50ms on 100k vectors
- [ ] Graph traversal < 10ms on 10k nodes
- [ ] Zero known data corruption scenarios
- [ ] Full docs.rs documentation on all public items
- [ ] Announcement: crates.io, r/rust, Hacker News
