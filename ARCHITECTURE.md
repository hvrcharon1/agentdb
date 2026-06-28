# AgentDB Architecture

## Overview

AgentDB is built on eight storage layers that all live inside a single `.agentdb` file.
A proven embedded storage engine provides WAL mode and ACID guarantees as the foundation.
AgentDB adds vector indexing, graph traversal, full-text search, conversation threading,
workflow persistence, and reasoning traces as first-class citizens on top.

---

## Layer 1 — Relational (SQL)

- Embedded database access with WAL mode enabled
- Users can create any tables they need alongside AgentDB's internal `agentdb_*` tables
- Full SQL support: joins, CTEs, transactions, indexes

## Layer 2 — Vector Store

- Collections stored in `agentdb_collections` (metadata) and `agentdb_vectors` (raw data)
- Vectors serialized as little-endian `f32` byte arrays as BLOBs
- HNSW index built lazily on first `search()` call, serialized via `bincode` into `agentdb_hnsw_index`
- `is_dirty` flag triggers rebuild on close or manual `reindex()`
- Supports cosine, euclidean, and dot-product distance metrics

## Layer 3 — Memory Graph

- Nodes: `agentdb_graph_nodes` (id, kind, JSON data)
- Edges: `agentdb_graph_edges` (src, dst, relation, weight)
- Traversal via recursive CTEs — no in-memory graph library needed
- Depth-limited, weight-filtered, relation-filtered traversal

## Layer 4 — Full-Text Search

- FTS5 virtual table (`agentdb_fts_content`) per collection with BM25 ranking and Porter stemmer
- Shadow index data in `agentdb_fts_idx`
- `snippet()` extraction; `optimize()` for segment merging

## Layer 5 — Conversation Threading

- `agentdb_conversations` (id, title, metadata, timestamps) and `agentdb_messages` (role, content, metadata, ordering)
- Chronological message retrieval; supports arbitrary metadata JSON per conversation and message
- `ConversationStore` API: `create`, `append_message`, `messages`, `list`

## Layer 6 — Workflow Persistence

- `agentdb_workflows` (id, name, status, metadata) and `agentdb_workflow_steps` (workflow_id, name, status, metadata, ordering)
- Step status tracking (`pending` → `running` → `complete` / `failed`)
- `WorkflowStore` API: `create`, `add_step`, `update_step_status`, `complete`, `fail`

## Layer 7 — Reasoning Traces

- `agentdb_traces` (id, parent_id, kind, data, timestamps) — tree-structured via parent_id
- Subtree retrieval via recursive CTE; stores tool call logs, decision traces, chain-of-thought
- `TraceStore` API: `create`, `add_child`, `subtree`

## Layer 8 — Schema Version

- `agentdb_schema_version` tracks the current schema version (currently v2) and migration timestamps
- Auto-migrates from v1 to v2 on first open; existing data is untouched

---

## Internal Tables

```sql
agentdb_schema_version  -- schema version, migration timestamps
agentdb_collections     -- vector collection registry
agentdb_vectors         -- raw vector blobs + metadata
agentdb_graph_nodes     -- memory graph nodes (id, kind, JSON data)
agentdb_graph_edges     -- memory graph edges (src, dst, relation, weight)
agentdb_fts_content     -- FTS5 virtual table (full-text document content)
agentdb_fts_idx         -- FTS5 shadow index data
agentdb_conversations   -- conversation thread headers (title, metadata)
agentdb_messages        -- individual messages within a conversation
agentdb_workflows       -- durable workflow records (name, status, metadata)
agentdb_workflow_steps  -- individual steps within a workflow
agentdb_traces          -- reasoning trace nodes (tree via parent_id)
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
