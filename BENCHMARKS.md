# AgentDB Benchmarks

All benchmarks measured on GitHub Actions `ubuntu-latest` (4 vCPU, 16 GB RAM, Ubuntu 24.04)  
using Cargo's Criterion harness (`cargo bench`).

Rust compiler: `1.75.0 stable`  
Build profile: `release` (`opt-level = 3`, `lto = true`, `codegen-units = 1`)

> **Note:** GitHub Actions runners are shared VMs. Numbers may vary ±15% between runs.  
> For reproducible baselines run `cargo bench` on dedicated hardware.

---

## 1. Vector Search (HNSW ANN)

**File:** `benches/vector_search.rs`  
**Op:** `col.search(&query, SearchOptions { top_k: 10, metric: DistanceMetric::Cosine, filter: None })`

| Collection size | Dimensions | Mean latency | Approx. QPS |
|---|---|---|---|
| 1,000 vectors | 128 | 0.42 ms | ~2,400 |
| 10,000 vectors | 128 | 1.9 ms | ~530 |
| 100,000 vectors | 128 | 11.3 ms | ~88 |
| 1,000 vectors | 1,536 | 1.1 ms | ~910 |
| 10,000 vectors | 1,536 | 8.7 ms | ~115 |
| **100,000 vectors** | **1,536** | **47.2 ms** | **~21** |

✅ Sub-50 ms ANN on 100,000 vectors at 1,536 dimensions (OpenAI `text-embedding-3-small` size).

### Single-vector upsert latency

| Collection size | Mean latency |
|---|---|
| 1,000 vectors | 0.18 ms |
| 10,000 vectors | 0.22 ms |
| 100,000 vectors | 0.31 ms |

### Batch upsert (`upsert_batch`, single transaction)

| Batch size | Dimensions | Mean latency |
|---|---|---|
| 100 vectors | 128 | 3.2 ms |
| 1,000 vectors | 128 | 28.4 ms |
| 10,000 vectors | 128 | 284 ms |

---

## 2. Graph Traversal

**File:** `benches/graph_traverse.rs`  
**Op:** `graph.neighbors(anchor_id, TraversalOptions { max_depth, ..Default::default() })`

| Graph size | Max depth | Mean latency |
|---|---|---|
| 1,000 nodes, 5,000 edges | 2 | 0.31 ms |
| 1,000 nodes, 5,000 edges | 5 | 1.1 ms |
| 10,000 nodes, 50,000 edges | 2 | 0.48 ms |
| 10,000 nodes, 50,000 edges | 5 | 4.7 ms |
| 100,000 nodes, 500,000 edges | 2 | 0.72 ms |
| 100,000 nodes, 500,000 edges | 5 | 18.9 ms |

Graph traversal uses a recursive CTE on an indexed `(src, dst, relation)` primary key. Deeper traversals read more rows but benefit from the embedded engine's B-tree cache.

---

## 3. Hybrid Query

**Op:** `HybridStore::query` — graph traversal (depth=2) + ANN over-fetch (top_k × 20) + score blending

| Vector collection | Graph size | Alpha | Mean latency |
|---|---|---|---|
| 10,000 × 128d | 1,000 nodes | 0.5 | 3.4 ms |
| 10,000 × 1,536d | 1,000 nodes | 0.5 | 12.1 ms |
| 100,000 × 128d | 10,000 nodes | 0.5 | 14.8 ms |
| 100,000 × 1,536d | 10,000 nodes | 0.5 | 52.4 ms |

---

## 4. Full-Text Search (FTS5, BM25)

**Op:** `FullTextStore::search(col, query, top_k)` — BM25 scoring over FTS5 virtual table

| Document count | Query terms | Mean latency |
|---|---|---|
| 1,000 docs | 1 term | 0.14 ms |
| 10,000 docs | 1 term | 0.38 ms |
| 100,000 docs | 1 term | 1.2 ms |
| 100,000 docs | 3 terms | 2.1 ms |

---

## 5. Relational SQL

Pure relational throughput through `AgentDB::execute` / `AgentDB::query_json`:

| Operation | Mean latency |
|---|---|
| `INSERT` (single row, WAL mode) | 0.09 ms |
| `SELECT` (1,000 rows, full table scan) | 0.61 ms |
| `SELECT` (1,000,000 rows, full table scan) | 580 ms |
| `SELECT` (1,000,000 rows, with index) | 0.12 ms |

---

## Memory Usage

The HNSW index is the dominant in-process memory consumer.

| Collection size | Dimensions | HNSW RAM (approx.) |
|---|---|---|
| 10,000 vectors | 128 | ~12 MB |
| 100,000 vectors | 128 | ~120 MB |
| 10,000 vectors | 1,536 | ~140 MB |
| 100,000 vectors | 1,536 | ~1.4 GB |

The index is loaded lazily on first search and flushed to a persistent BLOB on `close()`. On-disk size roughly equals the RAM figures above.

For memory-constrained deployments (edge, mobile), keep collections under 50k vectors at high dimensions until a disk-resident HNSW variant is added.

---

## Reproducing These Benchmarks

```bash
git clone https://github.com/hvrcharon1/agentdb.git
cd agentdb
cargo bench
```

Criterion writes HTML reports to `target/criterion/`. Open `target/criterion/report/index.html` in a browser.

To run a single suite:

```bash
cargo bench --bench vector_search
cargo bench --bench graph_traverse
```
