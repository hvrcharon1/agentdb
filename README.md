<p align="center">
  <img src="./assets/logo.svg" alt="AgentDB" width="420"/>
</p>

<p align="center">
  <b>The embedded database built for AI agents.</b><br/>
  One file. Five layers. Zero servers.<br/>
  Relational SQL &nbsp;·&nbsp; Vector Search &nbsp;·&nbsp; Full-Text Search &nbsp;·&nbsp; Hybrid Queries &nbsp;·&nbsp; Memory Graphs — all in Rust.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/license-Public%20Domain-brightgreen.svg" alt="License"/>
  &nbsp;
  <img src="https://img.shields.io/badge/language-Rust%202021-orange.svg" alt="Rust"/>
  &nbsp;
  <img src="https://img.shields.io/badge/version-v0.2.0-blue.svg" alt="v0.2.0"/>
  &nbsp;
  <img src="https://img.shields.io/badge/by-Datacules%20LLC-lightgrey.svg" alt="Datacules LLC"/>
</p>

---

## Table of Contents

- [Overview](#overview)
- [Why AgentDB?](#why-agentdb)
- [Architecture](#architecture)
- [The Five Layers](#the-five-layers)
  - [Layer 1 — Relational SQL](#layer-1--relational-sql)
  - [Layer 2 — Vector Store](#layer-2--vector-store)
  - [Layer 3 — Memory Graph](#layer-3--memory-graph)
  - [Layer 4 — Full-Text Search](#layer-4--full-text-search)
  - [Layer 5 — Hybrid Queries](#layer-5--hybrid-queries)
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

AgentDB is a single-file, embedded database engine written in Rust, purpose-built for AI agents and LLM-powered applications. It unifies five storage and query primitives into one self-contained `.agentdb` file:

- Structured relational SQL
- Semantic vector search (HNSW, pure Rust)
- Episodic memory graphs (typed nodes, weighted edges, recursive traversal)
- Full-text search (FTS5, BM25 ranking, Porter stemming)
- Hybrid queries (graph + vector blended ranking)

There is no server to run. No daemon to manage. No network to configure.

```rust
let db = AgentDB::open("agent.agentdb")?;
```

That single line gives your agent a full relational database, a vector index, a traversable memory graph, a full-text search engine, and hybrid query capability — all persisted to one file on disk.

---

## Why AgentDB?

Modern AI agents have needs that today require multiple separate tools:

| What the agent needs | Today's solution | The problem |
|---|---|---|
| Store structured events, sessions, logs | Relational database | No vector search, no graph |
| Semantic similarity search over memories | ChromaDB, Qdrant, Pinecone | Separate service, no SQL, network required |
| Traverse knowledge and memory relationships | Neo4j, custom graph DB | Heavy, not embeddable, not offline |
| Keyword search over stored text | Elasticsearch, Typesense | Yet another service, heavy infra |
| Combined graph + semantic retrieval | Custom code | Fragile, no standard, high latency |

**AgentDB collapses all five into one embedded file.** No services. No ports. No sync headaches.

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

All five layers share the same underlying storage engine and co-exist within one `.agentdb` file.

```
┌──────────────────────────────────────────────────────────┐
│                    agent.agentdb                         │
│                                                          │
│  ┌───────────────────────┐  ┌───────────────────────┐   │
│  │  Layer 1: Relational   │  │  Layer 2: Vector Store  │   │
│  │  Full SQL, indexes,    │  │  HNSW (pure Rust),      │   │
│  │  transactions, JSON    │  │  cosine/euclidean/dot,  │   │
│  │  user-defined tables   │  │  batch upsert, filter   │   │
│  └───────────────────────┘  └───────────────────────┘   │
│                                                          │
│  ┌───────────────────────┐  ┌───────────────────────┐   │
│  │  Layer 3: Memory Graph │  │  Layer 4: Full-Text     │   │
│  │  Typed nodes, weighted │  │  FTS5 virtual tables,   │   │
│  │  edges, recursive CTE  │  │  BM25, Porter stemmer,  │   │
│  │  depth + weight filter │  │  snippets, optimize      │   │
│  └───────────────────────┘  └───────────────────────┘   │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │           Layer 5: Hybrid Queries               │   │
│  │  Graph traversal + vector ANN + alpha blending   │   │
│  │  Graph-connected nodes get a weighted rank boost  │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  _adb_meta · _adb_collections · _adb_vectors             │
│  _adb_hnsw_index · _adb_nodes · _adb_edges               │
│  _adb_fts_{collection} (FTS5 virtual tables)              │
│                                                          │
│  WAL mode · ACID · Foreign keys · Concurrent reads       │
└──────────────────────────────────────────────────────────┘
```

---

## The Five Layers

### Layer 1 — Relational SQL

Full SQL engine. Create any tables alongside AgentDB’s internal tables. Run joins, aggregations, CTEs. ACID-compliant and durable.

**Use it for:** conversation history, session metadata, user profiles, event logs, structured agent state.

```rust
db.execute("
    CREATE TABLE IF NOT EXISTS events (
        id       TEXT PRIMARY KEY,
        kind     TEXT NOT NULL,
        payload  TEXT,
        agent_id TEXT NOT NULL,
        ts       INTEGER NOT NULL
    )
")?;

db.execute_params(
    "INSERT INTO events VALUES (?1, ?2, ?3, ?4, ?5)",
    &[&"evt_001", &"user_message", &r#"{"text":"Hello"}"#, &"agent_42", &1700000000_i64],
)?;

let rows = db.query_json(
    "SELECT * FROM events WHERE agent_id = 'agent_42' ORDER BY ts DESC LIMIT 20"
)?;
```

**Capabilities:** full SQL, transactions, indexes, JSON payloads, user tables co-exist with `_adb_*` internal tables.

---

### Layer 2 — Vector Store

HNSW-based approximate nearest-neighbor search. Pure Rust, no C libraries. Advanced metadata filtering with MongoDB-style operators.

**Use it for:** semantic memory search, RAG pipelines, similarity retrieval, embedding deduplication.

```rust
let col = db.vectors().collection("memories", 1536)?;

// Single upsert
col.upsert(VectorEntry {
    id: "mem_001".into(),
    vector: my_embedding,
    metadata: Some(json!({ "role": "user", "score": 9, "ts": 1700000000 })),
})?;

// Batch upsert (single transaction)
col.upsert_batch(vec![
    BatchEntry { id: "m1".into(), vector: embed_1, metadata: Some(json!({ "score": 8 })) },
    BatchEntry { id: "m2".into(), vector: embed_2, metadata: Some(json!({ "score": 6 })) },
])?;

// Advanced metadata filter
let results = col.search(
    &query_vec,
    SearchOptions {
        top_k: 5,
        metric: DistanceMetric::Cosine,
        filter: Some(json!({ "role": "user", "score": { "$gte": 8 } })),
    },
)?;
```

**Filter operators:** `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists`

**HNSW:** M=16, ef\_construction=200, cosine / euclidean / dot product, lazy index build, serialized to disk.

---

### Layer 3 — Memory Graph

Typed nodes and weighted directed edges with recursive CTE traversal. No in-memory graph library required.

**Use it for:** agent knowledge graphs, concept maps, session-to-concept linking, multi-hop reasoning chains.

```rust
let graph = db.memory();

graph.add_node("session_42",   "session", Some(json!({ "user": "harshal" })))?;
graph.add_node("concept_rust", "concept", Some(json!({ "label": "Rust" })))?;
graph.add_edge("session_42", "concept_rust", "discussed", 0.95)?;

let neighbors = graph.neighbors("session_42", TraversalOptions {
    relation:   Some("discussed".into()),
    max_depth:  2,
    min_weight: Some(0.6),
})?;
```

**Capabilities:** typed nodes, labeled directed edges, weighted traversal, depth limit, relation filter, weight filter, cascade deletes.

---

### Layer 4 — Full-Text Search

FTS5 virtual tables with BM25 ranking and Porter stemmer. Per-collection FTS indexes, snippet extraction, index optimization.

**Use it for:** keyword search over stored documents, hybrid keyword + semantic retrieval, search-as-you-type on agent memory.

```rust
let fts = db.fts();

// Index document text
fts.index_text("memories", "mem_001", &col.id, "Rust is a systems language focused on safety")?;
fts.optimize("memories")?;

// Keyword search with BM25 ranking
let results = fts.search("memories", "systems safety", 5)?;
for r in &results {
    println!("{} | snippet: {} | rank: {:.4}", r.id, r.snippet, r.rank);
}

// Delete from index
fts.delete_text("memories", "mem_001")?;
```

**Capabilities:** FTS5 virtual tables per collection, BM25 ranking, Porter stemmer, highlighted snippets, `optimize()` to merge segments.

---

### Layer 5 — Hybrid Queries

Combines memory graph traversal with vector ANN search into a single blended ranking. Graph-connected nodes get a weighted boost proportional to their edge weight. Alpha controls the blend.

**Use it for:** personalized retrieval (graph priors + semantic similarity), session-aware RAG, multi-hop context retrieval.

```rust
let results = db.hybrid_query(HybridQuery {
    anchor_node: "session_42",   // start graph traversal here
    embedding:   &query_vec,     // ANN search with this vector
    collection:  "memories",     // which vector collection to search
    graph_depth: 2,              // traverse up to 2 hops
    top_k:       10,             // return top 10 results
    alpha:       0.6,            // 0.0 = pure graph, 1.0 = pure vector
    filter:      None,
})?;

for r in &results {
    println!("{} | rank={:.4}  vec={:.4}  graph={:.2}",
        r.id, r.rank_score, r.vector_score, r.graph_weight);
}
```

**Ranking formula:** `rank = alpha × vector_similarity + (1 - alpha) × graph_weight`

---

## Quick Start

### 1. Add to your project

```toml
[dependencies]
agentdb = { git = "https://github.com/hvrcharon1/agentdb" }
```

### 2. Run the examples

```bash
git clone https://github.com/hvrcharon1/agentdb
cd agentdb
cargo run --example agent_memory       # all three base layers
cargo run --example rag_pipeline       # local RAG with vector search
cargo run --example graph_traverse     # memory graph traversal
cargo run --example v020_query_power   # batch, filters, FTS, hybrid
```

### 3. Full working example

```rust
use agentdb::{
    AgentDB, BatchEntry, DistanceMetric, HybridQuery,
    SearchOptions, TraversalOptions, VectorEntry,
};
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open("agent.agentdb")?;

    // Layer 1: SQL
    db.execute("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, user TEXT)")?;

    // Layer 2: Vectors + batch upsert + advanced filter
    let col = db.vectors().collection("thoughts", 4)?;
    col.upsert_batch(vec![
        BatchEntry { id: "t1".into(), vector: vec![0.9,0.1,0.0,0.0], metadata: Some(json!({"score":9})) },
        BatchEntry { id: "t2".into(), vector: vec![0.1,0.9,0.0,0.0], metadata: Some(json!({"score":5})) },
    ])?;
    let results = col.search(&[0.9,0.1,0.0,0.0], SearchOptions {
        top_k: 1,
        metric: DistanceMetric::Cosine,
        filter: Some(json!({ "score": { "$gte": 8 } })),
    })?;

    // Layer 3: Memory graph
    let graph = db.memory();
    graph.add_node("s1", "session", None)?;
    graph.add_node("t1", "thought", None)?;
    graph.add_edge("s1", "t1", "recalled", 0.9)?;

    // Layer 4: Full-text search
    let fts = db.fts();
    fts.index_text("thoughts", "t1", &col.id, "Rust systems programming")?;
    let kw = fts.search("thoughts", "systems", 5)?;

    // Layer 5: Hybrid query
    let hybrid = db.hybrid_query(HybridQuery {
        anchor_node: "s1",
        embedding:   &[0.9,0.1,0.0,0.0],
        collection:  "thoughts",
        graph_depth: 1,
        top_k:       5,
        alpha:       0.6,
        filter:      None,
    })?;

    let stats = db.stats()?;
    println!("collections={} vectors={} nodes={} edges={}",
        stats.collections, stats.vectors, stats.nodes, stats.edges);
    Ok(())
}
```

---

## API Reference

### `AgentDB`

| Method | Description |
|---|---|
| `AgentDB::open(path)` | Open or create a database file |
| `AgentDB::open(":memory:")` | Open an in-memory database (tests) |
| `db.execute(sql)` | Execute a SQL statement |
| `db.execute_params(sql, params)` | Execute a parameterized SQL statement |
| `db.query_json(sql)` | Query → `Vec<serde_json::Value>` |
| `db.vectors()` | Access vector store → `VectorStore` |
| `db.memory()` | Access memory graph → `MemoryGraph` |
| `db.fts()` | Access full-text search → `FullTextStore` |
| `db.hybrid_query(q)` | Run a hybrid graph + vector query |
| `db.stats()` | Return `DbStats` |
| `db.close()` | Flush dirty indexes and close |

### `VectorStore`

| Method | Description |
|---|---|
| `db.vectors().collection(name, dim)` | Get or create collection (cosine) |
| `db.vectors().collection_with_metric(name, dim, metric)` | Get or create with explicit metric |
| `db.vectors().list_collections()` | List all collections |
| `db.vectors().drop_collection(name)` | Drop a collection and its vectors |

### `Collection`

| Method | Description |
|---|---|
| `col.upsert(entry)` | Insert or update a single vector |
| `col.upsert_batch(entries)` | Bulk insert in a single transaction |
| `col.search(query, opts)` | ANN search → `Vec<SearchResult>` |
| `col.delete(id)` | Delete a vector by ID |
| `col.reindex()` | Force rebuild the HNSW index |
| `col.count()` | Number of vectors in collection |

### `MemoryGraph`

| Method | Description |
|---|---|
| `graph.add_node(id, kind, data)` | Insert or update a node |
| `graph.get_node(id)` | Fetch a node by ID |
| `graph.delete_node(id)` | Delete node and cascade its edges |
| `graph.add_edge(src, dst, relation, weight)` | Insert or update a directed edge |
| `graph.delete_edge(src, dst, relation)` | Delete a specific edge |
| `graph.neighbors(id, opts)` | Recursive graph traversal |
| `graph.nodes_by_kind(kind)` | List all nodes of a given type |
| `graph.stats()` | Node count and edge count |

### `FullTextStore`

| Method | Description |
|---|---|
| `fts.index_text(collection, vec_id, col_id, text)` | Index text for a vector entry |
| `fts.search(collection, query, top_k)` | BM25 full-text search |
| `fts.delete_text(collection, vec_id)` | Remove a document from the index |
| `fts.optimize(collection)` | Merge FTS index segments |

### `HybridQuery`

```rust
HybridQuery {
    anchor_node: &str,       // graph traversal start node
    embedding:   &[f32],     // ANN search query vector
    collection:  &str,       // vector collection name
    graph_depth: usize,      // max hops from anchor
    top_k:       usize,      // results to return
    alpha:       f64,        // 0.0=graph only, 1.0=vector only
    filter:      Option<Value>, // metadata filter on vector results
}
```

### `SearchOptions`

```rust
SearchOptions {
    top_k:  usize,
    metric: DistanceMetric,   // Cosine | Euclidean | DotProduct
    filter: Option<Value>,    // supports $eq $ne $gt $gte $lt $lte $in $nin $exists
}
```

### `TraversalOptions`

```rust
TraversalOptions {
    relation:   Option<String>,  // filter by relation label (None = all)
    max_depth:  usize,           // max hops
    min_weight: Option<f64>,     // exclude edges below this weight
}
```

---

## Internal Schema

```sql
CREATE TABLE _adb_meta        (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE _adb_collections (id TEXT PRIMARY KEY, name TEXT UNIQUE, dim INTEGER,
                                metric TEXT, count INTEGER, created_at INTEGER);
CREATE TABLE _adb_vectors     (id TEXT, collection_id TEXT, vector BLOB,
                                metadata TEXT, created_at INTEGER,
                                PRIMARY KEY (id, collection_id));
CREATE TABLE _adb_hnsw_index  (collection_id TEXT PRIMARY KEY, index_blob BLOB,
                                built_at INTEGER, is_dirty INTEGER);
CREATE TABLE _adb_nodes       (id TEXT PRIMARY KEY, kind TEXT, data TEXT,
                                created_at INTEGER, updated_at INTEGER);
CREATE TABLE _adb_edges       (src TEXT, dst TEXT, relation TEXT, weight REAL,
                                created_at INTEGER, PRIMARY KEY (src, dst, relation));
-- FTS5 virtual tables (one per collection, created on first index_text call):
CREATE VIRTUAL TABLE _adb_fts_{name} USING fts5(vec_id, collection_id UNINDEXED,
                                                 text, content='',
                                                 tokenize='porter ascii');
```

---

## Comparison

AgentDB replaces the combination of multiple tools that AI agent stacks currently require. SQLite is included as the closest embedded relational baseline.

| Feature | AgentDB | SQLite | ChromaDB | Weaviate | Qdrant | Neo4j |
|---|---|---|---|---|---|---|
| Embedded (no server) | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Single file | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Zero configuration | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Offline-first | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Relational SQL | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Vector / ANN search | ✅ | ❌ | ✅ | ✅ | ✅ | ❌ |
| Advanced metadata filter | ✅ | ❌ | ⚠️ | ✅ | ✅ | ❌ |
| Batch vector upsert | ✅ | ❌ | ✅ | ✅ | ✅ | ❌ |
| Full-text search (BM25) | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ |
| Memory graph layer | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ |
| Hybrid graph + vector query | ✅ | ❌ | ❌ | ⚠️ | ❌ | ❌ |
| Rust native | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| Public domain license | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| Works on edge / mobile | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

> ⚠️ = Partial support or requires additional tooling

**Key takeaway:** SQLite is the closest thing to AgentDB in terms of embedding model and file simplicity, but has no vector search, no graph layer, no FTS with BM25, and no hybrid queries. AgentDB extends that zero-config philosophy with everything AI agents specifically need.

---

## Project Structure

```
agentdb/
├── Cargo.toml
├── README.md
├── LICENSE                     # Public domain dedication
├── NOTICE                      # Copyright waiver + dependency audit
├── ARCHITECTURE.md
├── ROADMAP.md
├── .gitignore
├── .github/
│   └── workflows/
│       ├── ci.yml              # lint, test (ubuntu/macos/windows), audit, coverage
│       ├── bench.yml           # Criterion benchmarks on push to main
│       └── release.yml         # build + publish on git tag
├── assets/
│   └── logo.svg
├── src/
│   ├── lib.rs              # public API re-exports
│   ├── db.rs               # AgentDB — main connection struct
│   ├── schema.rs           # internal table bootstrap + versioning
│   ├── error.rs            # AgentDbError (thiserror)
│   ├── filter.rs           # metadata filter engine ($gt, $in, $exists ...)
│   ├── fts.rs              # full-text search (FTS5, BM25, Porter stemmer)
│   ├── hybrid.rs           # hybrid graph + vector query engine
│   ├── vectors/
│   │   ├── mod.rs
│   │   ├── collection.rs   # upsert, batch upsert, search, delete, reindex
│   │   └── hnsw.rs         # pure Rust HNSW + cosine/euclidean/dot
│   └── memory/
│       ├── mod.rs
│       └── graph.rs        # nodes, edges, recursive CTE traversal
├── examples/
│   ├── agent_memory.rs     # all three base layers demo
│   ├── rag_pipeline.rs     # local RAG pipeline
│   ├── graph_traverse.rs   # memory graph traversal
│   └── v020_query_power.rs # batch, advanced filters, FTS, hybrid
├── tests/
│   ├── test_relational.rs  # SQL layer (6 tests)
│   ├── test_vectors.rs     # vector upsert, search, filter (12 tests)
│   ├── test_memory_graph.rs # graph nodes, edges, traversal (11 tests)
│   └── test_v020.rs        # filter engine, batch, hybrid (20 tests)
└── benches/
    ├── vector_search.rs    # upsert/search/reindex at 1k–10k–100k
    └── graph_traverse.rs   # traversal at depth 1–4 on 1k-node graph
```

---

## Roadmap

Full detail tracked in [GitHub Issues](https://github.com/hvrcharon1/agentdb/issues).

### v0.1.0 — Core ✅
- [x] Schema bootstrap, WAL mode, ACID writes
- [x] Relational SQL layer
- [x] Pure Rust HNSW (cosine, euclidean, dot product)
- [x] Vector collection API — upsert, search, delete, reindex
- [x] Memory graph — typed nodes, weighted edges, recursive traversal
- [x] Examples: agent\_memory, rag\_pipeline, graph\_traverse
- [x] Test suite: 29 tests across 3 files
- [x] Criterion benchmarks (vector + graph)
- [x] GitHub Actions CI — lint, test, audit, coverage, release

### v0.2.0 — Query Power ✅
- [x] Advanced metadata filtering — `$gt`, `$gte`, `$lt`, `$lte`, `$eq`, `$ne`, `$in`, `$nin`, `$exists`
- [x] Hybrid query — graph traversal + ANN search with alpha blending
- [x] Full-text search — FTS5 virtual tables, BM25 ranking, Porter stemmer, snippets
- [x] Batch upsert — single-transaction bulk insert with full rollback on error
- [x] Example: v020\_query\_power
- [x] Test suite: 20 new tests (filter engine, batch, hybrid)
- [x] CI fixes — resolved self-referencing imports, missing VectorStore, fmt issues

### v0.3.0 — Developer Experience
- [ ] C FFI flat API + auto-generated `agentdb.h` via cbindgen
- [ ] CLI: `agentdb inspect`, `agentdb stats`, `agentdb export`, `agentdb shell`, `agentdb reindex`
- [ ] Schema migration runner
- [ ] `BENCHMARKS.md` with baseline numbers

### v0.4.0 — Language Bindings
- [ ] Node.js bindings via napi-rs (TypeScript types)
- [ ] Python bindings via PyO3 + maturin (numpy support)
- [ ] WASM build for browser and Cloudflare Workers

### v0.5.0 — Sync
- [ ] AgentDB Sync — CRDT-based replication protocol
- [ ] Conflict resolution: last-write-wins + custom
- [ ] CLI sync: push, pull, watch

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

**Code standards:**
- No `unwrap()` in library code — propagate errors with `?`
- All public API items must have doc comments
- New features require at least one integration test
- Run `cargo fmt` before committing

---

## License

AgentDB is released into the **public domain** by Datacules LLC.

No attribution required. No license file required. No royalties. Use it in any project — open source, closed source, commercial, or personal.

For jurisdictions where public domain is not legally recognized, a permissive fallback license granting identical rights is provided. For enterprises requiring written warranty or indemnification, optional commercial licenses are available at [legal@datacules.com](mailto:legal@datacules.com).

See [LICENSE](LICENSE) and [NOTICE](NOTICE) for full terms.

---

<p align="center">
  Built and maintained by <a href="https://datacules.com"><strong>Datacules LLC</strong></a>
</p>
