<p align="center">
  <img src="./assets/logo.svg" alt="AgentDB" width="420"/>
</p>

<p align="center">
  <b>The embedded database built for AI agents.</b><br/>
  One file. Three layers. Zero servers.<br/>
  Relational SQL &nbsp;·&nbsp; Vector Search &nbsp;·&nbsp; Memory Graphs — all in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-Public%20Domain-brightgreen.svg" alt="License"/>
  &nbsp;
  <img src="https://img.shields.io/badge/language-Rust%202021-orange.svg" alt="Rust"/>
  &nbsp;
  <img src="https://img.shields.io/badge/status-alpha-blue.svg" alt="Status"/>
  &nbsp;
  <img src="https://img.shields.io/badge/by-Datacules%20LLC-lightgrey.svg" alt="Datacules LLC"/>
</p>

---

## Table of Contents

- [Overview](#overview)
- [Why AgentDB?](#why-agentdb)
- [Architecture](#architecture)
- [The Three Layers](#the-three-layers)
  - [Layer 1 — Relational SQL](#layer-1--relational-sql)
  - [Layer 2 — Vector Store](#layer-2--vector-store)
  - [Layer 3 — Memory Graph](#layer-3--memory-graph)
- [Quick Start](#quick-start)
- [API Reference](#api-reference)
- [Internal Schema](#internal-schema)
- [Comparison](#comparison)
- [Project Structure](#project-structure)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Overview

AgentDB is a single-file, embedded database engine written in Rust, purpose-built for AI agents and LLM-powered applications. It unifies three storage primitives that AI applications need — structured queries, semantic vector search, and episodic memory graphs — into one self-contained `.agentdb` file.

There is no server to run. No daemon to manage. No network to configure. You open a file and start building.

```rust
let db = AgentDB::open("agent.agentdb")?;
```

That single line gives your agent a full relational database, a vector index, and a traversable memory graph — all persisted to a single file on disk.

---

## Why AgentDB?

Modern AI agents have three distinct data needs, and today every one of them requires a separate tool:

| What the agent needs | Today's solution | The problem |
|---|---|---|
| Store structured events, sessions, logs | Relational database | No vector search, no graph |
| Semantic similarity search over memories | ChromaDB, Qdrant, Pinecone | Separate service, no SQL, network required |
| Traverse knowledge and memory relationships | Neo4j, custom graph DB | Heavy, not embeddable, not offline |

Every additional service adds latency, operational complexity, infrastructure cost, and failure points. For edge deployments, mobile agents, or local-first applications, running three separate databases is not viable.

**AgentDB collapses all three into one embedded file.** No services. No ports. No sync headaches. The entire database is a single `.agentdb` file you can copy, move, back up, or delete like any other file.

### Key Properties

- **Embedded** — ships as a Rust library, no standalone process
- **Single file** — the entire database state lives in one `.agentdb` file
- **Zero configuration** — open a path and go, no setup required
- **Offline-first** — works fully without network access
- **ACID compliant** — all writes are durable and transactional
- **WAL mode** — concurrent reads while writes are in progress
- **Pure Rust** — memory safe, no C dependencies in the core engine
- **Public domain** — zero legal friction, use in any project freely

---

## Architecture

AgentDB is built on a layered architecture. All three layers share the same underlying storage engine and co-exist within one `.agentdb` file.

```
┌──────────────────────────────────────────────────────────┐
│                    agent.agentdb                         │
│                                                          │
│  ┌───────────────────────┐  ┌───────────────────────┐   │
│  │   Layer 1: Relational  │  │  Layer 2: Vector Store │   │
│  │                       │  │                       │   │
│  │  Full SQL support     │  │  HNSW index (pure     │   │
│  │  Transactions         │  │  Rust), cosine /      │   │
│  │  Indexes              │  │  euclidean / dot      │   │
│  │  Any user-defined     │  │  Stored as BLOBs,     │   │
│  │  tables               │  │  lazy-built index     │   │
│  └───────────────────────┘  └───────────────────────┘   │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │              Layer 3: Memory Graph               │   │
│  │                                                  │   │
│  │  Typed nodes (session, concept, entity, ...)     │   │
│  │  Weighted, labeled directed edges                │   │
│  │  Depth-limited traversal via recursive queries   │   │
│  │  Weight and relation filtering                   │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  Internal meta-tables: _adb_meta, _adb_collections,      │
│  _adb_vectors, _adb_hnsw_index, _adb_nodes, _adb_edges   │
│                                                          │
│  WAL mode · ACID · Foreign keys · Concurrent reads       │
└──────────────────────────────────────────────────────────┘
```

The three layers are fully independent — you can use only the relational layer, only vectors, or all three simultaneously. They share a single write lock and WAL journal, so they stay consistent with each other at all times.

---

## The Three Layers

### Layer 1 — Relational SQL

The relational layer gives you a complete SQL engine. Create any tables you need alongside AgentDB's internal tables. Run joins, aggregations, CTEs, and full-text queries. Everything is ACID-compliant and durable.

**Use it for:** conversation history, session metadata, user profiles, event logs, structured agent state, audit trails.

```rust
// Create a table for agent events
db.execute("
    CREATE TABLE IF NOT EXISTS events (
        id       TEXT PRIMARY KEY,
        kind     TEXT NOT NULL,
        payload  TEXT,          -- JSON
        agent_id TEXT NOT NULL,
        ts       INTEGER NOT NULL
    )
")?;

// Insert an event
db.execute_params(
    "INSERT INTO events VALUES (?1, ?2, ?3, ?4, ?5)",
    &[&"evt_001", &"user_message", &r#"{"text":"What is Rust?"}"#, &"agent_42", &1700000000_i64],
)?;

// Query with filtering
let rows = db.query_json("
    SELECT e.id, e.kind, e.ts
    FROM events e
    WHERE e.agent_id = 'agent_42'
    ORDER BY e.ts DESC
    LIMIT 20
")?;
```

**Capabilities:**
- Full SQL: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `DROP`
- Transactions: wrap multiple operations in a single atomic commit
- Indexes: create indexes on any column for fast lookups
- JSON: store and query structured JSON payloads inline
- User tables co-exist safely with internal `_adb_*` tables

---

### Layer 2 — Vector Store

The vector layer stores high-dimensional embeddings alongside their metadata and builds an HNSW (Hierarchical Navigable Small World) index for approximate nearest-neighbor (ANN) search. The index is implemented from scratch in pure Rust — no external C libraries.

**Use it for:** semantic memory search, RAG pipelines, similarity-based retrieval, embedding-based deduplication, clustering agent thoughts.

```rust
// Create a collection (dim=1536 for OpenAI text-embedding-ada-002)
let col = db.vectors().collection("memories", 1536)?;

// Store an embedding with metadata
col.upsert(VectorEntry {
    id: "mem_001".into(),
    vector: embedding_from_openai("Rust is a systems language"),
    metadata: Some(json!({
        "text": "Rust is a systems language",
        "role": "user",
        "session": "session_42",
        "ts": 1700000000
    })),
})?;

// Semantic search — find the 5 closest memories
let results = col.search(
    &query_embedding,
    SearchOptions {
        top_k: 5,
        metric: DistanceMetric::Cosine,
        filter: Some(json!({ "role": "user" })),
    },
)?;

for r in &results {
    println!("id={} score={:.4}", r.id, r.score);
}
```

**Capabilities:**
- Multiple named collections per database, each with its own dimension and metric
- Distance metrics: cosine similarity, euclidean distance, dot product
- Metadata: attach arbitrary JSON to every vector
- Filtering: filter search results by metadata fields
- Lazy indexing: HNSW index is built on first search, serialized to disk
- Manual reindex: call `col.reindex()` to force a rebuild
- Persistence: index and raw vectors both stored in the `.agentdb` file

**HNSW parameters:**
- `M = 16` — maximum connections per node
- `ef_construction = 200` — search width during index construction
- Supports up to tens of millions of vectors per collection

---

### Layer 3 — Memory Graph

The memory graph layer lets you model relationships between concepts, sessions, entities, and any other typed objects your agent encounters. Nodes and edges are stored in the database with typed labels and weights. Traversal is powered by recursive queries — no in-memory graph library required.

**Use it for:** agent knowledge graphs, concept relationship maps, session-to-concept linking, multi-hop reasoning chains, episodic memory networks.

```rust
let graph = db.memory();

// Add typed nodes
graph.add_node("session_42",    "session", Some(json!({ "user": "harshal", "date": "2025-01-01" })))?;
graph.add_node("concept_rust",  "concept", Some(json!({ "label": "Rust programming" })))?;
graph.add_node("concept_perf",  "concept", Some(json!({ "label": "Performance" })))?;
graph.add_node("concept_safety","concept", Some(json!({ "label": "Memory safety" })))?;

// Connect them with labeled, weighted edges
graph.add_edge("session_42",   "concept_rust",   "discussed",  0.95)?;
graph.add_edge("session_42",   "concept_perf",   "discussed",  0.70)?;
graph.add_edge("concept_rust", "concept_perf",   "relates_to", 0.85)?;
graph.add_edge("concept_rust", "concept_safety", "relates_to", 0.90)?;

// Traverse: what did session_42 discuss, and what does it relate to?
let neighbors = graph.neighbors(
    "session_42",
    TraversalOptions {
        relation:   None,          // all relation types
        max_depth:  2,             // up to 2 hops
        min_weight: Some(0.6),    // only strong connections
    },
)?;

for n in &neighbors {
    println!("depth={} weight={:.2}  {} ({})",
        n.depth, n.weight, n.node.id, n.node.kind);
}
```

**Capabilities:**
- Typed nodes: any string type (`"session"`, `"concept"`, `"entity"`, `"document"`, ...)
- Labeled directed edges: any relation name (`"discussed"`, `"relates_to"`, `"authored"`, ...)
- Weighted edges: float weight from `0.0` to `1.0` expressing connection strength
- Depth-limited traversal: bound recursion to prevent unbounded graph walks
- Relation filtering: traverse only edges matching a specific relation type
- Weight filtering: exclude weak edges below a minimum threshold
- Node lookup by kind: list all nodes of a given type
- Stats: node count and edge count in O(1)

---

## Quick Start

### 1. Add to your project

```toml
# Cargo.toml
[dependencies]
agentdb = { git = "https://github.com/hvrcharon1/agentdb" }
```

### 2. Run the example

```bash
git clone https://github.com/hvrcharon1/agentdb
cd agentdb
cargo run --example agent_memory
```

### 3. Full working example

```rust
use agentdb::{AgentDB, VectorEntry, TraversalOptions, SearchOptions, DistanceMetric};
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open("agent.agentdb")?;

    // ── Layer 1: Relational ────────────────────────────────────────
    db.execute("
        CREATE TABLE IF NOT EXISTS sessions (
            id      TEXT PRIMARY KEY,
            user    TEXT NOT NULL,
            started INTEGER NOT NULL
        )
    ")?;
    db.execute_params(
        "INSERT OR IGNORE INTO sessions VALUES (?1, ?2, ?3)",
        &[&"session_42", &"harshal", &1700000000_i64],
    )?;

    // ── Layer 2: Vectors ───────────────────────────────────────────
    let col = db.vectors().collection("thoughts", 4)?;
    col.upsert(VectorEntry {
        id: "thought_rust".into(),
        vector: vec![0.9, 0.1, 0.05, 0.0],
        metadata: Some(json!({ "text": "Rust is fast", "session": "session_42" })),
    })?;
    col.upsert(VectorEntry {
        id: "thought_ai".into(),
        vector: vec![0.1, 0.05, 0.9, 0.0],
        metadata: Some(json!({ "text": "AI agents need memory", "session": "session_42" })),
    })?;

    let results = col.search(
        &[0.85, 0.1, 0.1, 0.0],
        SearchOptions { top_k: 2, metric: DistanceMetric::Cosine, filter: None },
    )?;
    println!("Vector search top result: {}", results[0].id);

    // ── Layer 3: Memory Graph ──────────────────────────────────────
    let graph = db.memory();
    graph.add_node("session_42",   "session", Some(json!({ "user": "harshal" })))?;
    graph.add_node("concept_rust", "concept", Some(json!({ "label": "Rust" })))?;
    graph.add_node("concept_ai",   "concept", Some(json!({ "label": "AI" })))?;
    graph.add_edge("session_42",   "concept_rust", "discussed", 0.95)?;
    graph.add_edge("session_42",   "concept_ai",   "discussed", 0.80)?;
    graph.add_edge("concept_rust", "concept_ai",   "relates_to", 0.70)?;

    let neighbors = graph.neighbors("session_42", TraversalOptions {
        relation:   Some("discussed".into()),
        max_depth:  2,
        min_weight: Some(0.5),
    })?;
    println!("Graph neighbors: {}", neighbors.len());

    // ── Stats ──────────────────────────────────────────────────────
    let stats = db.stats()?;
    println!("Collections: {}  Vectors: {}  Nodes: {}  Edges: {}",
        stats.collections, stats.vectors, stats.nodes, stats.edges);

    Ok(())
}
```

---

## API Reference

### `AgentDB`

| Method | Description |
|---|---|
| `AgentDB::open(path)` | Open or create a database at the given file path |
| `AgentDB::open(":memory:")` | Open an in-memory database (useful for tests) |
| `db.execute(sql)` | Execute a SQL statement, returns rows changed |
| `db.execute_params(sql, params)` | Execute a parameterized SQL statement |
| `db.query_json(sql)` | Run a SQL query, returns `Vec<serde_json::Value>` |
| `db.vectors()` | Access the vector store layer → `VectorStore` |
| `db.memory()` | Access the memory graph layer → `MemoryGraph` |
| `db.stats()` | Return `DbStats` — collections, vectors, nodes, edges |
| `db.close()` | Flush dirty indexes and close the database gracefully |

### `VectorStore`

| Method | Description |
|---|---|
| `db.vectors().collection(name, dim)` | Get or create a collection with cosine metric |
| `db.vectors().collection_with_metric(name, dim, metric)` | Get or create with explicit metric |
| `db.vectors().list_collections()` | List all collections with name, dim, count |
| `db.vectors().drop_collection(name)` | Delete a collection and all its vectors |

### `Collection`

| Method | Description |
|---|---|
| `col.upsert(entry)` | Insert or update a vector entry |
| `col.search(query, opts)` | ANN search — returns `Vec<SearchResult>` |
| `col.delete(id)` | Delete a vector by ID |
| `col.reindex()` | Force rebuild the HNSW index |
| `col.count()` | Return the number of vectors in the collection |

### `MemoryGraph`

| Method | Description |
|---|---|
| `graph.add_node(id, kind, data)` | Insert or update a node |
| `graph.get_node(id)` | Fetch a node by ID |
| `graph.delete_node(id)` | Delete a node and all its edges |
| `graph.add_edge(src, dst, relation, weight)` | Insert or update a directed edge |
| `graph.delete_edge(src, dst, relation)` | Delete a specific edge |
| `graph.neighbors(id, opts)` | Traverse the graph from a node |
| `graph.nodes_by_kind(kind)` | List all nodes of a given type |
| `graph.stats()` | Return total node count and edge count |

### `SearchOptions`

```rust
SearchOptions {
    top_k:  usize,               // number of results to return
    metric: DistanceMetric,      // Cosine | Euclidean | DotProduct
    filter: Option<Value>,       // JSON metadata filter (exact match)
}
```

### `TraversalOptions`

```rust
TraversalOptions {
    relation:   Option<String>,  // filter by edge relation label (None = all)
    max_depth:  usize,           // maximum hops from the anchor node
    min_weight: Option<f64>,     // exclude edges below this weight
}
```

### `DistanceMetric`

| Variant | Description |
|---|---|
| `DistanceMetric::Cosine` | Cosine distance (default, good for text embeddings) |
| `DistanceMetric::Euclidean` | Euclidean (L2) distance |
| `DistanceMetric::DotProduct` | Dot product distance (good for normalized vectors) |

---

## Internal Schema

AgentDB manages six internal tables, all prefixed with `_adb_`. These are created automatically on first open and should not be modified directly.

```sql
-- Database metadata and schema version
CREATE TABLE _adb_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Vector collection registry
CREATE TABLE _adb_collections (
    id         TEXT PRIMARY KEY,
    name       TEXT UNIQUE NOT NULL,
    dim        INTEGER NOT NULL,
    metric     TEXT NOT NULL DEFAULT 'cosine',
    count      INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Raw vector storage (f32 arrays as little-endian BLOBs)
CREATE TABLE _adb_vectors (
    id            TEXT NOT NULL,
    collection_id TEXT NOT NULL,
    vector        BLOB NOT NULL,
    metadata      TEXT,           -- JSON
    created_at    INTEGER NOT NULL,
    PRIMARY KEY (id, collection_id)
);

-- Serialized HNSW index per collection
CREATE TABLE _adb_hnsw_index (
    collection_id TEXT PRIMARY KEY,
    index_blob    BLOB NOT NULL,   -- bincode-serialized HnswIndex
    built_at      INTEGER NOT NULL,
    is_dirty      INTEGER NOT NULL DEFAULT 0
);

-- Memory graph nodes
CREATE TABLE _adb_nodes (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    data       TEXT,               -- JSON
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Memory graph edges (directed, weighted, labeled)
CREATE TABLE _adb_edges (
    src        TEXT NOT NULL,
    dst        TEXT NOT NULL,
    relation   TEXT NOT NULL,
    weight     REAL NOT NULL DEFAULT 1.0,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (src, dst, relation)
);
```

---

## Comparison

AgentDB is designed to replace the combination of multiple tools that AI agent stacks currently require. The table below shows how it stacks up against the most common alternatives, including SQLite as the closest embedded relational baseline.

| Feature | AgentDB | SQLite | ChromaDB | Weaviate | Qdrant | Neo4j |
|---|---|---|---|---|---|---|
| Embedded (no server) | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Single file | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Zero configuration | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Offline-first | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Relational SQL | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Vector / ANN search | ✅ | ❌ | ✅ | ✅ | ✅ | ❌ |
| Memory graph layer | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ |
| Rust native | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| Public domain license | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Works on edge / mobile | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Built for AI agents | ✅ | ❌ | ⚠️ | ⚠️ | ⚠️ | ❌ |

> ⚠️ = Partial support or requires significant additional tooling

**Key takeaway:** SQLite is the closest thing to AgentDB in terms of embedding model and file simplicity, but it has no vector search and no graph layer. AgentDB extends that embedded, zero-config philosophy with the two capabilities AI agents specifically need.

---

## Project Structure

```
agentdb/
├── Cargo.toml                  # Dependencies and crate configuration
├── README.md                   # This file
├── LICENSE                     # Public domain dedication
├── NOTICE                      # Copyright waiver + dependency audit
├── ARCHITECTURE.md             # Deep-dive on internal design decisions
├── ROADMAP.md                  # Versioned feature roadmap
├── .gitignore
│
├── assets/
│   └── logo.svg                # Project logo
│
├── src/
│   ├── lib.rs                  # Crate root — public API re-exports
│   ├── db.rs                   # AgentDB — main connection struct
│   ├── schema.rs               # Internal table bootstrap + versioning
│   ├── error.rs                # AgentDbError enum (thiserror)
│   ├── vectors/
│   │   ├── mod.rs              # VectorStore public API
│   │   ├── collection.rs       # Collection — upsert, search, delete, reindex
│   │   └── hnsw.rs             # Pure Rust HNSW index + distance metrics
│   └── memory/
│       ├── mod.rs              # MemoryGraph public API
│       └── graph.rs            # Nodes, edges, recursive traversal
│
├── examples/
│   ├── agent_memory.rs         # Full demo — all three layers
│   ├── rag_pipeline.rs         # Local RAG with vector search
│   └── graph_traverse.rs       # Memory graph traversal walkthrough
│
├── tests/
│   ├── test_relational.rs      # SQL layer tests
│   ├── test_vectors.rs         # Vector upsert, search, reindex tests
│   └── test_memory_graph.rs    # Graph node, edge, traversal tests
│
└── benches/
    ├── vector_search.rs        # Criterion: ANN search on 100k vectors
    └── graph_traverse.rs       # Criterion: traversal on 10k node graph
```

---

## Roadmap

Full detail on each item is tracked in [GitHub Issues](https://github.com/hvrcharon1/agentdb/issues).

### v0.1.0 — Core ✅ (current)
- [x] Internal schema bootstrap with versioning
- [x] WAL mode, foreign keys, ACID writes
- [x] Full relational SQL layer
- [x] Pure Rust HNSW vector index (cosine, euclidean, dot product)
- [x] Vector collection API — upsert, search, delete, reindex
- [x] Memory graph — typed nodes, weighted edges, recursive traversal
- [x] `agent_memory` example demonstrating all three layers

### v0.2.0 — Query Power
- [ ] Advanced metadata filtering on vector search (`$gt`, `$lt`, `$in`)
- [ ] Hybrid query — graph traversal + vector search with alpha blending
- [ ] Full-text search integration (FTS5 virtual tables)
- [ ] Batch upsert API for bulk vector ingestion

### v0.3.0 — Developer Experience
- [ ] C FFI flat API + auto-generated `agentdb.h` header via cbindgen
- [ ] Criterion benchmarks — 100k vector search, 10k node traversal
- [ ] CLI: `agentdb inspect`, `agentdb stats`, `agentdb export`, `agentdb shell`
- [ ] Schema migration runner for future schema upgrades

### v0.4.0 — Language Bindings
- [ ] Node.js bindings via napi-rs (TypeScript types included)
- [ ] Python bindings via PyO3 + maturin (numpy array support)
- [ ] WASM build for browser and edge runtimes

### v0.5.0 — Sync
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution strategies: last-write-wins, custom
- [ ] CLI sync commands: push, pull, watch

### v1.0.0 — Production
- [ ] Published to crates.io
- [ ] 80%+ test coverage
- [ ] ANN search < 50ms on 100k vectors
- [ ] Graph traversal < 10ms on 10k nodes
- [ ] Zero known data corruption scenarios
- [ ] Full docs.rs documentation

---

## Contributing

AgentDB is in active early development. Contributions are welcome.

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feat/your-feature`
3. **Write tests** for your changes
4. **Run** `cargo test` and `cargo clippy` — both must pass
5. **Open** a pull request with a clear description

Please check [open issues](https://github.com/hvrcharon1/agentdb/issues) before starting work on a new feature — it may already be in progress.

**Code standards:**
- No `unwrap()` in library code — propagate errors with `?`
- All public API items must have doc comments
- New features require at least one integration test
- Run `cargo fmt` before committing

---

## License

AgentDB is released into the **public domain** by Datacules LLC.

No attribution required. No license file inclusion required. No royalties. No notification. Use it in any project — open source, closed source, commercial, or personal.

For jurisdictions where public domain is not legally recognized, a permissive fallback license granting identical rights is provided.

For enterprises requiring written warranty, indemnification, or SLA guarantees, optional commercial licenses are available. Contact [legal@datacules.com](mailto:legal@datacules.com).

See [LICENSE](LICENSE) and [NOTICE](NOTICE) for full terms.

---

<p align="center">
  Built and maintained by <a href="https://datacules.com"><strong>Datacules LLC</strong></a>
</p>
