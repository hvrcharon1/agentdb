<p align="center">
  <img src="./assets/logo.svg" alt="AgentDB" width="420"/>
</p>

<p align="center">
  <b>The embedded database built for AI agents.</b><br/>
  One file. Five layers. Zero servers.<br/>
  Relational SQL &nbsp;·&nbsp; Vector Search &nbsp;·&nbsp; Full-Text Search &nbsp;·&nbsp; Hybrid Queries &nbsp;·&nbsp; Memory Graphs — in Rust, Python, Node.js, C, WASM, and CLI.
</p>

<p align="center">
  <a href="https://github.com/hvrcharon1/agentdb/actions/workflows/ci.yml">
    <img src="https://github.com/hvrcharon1/agentdb/actions/workflows/ci.yml/badge.svg" alt="CI"/>
  </a>
  &nbsp;
  <a href="https://codecov.io/gh/hvrcharon1/agentdb">
    <img src="https://codecov.io/gh/hvrcharon1/agentdb/branch/main/graph/badge.svg" alt="Coverage"/>
  </a>
  &nbsp;
  <img src="https://img.shields.io/badge/license-Unlicense-brightgreen.svg" alt="License"/>
  &nbsp;
  <img src="https://img.shields.io/badge/language-Rust%202021-orange.svg" alt="Rust"/>
  &nbsp;
  <img src="https://img.shields.io/badge/version-v0.3.0-blue.svg" alt="v0.3.0"/>
  &nbsp;
  <img src="https://img.shields.io/badge/by-Datacules%20LLC-lightgrey.svg" alt="Datacules LLC"/>
</p>

---

## Installation

AgentDB is available in five distribution channels. Pick the one that matches your stack.

### Rust — `cargo add`

```toml
# Cargo.toml
[dependencies]
agentdb = "0.3"
```

```rust
use agentdb::AgentDB;
let db = AgentDB::open("agent.agentdb")?;
```

### Python — `pip install`

```bash
pip install agentdb
```

```python
import agentdb

db = agentdb.AgentDB.open(":memory:")
col = db.collection("thoughts", dim=1536)
col.upsert("m1", embedding, metadata={"score": 9})
results = col.search(query_vec, top_k=5)
```

Verify install: `python -c "import agentdb; print(agentdb.__version__)"`

Wheels available for CPython 3.9+, PyPy, manylinux, macOS (x64 + arm64), Windows.

### Node.js — `npm install`

```bash
npm install agentdb
```

```typescript
import { AgentDB } from 'agentdb';

const db = AgentDB.open(':memory:');
const col = db.collection('thoughts', 1536);
col.upsert('m1', embedding, { score: 9 });
const results = col.search(queryVec, { topK: 5 });
```

Verify install: `node -e "const {AgentDB}=require('agentdb'); console.log('ok')"`

Pre-built native addons for Linux x64/arm64, macOS x64/arm64, Windows x64.
Full TypeScript type definitions included.

### C / Go / Ruby / Swift — shared library + header

```bash
# Build the shared library
cargo build --release --features ffi --lib
# Linux:   target/release/libagentdb.so
# macOS:   target/release/libagentdb.dylib
# Windows: target/release/agentdb.dll

# Generate the C header (requires cbindgen)
cargo install cbindgen
cbindgen --config cbindgen.toml --output agentdb.h
```

```c
#include "agentdb.h"

AgentDbHandle *db = agentdb_open(":memory:");
agentdb_execute(db, "CREATE TABLE t (id TEXT PRIMARY KEY)");
agentdb_close(db);
```

The flat C API covers open/close, SQL execute/query, vector upsert/search,
graph add\_node/add\_edge/neighbors, FTS index/search, hybrid query, and stats.
Any language with C FFI (Go via cgo, Ruby via `ffi` gem, Swift, Kotlin/JNI) can use it.

### CLI — `cargo install` or download binary

```bash
# Install from crates.io
cargo install agentdb

# Or download a pre-built binary from the GitHub Releases page.
```

```bash
agentdb stats      agent.agentdb          # print database statistics
agentdb inspect    agent.agentdb          # full summary: stats + collections + nodes
agentdb sql        agent.agentdb "SELECT * FROM sessions LIMIT 5"
agentdb search     agent.agentdb thoughts 0.9 0.1 0.0 0.0 --top-k 5
agentdb reindex    agent.agentdb          # rebuild all dirty HNSW indexes
agentdb collections agent.agentdb         # list all vector collections
```

### WASM — browser + Cloudflare Workers

```bash
cargo install wasm-pack
wasm-pack build --target web --features wasm -- --no-default-features
```

```js
import init, { WasmAgentDB } from './pkg/agentdb.js';
await init();
const db = WasmAgentDB.open_memory();
db.execute("CREATE TABLE notes (id TEXT)");
console.log(JSON.parse(db.stats()));
// { collections: 0, vectors: 0, nodes: 0, edges: 0 }
```

