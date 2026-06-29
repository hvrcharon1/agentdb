# AgentDB Architecture

## Overview

AgentDB is built on eight storage layers that all live inside a single `.agentdb` file.
A proven embedded storage engine provides WAL mode and ACID guarantees as the foundation.
AgentDB adds vector indexing, graph traversal, full-text search, conversation threading,
workflow persistence, and reasoning traces as first-class citizens on top.

---

## Layer 1 — Relational (SQL)

- Embedded database access with WAL mode enabled
- Users can create any tables they need alongside AgentDB's internal `_adb_*` tables
- Full SQL support: joins, CTEs, transactions, indexes

## Layer 2 — Vector Store

- Collections stored in `_adb_collections` (metadata) and `_adb_vectors` (raw data)
- Vectors serialized as little-endian `f32` byte arrays as BLOBs
- HNSW index built lazily on first `search()` call, serialized via `bincode` into `_adb_hnsw_index`
- `is_dirty` flag triggers rebuild on close or manual `reindex()`
- Supports cosine, euclidean, and dot-product distance metrics

## Layer 3 — Memory Graph

- Nodes: `_adb_nodes` (id, kind, JSON data)
- Edges: `_adb_edges` (src, dst, relation, weight)
- Traversal via recursive CTEs — no in-memory graph library needed
- Depth-limited, weight-filtered, relation-filtered traversal

## Layer 4 — Full-Text Search

- FTS5 virtual table (`_adb_fts_content`) per collection with BM25 ranking and Porter stemmer
- Shadow index data in `_adb_fts_idx`
- `snippet()` extraction; `optimize()` for segment merging

## Layer 5 — Conversation Threading

- `_adb_conversations` (id, title, metadata, timestamps) and `_adb_messages` (role, content, metadata, ordering)
- Chronological message retrieval; supports arbitrary metadata JSON per conversation and message
- `ConversationStore` API: `create_conversation`, `add_message`, `get_messages`, `list_conversations`, `delete_conversation`

## Layer 6 — Workflow Persistence

- `_adb_workflows` (id, name, status, input, output, error, metadata) and `_adb_workflow_steps` (workflow_id, name, status, metadata, ordering)
- Step status tracking (`pending` → `running` → `completed` / `failed`)
- `WorkflowStore` API: `create_workflow`, `add_step`, `update_step`, `complete_workflow`, `fail_workflow`, `get_workflow`, `list_workflows`

## Layer 7 — Reasoning Traces

- `_adb_traces` (id, parent_id, kind, data, timestamps) — tree-structured via parent_id
- Subtree retrieval via recursive CTE; stores tool call logs, decision traces, chain-of-thought
- `TraceStore` API: `add_trace`, `get_traces`, `get_trace_tree`

## Layer 8 — Schema Version

- `_adb_meta` holds `schema_version` and `created_at` keys
- Opening a database with a mismatched schema version returns a `SchemaMigration` error;
  run `agentdb migrate <path>` or call `schema::migrate(&conn)` to upgrade in place

---

## Internal Tables

```sql
_adb_meta              -- schema version, creation timestamp
_adb_collections       -- vector collection registry
_adb_vectors           -- raw vector blobs + metadata
_adb_hnsw_index        -- serialized HNSW index blobs (one per collection)
_adb_nodes             -- memory graph nodes (id, kind, JSON data)
_adb_edges             -- memory graph edges (src, dst, relation, weight)
_adb_fts_content       -- FTS5 virtual table (full-text document content)
_adb_fts_idx           -- FTS5 shadow index data
_adb_conversations     -- conversation thread headers (title, metadata)
_adb_messages          -- individual messages within a conversation
_adb_workflows         -- durable workflow records (name, status, metadata)
_adb_workflow_steps    -- individual steps within a workflow
_adb_traces            -- reasoning trace nodes (tree via parent_id)
```

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Embedded storage engine | Proven, ACID, single-file, zero config |
| WAL mode | Concurrent reads while writing |
| Pure Rust HNSW | No C deps, memory safe, serializable |
| Recursive CTEs for graph | Let the storage engine do graph work, not Rust |
| bincode for index blobs | Fast, compact binary serialization |
| Lazy index build | Don't pay for indexing until first search |
