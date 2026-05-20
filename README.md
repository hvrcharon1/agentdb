# AgentDB

> **SQLite for AI agents.** A single-file embedded database combining relational SQL, vector search, and episodic memory graphs — built in Rust.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![GitHub](https://img.shields.io/badge/github-hvrcharon1%2Fagentdb-lightgrey)](https://github.com/hvrcharon1/agentdb)

---

## Why AgentDB?

Modern AI agents need three things that no single database provides today:

| Need | Current Solution | Problem |
|------|-----------------|---------||
| Structured data | SQLite | No vectors, no graph |
| Semantic search | ChromaDB / Qdrant | Separate service, no SQL |
| Memory graph | Neo4j / custom | Heavy, not embedded |

**AgentDB puts all three in one `.agentdb` file.** No servers. No daemons. No sync headaches.

---

## Architecture

```
┌─────────────────────────────────────────────┐
│              agent.agentdb                  │
│                                             │
│  ┌──────────────┐  ┌──────────────────────┐ │
│  │  Relational  │  │    Vector Store      │ │
│  │    Layer     │  │  (HNSW index in      │ │
│  │  (SQLite     │  │   BLOB, pure Rust)   │ │
│  │  compatible) │  └──────────────────────┘ │
│  └──────────────┘                           │
│  ┌──────────────────────────────────────┐   │
│  │         Memory Graph Layer           │   │
│  │  (nodes + edges + recursive CTE      │   │
│  │   traversal via SQLite)              │   │
│  └──────────────────────────────────────┘   │
│                                             │
│  Storage: SQLite WAL mode under the hood    │
└─────────────────────────────────────────────┘
```

---

## Quick Start

```rust
use agentdb::{AgentDB, VectorEntry, TraversalOptions};
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open("agent.agentdb")?;

    // ── Relational layer (SQL) ─────────────────────────────────────
    db.execute("CREATE TABLE IF NOT EXISTS events (
        id TEXT PRIMARY KEY, kind TEXT, data TEXT, ts INTEGER
    )")?;

    // ── Vector layer (semantic search) ────────────────────────────
    let col = db.vectors().collection("thoughts", 1536)?;

    col.upsert(VectorEntry {
        id: "thought_1".into(),
        vector: vec![0.1_f32; 1536],
        metadata: Some(json!({"text": "Rust is fast and memory safe"})),
    })?;

    let results = col.search(&[0.1_f32; 1536], Default::default())?;
    println!("Top match: {}", results[0].id);

    // ── Memory graph layer ────────────────────────────────────────
    let graph = db.memory();

    graph.add_node("session_1", "session", Some(json!({"user": "harshal"})))?;
    graph.add_node("concept_rust", "concept", Some(json!({"label": "Rust"})))?;
    graph.add_edge("session_1", "concept_rust", "discussed", 0.9)?;

    let neighbors = graph.neighbors("session_1", TraversalOptions {
        relation: Some("discussed".into()),
        max_depth: 2,
        min_weight: Some(0.5),
    })?;

    println!("Related concepts: {}", neighbors.len());
    Ok(())
}
```

---

## Comparison

| Feature | AgentDB | SQLite | ChromaDB | Weaviate | Qdrant |
|---------|---------|--------|----------|----------|--------|
| Embedded (no server) | ✅ | ✅ | ❌ | ❌ | ❌ |
| Single file | ✅ | ✅ | ❌ | ❌ | ❌ |
| Relational SQL | ✅ | ✅ | ❌ | ❌ | ❌ |
| Vector search | ✅ | ❌ | ✅ | ✅ | ✅ |
| Memory graph | ✅ | ❌ | ❌ | ✅ | ❌ |
| Rust native | ✅ | ❌ | ❌ | ❌ | ✅ |
| Offline-first | ✅ | ✅ | ❌ | ❌ | ❌ |
| Zero config | ✅ | ✅ | ❌ | ❌ | ❌ |

---

## Project Structure

```
agentdb/
├── src/
│   ├── lib.rs              # Public API
│   ├── db.rs               # AgentDB connection
│   ├── schema.rs           # SQLite meta-table bootstrap
│   ├── error.rs            # AgentDbError types
│   ├── vectors/
│   │   ├── hnsw.rs         # Pure Rust HNSW index
│   │   └── collection.rs   # Vector collection API
│   └── memory/
│       └── graph.rs        # Memory graph (nodes, edges, traversal)
├── examples/
│   ├── agent_memory.rs     # Full agent memory demo
│   ├── rag_pipeline.rs     # RAG with local vector search
│   └── graph_traverse.rs   # Graph traversal demo
├── tests/
└── benches/
```

---

## Roadmap

See [GitHub Issues](https://github.com/hvrcharon1/agentdb/issues) for the full roadmap.

- [x] Schema bootstrap + WAL mode
- [x] Relational SQL layer
- [x] Pure Rust HNSW vector index
- [x] Vector collection API (upsert, search, delete, reindex)
- [x] Memory graph (nodes, edges, recursive CTE traversal)
- [ ] Metadata filtering on vector search
- [ ] Hybrid query (graph + vector ranked results)
- [ ] C FFI flat API + auto-generated header
- [ ] Criterion benchmarks
- [ ] CLI — `agentdb inspect`, `agentdb stats`, `agentdb export`
- [ ] Node.js bindings (napi-rs)
- [ ] Python bindings (PyO3)
- [ ] AgentDB Sync — CRDT-based replication

---

## License

MIT — see [LICENSE](LICENSE)

Built by [Harshal Rasal](https://harshalrasal.com)
