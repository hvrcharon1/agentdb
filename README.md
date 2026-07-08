<p align="center">
  <img src="./assets/logo.svg" alt="AgentDB" width="420"/>
</p>

<p align="center">
  <b>The default embedded database for AI agents.</b><br/>
  One file. Eight layers. Every platform. Zero servers.<br/>
  Semantic Memory &nbsp;·&nbsp; Vector Search &nbsp;·&nbsp; Memory Graphs &nbsp;·&nbsp; Full-Text Search &nbsp;·&nbsp; Hybrid Queries &nbsp;·&nbsp; Conversations &nbsp;·&nbsp; Workflows &nbsp;·&nbsp; Reasoning Traces<br/>
  Rust &nbsp;·&nbsp; Python &nbsp;·&nbsp; Node.js &nbsp;·&nbsp; Go &nbsp;·&nbsp; Java &nbsp;·&nbsp; C# &nbsp;·&nbsp; C/C++ &nbsp;·&nbsp; WASM &nbsp;·&nbsp; CLI
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
  <a href="https://crates.io/crates/datacules-agentdb">
    <img src="https://img.shields.io/crates/v/datacules-agentdb.svg" alt="crates.io"/>
  </a>
  &nbsp;
  <a href="https://pypi.org/project/datacules-agentdb/">
    <img src="https://img.shields.io/pypi/v/datacules-agentdb.svg" alt="PyPI"/>
  </a>
  &nbsp;
  <a href="https://www.npmjs.com/package/@datacules/agentdb">
    <img src="https://img.shields.io/npm/v/@datacules/agentdb.svg" alt="npm"/>
  </a>
  &nbsp;
  <img src="https://img.shields.io/badge/by-Datacules%20LLC-lightgrey.svg" alt="Datacules LLC"/>
</p>

---

## Philosophy

AgentDB is the default embedded database for AI agents. It is a single file, zero configuration, cross-platform database that works consistently across desktop, mobile, edge, browser, server, CLI, and embedded devices. Every feature is built to optimize for agentic workloads — semantic memory, vector search, graph relationships, conversations, workflows, tool executions, and structured knowledge — while remaining lightweight, portable, deterministic, and developer-friendly.

AgentDB prioritizes local-first operations, high performance, reliability, language interoperability, and simple APIs so developers can drop it into any application and immediately give AI agents persistent, intelligent memory. Every new capability reinforces the philosophy of **embed, open, use** — minimal setup, maximum portability.

### Core Principles

| # | Principle | What it means |
|---|---|---|
| 1 | **AI-native first, not AI as an add-on** | Every API, schema, and default is designed around agentic access patterns — not retrofitted onto a general-purpose database |
| 2 | **One file, one API, every platform** | A single `.agentdb` file works identically whether you open it from Python, Rust, Node.js, Go, Java, or the CLI |
| 3 | **Offline-first with optional synchronization** | Full capability without any network connection; sync is an opt-in layer, not a requirement |
| 4 | **Deterministic and reproducible agent memory** | Given the same inputs, agents produce the same outputs — no hidden server state, no non-deterministic remote indexes |
| 5 | **Built-in semantic primitives** | Vectors, graphs, memory, workflows, conversations, and reasoning traces are first-class citizens, not plugins |
| 6 | **Language-agnostic** | Native bindings for Rust, Python, JavaScript, Go, Java, C#, C/C++, Swift, Kotlin, Ruby, and more |
| 7 | **No infrastructure required** | No server. No daemon. No configuration. Drop in a file and start writing |
| 8 | **Scales from a phone to a data center** | The same programming model works on a microcontroller, a laptop, and a cloud server — without code changes |

---

## What is AgentDB?

AgentDB is a **single-file, embedded database** purpose-built for AI agents and LLM-powered applications. It gives your agent a complete, production-ready persistence layer — relational SQL, semantic vector search, memory graphs, full-text search, conversation threading, workflow state, and reasoning traces — all in **one `.agentdb` file**, with no servers, no daemons, and no configuration.

```python
import agentdb

db = agentdb.AgentDB.open("my_agent.agentdb")
# That's it. Your agent now has SQL + vectors + graphs + conversations + more.
```

AgentDB is written in Rust for performance and safety, and ships native bindings for **Python, Node.js, Go, Java, C#, C/C++**, plus a **CLI** and **WASM** target for the browser.

---

## Who Is AgentDB For?

AgentDB is designed for developers who are building on top of AI models and need reliable, fast, local-first persistence — without stitching together multiple services.

**You'll love AgentDB if you are:**

