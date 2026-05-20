# AgentDB Roadmap

## v0.1.0 — Core (Current)
- [x] Schema bootstrap + WAL mode + integrity check
- [x] Relational SQL layer
- [x] Pure Rust HNSW index (cosine, euclidean, dot product)
- [x] Vector collection API (upsert, search, delete, reindex)
- [x] Memory graph — nodes, edges, recursive CTE traversal
- [x] Agent memory example

## v0.2.0 — Query Power
- [ ] Metadata filtering on vector search
- [ ] Hybrid query — graph-weighted + vector ranked results
- [ ] Full-text search integration (SQLite FTS5)
- [ ] Batch upsert API

## v0.3.0 — Developer Experience
- [ ] C FFI flat API + auto-generated `agentdb.h`
- [ ] Criterion benchmarks (100k vectors, 10k graph nodes)
- [ ] CLI: `agentdb inspect`, `agentdb stats`, `agentdb export`
- [ ] Schema migration system

## v0.4.0 — Bindings
- [ ] Node.js bindings via napi-rs
- [ ] Python bindings via PyO3
- [ ] WASM build for browser/edge

## v0.5.0 — Sync
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution strategies
- [ ] Optional cloud replica

## v1.0.0 — Production
- [ ] Publish to crates.io
- [ ] 80%+ test coverage
- [ ] Performance: <50ms ANN search on 100k vectors
- [ ] Zero known data corruption scenarios
