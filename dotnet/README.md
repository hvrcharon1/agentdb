# AgentDB — .NET SDK

.NET 8 P/Invoke bindings for [AgentDB](https://github.com/hvrcharon1/agentdb).

## Prerequisites

### 1. Build the native shared library

```bash
# From the repository root
cargo build --release --features ffi --lib

# Output:
#   Linux   → target/release/libagentdb.so
#   macOS   → target/release/libagentdb.dylib
#   Windows → target/release/agentdb.dll
```

### 2. Make the library visible to the .NET runtime

The P/Invoke layer loads the library by its base name `agentdb`. The runtime
resolves this to the platform-appropriate file name automatically.

```bash
# Linux — add to library search path
export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:/path/to/agentdb/target/release"

# macOS
export DYLD_LIBRARY_PATH="$DYLD_LIBRARY_PATH:/path/to/agentdb/target/release"

# Windows — add to PATH or copy agentdb.dll to the output directory
$env:PATH += ";C:\path\to\agentdb\target\release"
```

The simplest approach for development is to copy (or symlink) the shared
library into the same directory as your application's output binary.

### 3. Build the .NET SDK

```bash
cd dotnet
dotnet build
dotnet pack   # produces Datacules.AgentDB.0.3.4.nupkg
```

## Installation (from NuGet)

```bash
dotnet add package Datacules.AgentDB
```

## Quick start

```csharp
using Datacules.AgentDB;

// Open (or create) a database.  using ensures Dispose is called.
using var db = AgentDB.Open("agent.db");

// SQL
db.Execute("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, data TEXT)");
db.Execute("INSERT OR REPLACE INTO sessions VALUES ('s1','{\"turns\":3}')");
string rows = db.QueryJson("SELECT * FROM sessions");
Console.WriteLine(rows);

// Vector upsert and search
float[] embedding = [0.1f, 0.2f, 0.3f, 0.4f];
db.VectorUpsert("docs", "doc-1", embedding, """{"title":"hello"}""");
string results = db.VectorSearch("docs", embedding, topK: 5);
Console.WriteLine(results);

// Memory graph
db.GraphAddNode("session:s1", "session", """{"agent":"gpt-4o"}""");
db.GraphAddNode("concept:llm", "concept");
db.GraphAddEdge("session:s1", "concept:llm", "mentions", weight: 0.9);
string neighbors = db.GraphNeighbors("session:s1", maxDepth: 2, minWeight: 0.5);
Console.WriteLine(neighbors);

// Full-text search
db.FtsIndex("docs", "doc-1", "docs", "AgentDB is an embedded AI-agent database");
string fts = db.FtsSearch("docs", "embedded database", topK: 5);
Console.WriteLine(fts);

// Hybrid query
string hybrid = db.HybridQuery("session:s1", embedding, "docs",
    graphDepth: 2, topK: 5, alpha: 0.5);
Console.WriteLine(hybrid);

// Statistics
Console.WriteLine(db.Stats());
```

## API reference

| Method | Returns | Description |
|--------|---------|-------------|
| `AgentDB.Open(path)` | `AgentDB` | Open/create database. Use `":memory:"` for ephemeral. |
| `db.Dispose()` / `using` | `void` | Release native resources. |
| `db.Execute(sql)` | `long` | DDL/DML; returns rows affected. |
| `db.QueryJson(sql)` | `string` JSON array | SELECT → JSON rows. |
| `db.VectorUpsert(col, id, vec, meta?)` | `void` | Upsert vector with optional JSON metadata. |
| `db.VectorSearch(col, query, topK, filter?)` | `string` JSON array | ANN search. |
| `db.GraphAddNode(id, kind, data?)` | `void` | Upsert memory-graph node. |
| `db.GraphAddEdge(src, dst, rel, weight)` | `void` | Upsert directed edge. |
| `db.GraphNeighbors(id, depth, minW?)` | `string` JSON array | BFS/DFS traversal. |
| `db.FtsIndex(col, vecId, colId, text)` | `void` | Index document text. |
| `db.FtsSearch(col, query, topK)` | `string` JSON array | FTS with snippets. |
| `db.HybridQuery(anchor, emb, col, depth, k, α)` | `string` JSON array | Blended graph + vector ranking. |
| `db.Stats()` | `string` JSON object | Collection / vector / node / edge counts. |

All error paths throw `AgentDBException` (inherits `Exception`).

## JSON shapes

**VectorSearch / HybridQuery results:**

```json
[
  { "id": "doc-1", "score": 0.98, "metadata": { "title": "hello" } }
]
```

**GraphNeighbors results:**

```json
[
  { "id": "concept:llm", "kind": "concept", "depth": 1, "weight": 0.9, "data": null }
]
```

**FtsSearch results:**

```json
[
  { "id": "doc-1", "snippet": "…embedded <b>AI-agent database</b>…", "rank": 1.5 }
]
```

**Stats:**

```json
{ "collections": 1, "vectors": 42, "nodes": 10, "edges": 15 }
```