- Building an **AI agent** that needs to remember past interactions and recall them by semantic similarity
- Developing a **RAG pipeline** and tired of running a separate vector database
- Creating a **conversational application** that needs threaded message history with metadata
- Running **multi-step agentic workflows** and need durable, resumable state
- Shipping an **edge or offline-capable AI app** where network calls to external services aren't an option
- A researcher or indie developer who wants the power of Chroma + Neo4j + a full relational database **in one pip install**

---

## Quick Start

### Python — 60 seconds to your first agent memory

```bash
pip install datacules-agentdb
```

```python
import agentdb
import numpy as np

# Open (or create) a database
db = agentdb.AgentDB.open("my_agent.agentdb")

# Store a conversation
conv = db.conversations()
conv.create_conversation("chat_1", title="First session")
conv.add_message("chat_1", "user", "What is the capital of France?")
conv.add_message("chat_1", "assistant", "The capital of France is Paris.")

# Store and search a vector memory
col = db.collection("memories", dim=1536)
embedding = np.random.rand(1536).tolist()   # replace with your real embedding
col.upsert("mem_1", embedding, metadata={"topic": "geography", "score": 9})

results = col.search(embedding, top_k=5)
print(results)

# Query with SQL
rows = db.query_json("SELECT * FROM _adb_conversations")
print(rows)
```

### Node.js / TypeScript

```bash
npm install @datacules/agentdb
```

```typescript
import { AgentDB } from '@datacules/agentdb';

const db = AgentDB.open('my_agent.agentdb');

const col = db.collection('memories', 1536);
col.upsert('mem_1', queryEmbedding, { topic: 'geography' });
const results = col.search(queryEmbedding, { topK: 5 });
```

### Rust

```bash
cargo add datacules-agentdb
```

```rust
use agentdb::AgentDB;

let db = AgentDB::open("my_agent.agentdb")?;
let col = db.vectors().collection("memories", 1536)?;
col.upsert(VectorEntry { id: "mem_1".into(), vector: embedding, metadata: None })?;
let results = col.search(&query, SearchOptions { top_k: 5, ..Default::default() })?;
```

---

## Eight Capabilities in One File

AgentDB bundles eight storage and query primitives that typically require separate services — all in a single embedded file your application owns and controls.

### 1 — Relational SQL
Full SQL with joins, CTEs, transactions, and indexes. Store any structured data alongside your agent's memory — sessions, users, logs, events — and query it all with standard SQL.

### 2 — Vector Search
Semantic similarity search using a pure-Rust HNSW index. Search hundreds of thousands of embeddings in milliseconds with support for cosine, euclidean, and dot-product similarity, plus MongoDB-style metadata filtering.

```
Sub-50 ms ANN on 100,000 vectors at 1,536 dimensions (OpenAI text-embedding-3-small size)
```

### 3 — Memory Graph
Model relationships between concepts, entities, and sessions as a typed, weighted graph. Traverse connections with depth-limited queries — ideal for knowledge graphs, agent memory networks, and relationship-aware retrieval.

```
Graph traversal < 5 ms on 10,000 nodes at depth 2
```

### 4 — Full-Text Search
BM25-ranked full-text search with Porter stemming and snippet extraction. Index any content your agent sees and retrieve it by keyword in milliseconds — no Elasticsearch required.

### 5 — Hybrid Queries
Blend graph traversal and vector similarity in a single query with a tunable alpha parameter. Get results that are both contextually connected *and* semantically relevant.

### 6 — Conversation Threading
First-class message threading for any interaction your agent has. Store multi-turn conversations with roles, content, and per-message metadata. Retrieve full history in chronological order.

### 7 — Workflow Persistence
Durable, resumable state for multi-step agent tasks. Track workflow runs and individual steps — with status, inputs, outputs, and errors — so your agent can survive restarts and resume exactly where it left off.

### 8 — Reasoning Traces
Tree-structured logs for chain-of-thought, tool calls, and decision sequences. Every step of your agent's reasoning can be persisted, queried, and replayed — invaluable for debugging, auditing, and evaluation.

---

## Performance

Benchmarks run on GitHub Actions (`ubuntu-latest`, 4 vCPU, 16 GB RAM, Rust stable, `release` profile).

