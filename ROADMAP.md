# AgentDB Roadmap

## v0.1.0 — Core ✅
- [x] Schema bootstrap + WAL mode + integrity check
- [x] Relational SQL layer
- [x] Pure Rust HNSW index (cosine, euclidean, dot product)
- [x] Vector collection API (upsert, search, delete, reindex)
- [x] Memory graph — nodes, edges, recursive CTE traversal
- [x] Agent memory, RAG pipeline, graph traversal examples
- [x] Comprehensive test suite (30 test cases)
- [x] Criterion benchmarks (vector search + graph traversal)
- [x] GitHub Actions CI (lint, test, audit, coverage, release)

## v0.2.0 — Query Power ✅
- [x] Advanced metadata filtering ($gt, $gte, $lt, $lte, $eq, $ne, $in, $nin, $exists)
- [x] Hybrid query — graph-weighted + vector ranked results (alpha blending)
- [x] Full-text search via FTS5 virtual tables (BM25 ranking, Porter stemming)
- [x] Batch upsert API — single-transaction bulk insert
- [x] v0.2.0 example + full test suite (filter, batch, hybrid)

## v0.3.0 — Developer Experience
- [ ] C FFI flat API + auto-generated `agentdb.h` via cbindgen
- [ ] CLI: `agentdb inspect`, `agentdb stats`, `agentdb export`, `agentdb shell`, `agentdb reindex`
- [ ] Schema migration runner for future schema upgrades
- [ ] `BENCHMARKS.md` with baseline numbers

## v0.4.0 — Bindings
- [ ] Node.js bindings via napi-rs (TypeScript types included)
- [ ] Python bindings via PyO3 + maturin (numpy array support)
- [ ] WASM build for browser and Cloudflare Workers

## v0.5.0 — Sync
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution: last-write-wins + custom strategies
- [ ] CLI sync: `agentdb sync push/pull/watch`

## v1.0.0 — Production
- [ ] Published to crates.io
- [ ] 80%+ test coverage (cargo tarpaulin)
- [ ] ANN search < 50ms on 100k vectors
- [ ] Graph traversal < 10ms on 10k nodes
- [ ] Zero known data corruption scenarios
- [ ] Full docs.rs documentation
- [ ] Announced on crates.io, r/rust, HN
