# AgentDB — Go SDK

Go bindings for [AgentDB](https://github.com/hvrcharon1/agentdb) via cgo.

## Prerequisites

1. **Build the native shared library** (requires Rust / Cargo):

   ```bash
   # from the repo root
   cargo build --release --features ffi --lib

   # Output:
   #   Linux   → target/release/libagentdb.so
   #   macOS   → target/release/libagentdb.dylib
   #   Windows → target/release/agentdb.dll
   ```

2. **Copy (or symlink) the shared library and C header** to a location on the
   compiler and linker search path, e.g.:

   ```bash
   # Linux / macOS (example using /usr/local)
   sudo cp target/release/libagentdb.so /usr/local/lib/
   sudo cp agentdb.h /usr/local/include/
   sudo ldconfig          # Linux only

   # Or set per-project:
   export CGO_CFLAGS="-I/path/to/repo"
   export CGO_LDFLAGS="-L/path/to/repo/target/release -lagentdb"
   export LD_LIBRARY_PATH="/path/to/repo/target/release"
   ```

## Installation

```bash
go get github.com/hvrcharon1/agentdb/go
```

## Quick start

```go
package main

import (
    "fmt"
    "log"

    "github.com/hvrcharon1/agentdb/go"
)

func main() {
    // Open (or create) a database
    db, err := agentdb.Open("agent.db")
    if err != nil {
        log.Fatal(err)
    }
    defer db.Close()

    // Run SQL
    if _, err := db.Execute(`CREATE TABLE IF NOT EXISTS sessions (
        id   TEXT PRIMARY KEY,
        data TEXT
    )`); err != nil {
        log.Fatal(err)
    }

    if _, err := db.Execute(`INSERT OR REPLACE INTO sessions VALUES ('s1','{"turns":3}')`); err != nil {
        log.Fatal(err)
    }

    rows, err := db.QueryJSON("SELECT * FROM sessions")
    if err != nil {
        log.Fatal(err)
    }
    fmt.Println(rows)

    // Upsert and search vectors
    embedding := []float32{0.1, 0.2, 0.3, 0.4}
    if err := db.VectorUpsert("docs", "doc-1", embedding, []byte(`{"title":"hello"}`)); err != nil {
        log.Fatal(err)
    }

    results, err := db.VectorSearch("docs", embedding, 5, nil)
    if err != nil {
        log.Fatal(err)
    }
    for _, r := range results {
        fmt.Printf("id=%s score=%.4f\n", r.ID, r.Score)
    }

    // Memory graph
    _ = db.GraphAddNode("session:s1", "session", []byte(`{"agent":"gpt-4o"}`))
    _ = db.GraphAddNode("concept:llm", "concept", nil)
    _ = db.GraphAddEdge("session:s1", "concept:llm", "mentions", 0.9)

    neighbors, err := db.GraphNeighbors("session:s1", 2, 0.5)
    if err != nil {
        log.Fatal(err)
    }
    fmt.Println(neighbors)

    // Statistics
    stats, err := db.Stats()
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("vectors=%d nodes=%d edges=%d\n", stats.Vectors, stats.Nodes, stats.Edges)
}
```

## API reference

| Method | Description |
|--------|-------------|
| `Open(path) (*DB, error)` | Open/create a database. Use `":memory:"` for ephemeral. |
| `db.Close()` | Release native resources. |
| `db.Execute(sql) (int64, error)` | Run DDL/DML; returns rows affected. |
| `db.QueryJSON(sql) (string, error)` | SELECT → JSON array of objects. |
| `db.VectorUpsert(col, id, vec, meta)` | Upsert a vector with optional JSON metadata. |
| `db.VectorSearch(col, query, topK, filter)` | Approximate nearest-neighbour search. |
| `db.GraphAddNode(id, kind, data)` | Add/update a memory-graph node. |
| `db.GraphAddEdge(src, dst, relation, weight)` | Add/update a directed edge. |
| `db.GraphNeighbors(id, depth, minWeight)` | BFS/DFS traversal from a node. |
| `db.FTSIndex(col, vecID, colID, text)` | Index text for full-text search. |
| `db.FTSSearch(col, query, topK)` | Full-text search with snippet highlights. |
| `db.HybridQuery(anchor, emb, col, depth, k, α)` | Blended graph + vector ranking. |
| `db.Stats() (Stats, error)` | Snapshot of collection / vector / node / edge counts. |

## Building on Windows

On Windows the import library is `agentdb.lib` and the DLL is `agentdb.dll`.
Set `CGO_LDFLAGS=-L<path> -lagentdb` and ensure `agentdb.dll` is on `%PATH%`.