In-memory databases work today. Persistent storage via OPFS is tracked for v0.4.0.

---

## Table of Contents

- [Overview](#overview)
- [Why AgentDB?](#why-agentdb)
- [Architecture](#architecture)
- [The Five Layers](#the-five-layers)
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

The same database is now accessible from **Rust**, **Python**, **Node.js**, **C** (and any language with C FFI), the **command line**, and the **browser** (WASM).

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

---

## Architecture

All five layers share the same underlying SQLite storage engine and co-exist within one `.agentdb` file.

```
┌──────────────────────────────────────────────────────────┐
│                    agent.agentdb                         │
│  Layer 1: Relational SQL   │  Layer 2: Vector Store       │
│  Layer 3: Memory Graph     │  Layer 4: Full-Text Search   │
│         Layer 5: Hybrid Queries                          │
│  WAL mode · ACID · Foreign keys · Concurrent reads       │
└──────────────────────────────────────────────────────────┘
          │             │             │           │
       Rust API      Python        Node.js      C FFI
       cargo add    pip install   npm install  libagentdb.so
          │             │             │           │
        CLI           WASM        (Go, Ruby,    Browser
      agentdb       wasm-pack     Swift, etc)  Workers
```

---

## The Five Layers

### Layer 1 — Relational SQL

Full SQL engine. Create any tables alongside AgentDB's internal tables.

```rust
db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, user TEXT)")?;
db.execute_params("INSERT INTO sessions VALUES (?1, ?2)", &[&"s1", &"harshal"])?;
let rows = db.query_json("SELECT * FROM sessions")?;
```

### Layer 2 — Vector Store

HNSW-based approximate nearest-neighbor search. Pure Rust. MongoDB-style metadata filtering.

```rust
let col = db.vectors().collection("memories", 1536)?;
col.upsert(VectorEntry { id: "m1".into(), vector: embedding, metadata: Some(json!({ "score": 9 })) })?;
let results = col.search(&query, SearchOptions { top_k: 5, metric: DistanceMetric::Cosine, filter: Some(json!({ "score": { "$gte": 8 } })) })?;
```

### Layer 3 — Memory Graph

Typed nodes, weighted directed edges, recursive CTE traversal.

```rust
let graph = db.memory();
graph.add_node("session_42", "session", None)?;
graph.add_node("concept_rust", "concept", Some(json!({ "label": "Rust" })))?;
graph.add_edge("session_42", "concept_rust", "discussed", 0.95)?;
let neighbors = graph.neighbors("session_42", TraversalOptions { max_depth: 2, ..Default::default() })?;
```

### Layer 4 — Full-Text Search

FTS5, BM25 ranking, Porter stemmer, snippet extraction.

```rust
let fts = db.fts();
fts.index_text("memories", "m1", &col.id, "Rust systems programming safety")?;
let results = fts.search("memories", "systems safety", 5)?;
```

### Layer 5 — Hybrid Queries

Graph traversal + vector ANN blended by alpha.

```rust
let results = db.hybrid_query(HybridQuery {
    anchor_node: "session_42",
    embedding:   &query_vec,
    collection:  "memories",
    graph_depth: 2,
    top_k:       10,
    alpha:       0.6,   // 0.0=pure graph, 1.0=pure vector
    filter:      None,
})?;
```

---

## Quick Start

```bash
git clone https://github.com/hvrcharon1/agentdb
cd agentdb
cargo run --example agent_memory
cargo run --example rag_pipeline
cargo run --example graph_traverse
cargo run --example v020_query_power
```

See also: [`python/examples/agent_memory.py`](python/examples/agent_memory.py) and [`nodejs/examples/agent_memory.ts`](nodejs/examples/agent_memory.ts).

---

## API Reference

### `AgentDB`

| Method | Description |
|---|---|
| `AgentDB::open(path)` | Open or create a database file |
| `db.execute(sql)` | Execute a SQL statement |
| `db.execute_params(sql, params)` | Execute a parameterized SQL statement |
| `db.query_json(sql)` | Query → `Vec<serde_json::Value>` |
| `db.vectors()` | Access vector store |
| `db.memory()` | Access memory graph |
| `db.fts()` | Access full-text search |
| `db.hybrid_query(q)` | Run a hybrid graph + vector query |
| `db.stats()` | Return `DbStats` |
| `db.close()` | Flush dirty indexes and close |

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

### `FullTextStore`

| Method | Description |
|---|---|
| `fts.index_text(col, id, col_id, text)` | Index text for a vector entry |
| `fts.search(col, query, top_k)` | BM25 full-text search |
| `fts.delete_text(col, id)` | Remove a document from the index |
| `fts.optimize(col)` | Merge FTS index segments |

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
CREATE VIRTUAL TABLE _adb_fts_{name} USING fts5(...);
```

---

## Comparison

| Feature | AgentDB | SQLite | ChromaDB | Qdrant | Neo4j |
|---|---|---|---|---|---|
| Embedded (no server) | ✅ | ✅ | ❌ | ❌ | ❌ |
| Single file | ✅ | ✅ | ❌ | ❌ | ❌ |
| Relational SQL | ✅ | ✅ | ❌ | ❌ | ❌ |
| Vector / ANN search | ✅ | ❌ | ✅ | ✅ | ❌ |
| Advanced metadata filter | ✅ | ❌ | ⚠️ | ✅ | ❌ |
| Full-text search (BM25) | ✅ | ❌ | ❌ | ❌ | ❌ |
| Memory graph layer | ✅ | ❌ | ❌ | ❌ | ✅ |
| Hybrid graph + vector query | ✅ | ❌ | ❌ | ❌ | ❌ |
| Python | ✅ | ✅ | ✅ | ✅ | ✅ |
| Node.js | ✅ | ✅ | ✅ | ✅ | ✅ |
| C FFI | ✅ | ✅ | ❌ | ❌ | ❌ |
| CLI | ✅ | ✅ | ❌ | ✅ | ✅ |
| WASM / browser | ✅ | ✅ | ❌ | ❌ | ❌ |
| Works on edge / mobile | ✅ | ✅ | ❌ | ❌ | ❌ |
| Public domain license | ✅ | ✅ | ❌ | ❌ | ❌ |

---

## Project Structure

```
agentdb/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── CHANGELOG.md
├── ROADMAP.md
├── ARCHITECTURE.md
├── BENCHMARKS.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
├── NOTICE
├── cbindgen.toml          ← C header generation config
├── rustfmt.toml
├── .github/
│   ├── workflows/
│   │   ├── ci.yml             ← lint, test, audit, coverage
│   │   ├── bench.yml          ← Criterion benchmarks
│   │   ├── release.yml        ← build binaries + GitHub Release on tag
│   │   ├── publish.yml        ← crates.io publish on tag
│   │   ├── python-publish.yml ← PyPI wheels on tag
│   │   ├── nodejs-publish.yml ← npm publish on tag
│   │   ├── ffi-header.yml     ← auto-generate agentdb.h on ffi.rs change
│   │   └── wasm.yml           ← wasm-pack build + smoke test on push
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   └── feature_request.yml
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── codecov.yml
│   └── dependabot.yml
├── src/
│   ├── lib.rs
│   ├── db.rs
│   ├── schema.rs
│   ├── error.rs
│   ├── filter.rs
│   ├── fts.rs
│   ├── hybrid.rs
│   ├── ffi.rs             ← C FFI flat API (feature = "ffi")
│   ├── wasm.rs            ← WASM bindings (feature = "wasm")
│   ├── bin/
│   │   └── agentdb.rs     ← CLI binary
│   ├── vectors/
│   │   ├── mod.rs
│   │   ├── collection.rs
│   │   └── hnsw.rs
│   └── memory/
│       ├── mod.rs
│       └── graph.rs
├── python/
│   ├── Cargo.toml
│   ├── pyproject.toml
│   ├── src/lib.rs
│   ├── python/__init__.py
│   └── examples/
│       └── agent_memory.py
├── nodejs/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── package.json
│   ├── index.js
│   ├── index.d.ts
│   ├── src/lib.rs
│   ├── test/
│   │   └── smoke.js
│   └── examples/
│       └── agent_memory.ts
├── examples/
│   ├── agent_memory.rs
│   ├── rag_pipeline.rs
│   ├── graph_traverse.rs
│   └── v020_query_power.rs
├── tests/
│   ├── test_relational.rs
│   ├── test_vectors.rs
│   ├── test_memory_graph.rs
│   ├── test_v020.rs
│   ├── test_ffi.rs        ← FFI layer (--features ffi)
│   └── test_cli.rs        ← CLI binary integration tests
└── benches/
    ├── vector_search.rs
    └── graph_traverse.rs
```

---

## Roadmap

| Milestone | Status |
|---|---|
| v0.1.0 Core | ✅ Done |
| v0.2.0 Query Power | ✅ Done |
| v0.3.0 Universal Availability | ✅ Done |
| v0.4.0 WASM Persistence + Go/Ruby bindings | 🔜 Next |
| v0.5.0 LangChain + LlamaIndex + MCP + Sync | Planned |
| v1.0.0 Production + all registries published | Planned |

Full detail in [ROADMAP.md](ROADMAP.md) and [CHANGELOG.md](CHANGELOG.md).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, PR process, and code standards.

Quick summary:
1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feat/your-feature`
3. **Write tests** for your changes
4. **Run** `cargo test` and `cargo clippy` — both must pass
5. **Open** a pull request with a clear description

To report a security vulnerability, see [SECURITY.md](SECURITY.md).

---

## License

AgentDB is released into the **public domain** by Datacules LLC.
See [LICENSE](LICENSE) and [NOTICE](NOTICE) for full terms.

---

<p align="center">
  Built and maintained by <a href="https://datacules.com"><strong>Datacules LLC</strong></a>
</p>
