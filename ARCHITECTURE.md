# AgentDB Architecture

## Overview

AgentDB is built on three storage layers that all live inside a single SQLite file.
SQLite's proven storage engine, WAL mode, and ACID guarantees are the foundation.
AgentDB adds vector indexing and graph traversal as first-class citizens on top.

---

## Layer 1 — Relational (SQL)

- Direct SQLite access via `rusqlite` with WAL mode enabled
- Users can create any tables they need alongside AgentDB's internal `_adb_*` tables
- Full SQL support: joins, CTEs, transactions, indexes

## Layer 2 — Vector Store

- Collections stored in `_adb_collections` (metadata) and `_adb_vectors` (raw data)
- Vectors serialized as little-endian `f32` byte arrays in SQLite BLOBs
- HNSW index built lazily on first `search()` call, serialized via `bincode` into `_adb_hnsw_index`
- `is_dirty` flag triggers rebuild on close or manual `reindex()`
- Supports cosine, euclidean, and dot-product distance metrics

## Layer 3 — Memory Graph

- Nodes: `_adb_nodes` (id, kind, JSON data)
- Edges: `_adb_edges` (src, dst, relation, weight)
- Traversal via SQLite recursive CTEs — no in-memory graph library needed
- Depth-limited, weight-filtered, relation-filtered traversal

---

## Internal Tables

```sql
_adb_meta          -- schema version, timestamps
_adb_collections   -- vector collection registry
_adb_vectors       -- raw vector blobs + metadata
_adb_hnsw_index    -- serialized HNSW graph per collection
_adb_nodes         -- memory graph nodes
_adb_edges         -- memory graph edges
```

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| SQLite as storage | Proven, embedded, ACID, single-file, zero config |
| WAL mode | Concurrent reads while writing |
| Pure Rust HNSW | No C deps, memory safe, serializable |
| Recursive CTEs for graph | Let SQLite do graph work, not Rust |
| bincode for index blobs | Fast, compact binary serialization |
| Lazy index build | Don't pay for indexing until first search |