| Operation | Scale | Latency |
|---|---|---|
| Vector search (ANN, cosine) | 100k vectors, 1,536 dims | **~47 ms** |
| Vector search (ANN, cosine) | 10k vectors, 1,536 dims | **~8.7 ms** |
| Graph traversal (depth 2) | 10k nodes, 50k edges | **~0.5 ms** |
| Graph traversal (depth 5) | 100k nodes, 500k edges | **~19 ms** |
| Full-text search (BM25) | 100k documents | **~1.2 ms** |
| SQL INSERT (WAL mode) | single row | **~0.09 ms** |
| Vector upsert | single entry | **~0.2 ms** |
| Batch upsert | 1,000 vectors | **~28 ms** |

Full benchmark details in [BENCHMARKS.md](BENCHMARKS.md).

---

## Multi-Language Support

AgentDB ships a native library for every major language in the AI stack. There is no language-level performance penalty — every SDK wraps the same Rust core.

| Language | Install | Docs |
|---|---|---|
| **Python** | `pip install datacules-agentdb` | CPython 3.9+, PyPy, Linux / macOS / Windows |
| **Node.js** | `npm install @datacules/agentdb` | TypeScript types included |
| **Rust** | `cargo add datacules-agentdb` | Full API on [docs.rs](https://docs.rs/datacules-agentdb) |
| **Go** | `import "github.com/hvrcharon1/agentdb/go"` | See [`go/README.md`](go/README.md) |
| **Java** | Maven — see [`java/README.md`](java/README.md) | JNI wrapper |
| **C# / .NET** | NuGet — see [`dotnet/README.md`](dotnet/README.md) | P/Invoke wrapper |
| **C / C++** | Build `libagentdb.so` / `.dylib` / `.dll` | Flat C API included |
| **WASM** | `wasm-pack build --target web` | In-memory databases today; OPFS persistence coming |
| **CLI** | See install options below | Interactive shell + all operations |

### CLI Install

| Platform | Command |
|---|---|
| Any (Cargo) | `cargo install datacules-agentdb` |
| macOS / Linux (Homebrew) | `brew install hvrcharon1/tap/agentdb` |
| Windows (Scoop) | `scoop bucket add agentdb https://github.com/hvrcharon1/scoop-bucket && scoop install agentdb` |
| Windows (Chocolatey) | `choco install agentdb` |
| Windows (WinGet) | `winget install Datacules.AgentDB` |
| Linux (Snap) | `snap install agentdb` |
| Nix | `nix run github:hvrcharon1/agentdb` |
| Shell | `curl -fsSL https://raw.githubusercontent.com/hvrcharon1/agentdb/main/install.sh \| sh` |
| PowerShell | `irm https://raw.githubusercontent.com/hvrcharon1/agentdb/main/install.ps1 \| iex` |

```bash
# Common CLI operations
agentdb shell      my_agent.agentdb          # interactive SQL REPL
agentdb stats      my_agent.agentdb          # database summary
agentdb inspect    my_agent.agentdb          # full report: stats + collections + graph
agentdb sql        my_agent.agentdb "SELECT * FROM sessions LIMIT 10"
agentdb search     my_agent.agentdb memories 0.9 0.1 0.0 --top-k 5
agentdb collections my_agent.agentdb         # list vector collections
agentdb reindex    my_agent.agentdb          # rebuild all HNSW indexes
```

### Docker

```bash
docker build -t agentdb .
docker run -v $(pwd):/data agentdb stats my_agent.agentdb
docker run -it -v $(pwd):/data agentdb shell my_agent.agentdb
```

---

## Why AgentDB?

Modern AI agents have storage needs that today require five or more separate tools — each with its own server, configuration, and network dependency. AgentDB collapses all of them into one embedded file.

| What your agent needs | Typical solution | The problem |
|---|---|---|
| Structured storage for sessions, logs, events | Relational database | No vector search, no graph |
| Semantic memory retrieval | ChromaDB, Qdrant, Pinecone | Separate service, network required |
| Relationship and knowledge graph | Neo4j, custom solution | Heavy, not embeddable, not offline |
| Keyword search over stored text | Elasticsearch, Typesense | Yet another service to operate |
| Combined graph + semantic retrieval | Custom code | Fragile, high latency, no standard |
| **All of the above** | **AgentDB** | **One file. Zero servers.** |

### Full Feature Comparison

| Feature | AgentDB | ChromaDB | Qdrant | Neo4j |
|---|---|---|---|---|
| Embedded (no server) | ✅ | ❌ | ❌ | ❌ |
| Single file | ✅ | ❌ | ❌ | ❌ |
| Zero-configuration | ✅ | ❌ | ❌ | ❌ |
| ACID transactions + WAL | ✅ | ❌ | ❌ | ✅ |
| Relational SQL | ✅ | ❌ | ❌ | ❌ |
| Vector / ANN search | ✅ | ✅ | ✅ | ❌ |
| Metadata filtering | ✅ | ⚠️ | ✅ | ❌ |
| Full-text search (BM25) | ✅ | ❌ | ❌ | ❌ |
| Memory graph | ✅ | ❌ | ❌ | ✅ |
| Hybrid graph + vector query | ✅ | ❌ | ❌ | ❌ |
| Conversation threading | ✅ | ❌ | ❌ | ❌ |
| Workflow persistence | ✅ | ❌ | ❌ | ❌ |
| Reasoning traces | ✅ | ❌ | ❌ | ❌ |
| Python | ✅ | ✅ | ✅ | ✅ |
| Node.js | ✅ | ✅ | ✅ | ✅ |
| Go | ✅ | ❌ | ✅ | ✅ |
| Java | ✅ | ❌ | ✅ | ✅ |
| C# / .NET | ✅ | ❌ | ✅ | ✅ |
| C FFI | ✅ | ❌ | ❌ | ❌ |
| WASM / browser | ✅ | ❌ | ❌ | ❌ |
| Works offline / on edge | ✅ | ❌ | ❌ | ❌ |
| Free / open source | ✅ | ✅ | ⚠️ | ⚠️ |

---

## API Overview

A brief map of what's available. Full API documentation lives on [docs.rs](https://docs.rs/datacules-agentdb).

| What you want to do | API entry point |
|---|---|
| Open / create a database | `AgentDB::open(path)` |
| Run SQL | `db.execute()`, `db.query_json()`, `db.transaction()` |
| Store & search vectors | `db.vectors().collection("name", dim)` |
| Add / traverse graph nodes | `db.memory()` |
| Index & search text | `db.fts()` |
| Graph + vector blended search | `db.hybrid_query(...)` |
| Manage conversations | `db.conversations()` |
| Persist workflow state | `db.workflows()` |
| Log reasoning traces | `db.traces()` |
| Get database stats | `db.stats()` |

---

## Roadmap

| Milestone | Status |
|---|---|
| v0.1.0 — Core (SQL + Vectors + Graphs) | ✅ Released |
| v0.2.0 — Query Power (FTS + Hybrid + Filters) | ✅ Released |
| v0.3.0 — Universal Availability (C FFI, CLI, Python, Node.js, WASM) | ✅ Released |
| v0.4.0 — AI-Native (Conversations, Workflows, Traces + Go/Java/.NET SDKs) | ✅ Released |
| v0.4.5 — Dep upgrades (rusqlite 0.40, pyo3 0.29, bincode 2, thiserror 2), MSRV 1.85 | ✅ Released |
| v0.5.0 — API completeness: 9 new FFI ops, full SDK parity (Go/Node/Java/.NET), hybrid filter, fail_workflow, 9-field DbStats | ✅ Released |
| v0.6.0 — AI-Native Architecture (Tools, Audit, Context, Prompts, Labels, MCP) | ✅ Released |
| v0.7.0 — Ecosystem Integrations (LangChain, LlamaIndex, AgentDB Sync) | 🔜 Next |
| v0.8.0 — WASM Persistence (OPFS) + Ruby SDK | Planned |
| v1.0.0 — Production Release | Planned |

Full detail in [ROADMAP.md](ROADMAP.md).

---

## Documentation

| Resource | Link |
|---|---|
| Full API reference | [docs.rs/datacules-agentdb](https://docs.rs/datacules-agentdb) |
| Architecture deep-dive | [ARCHITECTURE.md](ARCHITECTURE.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Migration guide | [MIGRATION.md](MIGRATION.md) |
| Performance benchmarks | [BENCHMARKS.md](BENCHMARKS.md) |
| Security policy | [SECURITY.md](SECURITY.md) |

---

## Contributing

AgentDB welcomes contributions. Whether you're fixing a bug, adding a language binding, or improving documentation — we'd love your help.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development setup, PR process, and coding standards.

Quick summary:
1. Fork the repository
2. Create a feature branch: `git checkout -b feat/your-feature`
3. Write tests for your changes
4. Run `cargo test` and `cargo clippy` — both must pass
5. Open a pull request with a clear description

To report a security vulnerability, follow the process in [SECURITY.md](SECURITY.md).

---

## License

AgentDB is released under the **Unlicense** — effectively public domain.  
You are free to use, copy, modify, distribute, and sublicense without restriction.  
See [LICENSE](LICENSE) and [NOTICE](NOTICE) for the full terms.

---

<p align="center">
  Built and maintained by <a href="https://datacules.com"><strong>Datacules LLC</strong></a>
</p>
