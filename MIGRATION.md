# AgentDB Migration Guide

This document covers API changes between AgentDB versions and how to update your code.

---

## v0.3.1 → v0.3.2

**Package renamed on crates.io and PyPI to avoid name conflicts.** No API changes.

### Rust

```toml
# Before (v0.3.1 — never actually published to crates.io)
[dependencies]
agentdb = "0.3.1"

# After (v0.3.2)
[dependencies]
datacules-agentdb = "0.3.2"
```

The library name is still `agentdb`, so your `use` statements don't change:
```rust
use agentdb::{AgentDB, SearchOptions, DistanceMetric};
```

### Python

```bash
# Before
pip install agentdb

# After
pip install datacules-agentdb
```

The module name is still `agentdb`:
```python
import agentdb
db = agentdb.AgentDB.open(":memory:")
```

### Node.js (unchanged)

```bash
npm install @datacules/agentdb
```

---

## v0.2.0 → v0.3.0

**No breaking changes in the Rust API.** All v0.2.0 Rust code compiles and runs unchanged.

### What's new

Five distribution channels are now available:

```toml
# Rust
datacules-agentdb = "0.3"
```
```bash
pip install datacules-agentdb     # Python (PyPI)
npm install @datacules/agentdb    # Node.js (npm)
```
Plus: `agentdb.h` C header via `ffi-header.yml` CI artifact, and pre-built CLI
binaries on GitHub Releases (Linux, macOS x86_64/arm64, Windows).

### Node.js / TypeScript — options-object API

The v0.3.0 Node.js binding uses an **options object** for `search` and
`hybridQuery`. This is the only stable public API for the npm package:

```ts
import { AgentDB, DistanceMetric } from '@datacules/agentdb';

const db  = AgentDB.open(':memory:');
const col = db.collection('thoughts', 4);

col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { score: 9 });

// search — options object (all fields optional)
const results = col.search([0.9, 0.1, 0.0, 0.0]);
const results = col.search(vec, { topK: 5 });
const results = col.search(vec, { topK: 5, metric: 'euclidean' });
const results = col.search(vec, { topK: 10, filter: { score: { $gt: 7 } } });

// hybridQuery — options object (all fields optional)
const hits = db.hybridQuery('user:1', embedding, 'thoughts');
const hits = db.hybridQuery('user:1', embedding, 'thoughts', {
  topK: 10,
  graphDepth: 3,
  alpha: 0.7,   // 0.0 = pure graph, 1.0 = pure vector
});
```

**DistanceMetric values:** `'cosine'` (default) | `'euclidean'` | `'dot'`

---

## v0.1.0 → v0.2.0

**No breaking changes.** All v0.1.0 code continues to compile and behave identically.

### New APIs added in v0.2.0

**`Collection` — new methods:**

```rust
// Batch upsert (single transaction, full rollback on failure)
let n = col.upsert_batch(vec![
    BatchEntry { id: "a".into(), vector: vec![1.0, 0.0], metadata: None },
    BatchEntry { id: "b".into(), vector: vec![0.0, 1.0], metadata: Some(json!({"tag": "x"})) },
])?;

// Delete a single vector
col.delete("a")?;
```

**`SearchOptions` — extended filter support:**

```rust
// Advanced metadata filtering (MongoDB-style operators)
let results = col.search(&query, SearchOptions {
    top_k: 10,
    metric: DistanceMetric::Cosine,
    filter: Some(json!({
        "score": { "$gt": 7 },
        "tag":   { "$in": ["important", "recent"] },
        "draft": { "$exists": false }
    })),
})?;
```

**Supported filter operators:** `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`,
`$in`, `$nin`, `$exists`.

**`AgentDB` — new methods:**

```rust
// Hybrid graph + vector query
let hits = db.hybrid_query(HybridQuery {
    anchor_node: "user:1",
    embedding:   &embedding,
    collection:  "thoughts",
    graph_depth: 2,
    top_k:       10,
    alpha:       0.6,   // 0.0 = pure graph weight, 1.0 = pure vector score
    filter:      None,
})?;
// HybridResult { id, rank_score, vector_score, graph_weight }

// Full-text search
db.fts().index_text("docs", "doc1", "doc1", "The quick brown fox")?;
db.fts().index_text("docs", "doc2", "doc2", "The lazy dog")?;
let hits = db.fts().search("docs", "quick fox", 10)?;
// FtsResult { id, snippet, rank }

// Parameterized SQL
db.execute_params(
    "INSERT INTO sessions (id, user) VALUES (?1, ?2)",
    rusqlite::params!["s1", "alice"],
)?;

// Explicit close (flushes dirty HNSW indexes)
db.close()?;
```

**`VectorStore` — new method:**

```rust
db.vectors().drop_collection("old_embeddings")?;
```

---

## v0.1.0 API reference (unchanged fields)

The v0.1.0 core API surface remains stable across all versions:

```rust
// Open
let db = AgentDB::open(":memory:").unwrap();
let db = AgentDB::open("/path/to/agent.db").unwrap();

// SQL
db.execute("CREATE TABLE t (id TEXT PRIMARY KEY)").unwrap();
let rows = db.query_json("SELECT * FROM t").unwrap();

// Vectors
let col = db.vectors().collection("name", 4).unwrap();
col.upsert(VectorEntry { id: "v1".into(), vector: vec![1.0,0.0,0.0,0.0], metadata: None }).unwrap();
let results = col.search(&[1.0,0.0,0.0,0.0], SearchOptions::default()).unwrap();
col.count().unwrap();
col.reindex().unwrap();

// Memory graph
let g = db.memory();
g.add_node("n1", "concept", Some(json!({"label": "AI"}))).unwrap();
g.add_node("n2", "concept", None).unwrap();
g.add_edge("n1", "n2", "relates_to", 0.8).unwrap();
let nbrs = g.neighbors("n1", TraversalOptions {
    relation: None, max_depth: 2, min_weight: Some(0.5)
}).unwrap();

// Stats
let s = db.stats().unwrap();
println!("vectors={} nodes={} edges={}", s.vectors, s.nodes, s.edges);
```

---

*See [CHANGELOG.md](./CHANGELOG.md) for the full history of all additions and fixes.*
