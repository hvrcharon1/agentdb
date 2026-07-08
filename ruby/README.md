# AgentDB Ruby SDK

Ruby bindings for [AgentDB](https://github.com/hvrcharon1/agentdb) — a
single-file embedded database for AI agents.

AgentDB combines:
- **SQL storage** — full SQLite engine, relational tables
- **Vector search** — HNSW approximate nearest-neighbour (ANN)
- **Full-text search** — SQLite FTS5 / BM25
- **Hybrid queries** — weighted blend of graph traversal + vector similarity
- **Memory graph** — directed weighted graph of nodes and edges
- **Conversations** — message history with role/content
- **Workflows** — multi-step state machine with input/output
- **Traces** — hierarchical reasoning trace trees

## Requirements

- Ruby >= 2.7
- `ffi` gem (~> 1.15)
- The compiled `libagentdb` shared library (`.so` / `.dylib` / `.dll`)

## Building the shared library

```bash
cd /path/to/agentdb
cargo build --release --features ffi --lib
# Outputs:
#   Linux:   target/release/libagentdb.so
#   macOS:   target/release/libagentdb.dylib
#   Windows: target/release/agentdb.dll
```

Place the shared library on your system library path (or set `LD_LIBRARY_PATH`
/ `DYLD_LIBRARY_PATH`) before `require`-ing this gem.

## Installation

Add to your `Gemfile`:

```ruby
gem "agentdb"
```

Or install directly:

```bash
gem install agentdb
```

## Quick start

```ruby
require 'agentdb'

# Open (or create) a database file
db = AgentDB::Database.new("agent.agentdb")

# Use the block form to ensure the handle is always closed
AgentDB::Database.open("agent.agentdb") do |db|

  # --- SQL -----------------------------------------------------------------
  db.execute("CREATE TABLE IF NOT EXISTS notes (id TEXT, body TEXT)")
  db.execute("INSERT INTO notes VALUES ('n1', 'hello world')")
  rows = db.query("SELECT * FROM notes")
  # => [{"id"=>"n1", "body"=>"hello world"}]

  # Parameterised queries
  rows = db.query_params("SELECT * FROM notes WHERE id = ?", ["n1"])

  # --- Vectors -------------------------------------------------------------
  col = db.collection("memories", 1536)

  embedding = Array.new(1536) { rand(-1.0..1.0) }  # your real embedding here
  col.upsert("mem1", embedding, { topic: "ruby", source: "docs" })

  query_embedding = Array.new(1536) { rand(-1.0..1.0) }
  results = col.search(query_embedding, top_k: 5)
  # => [{"id"=>"mem1", "score"=>0.97, "metadata"=>{"topic"=>"ruby", ...}}, ...]

  # Filtered search
  results = col.search(query_embedding,
                       top_k: 5,
                       filter: { "topic" => { "$eq" => "ruby" } })

  # Delete a vector
  col.delete("mem1")

  # --- Full-text search ----------------------------------------------------
  col.fts_index("mem1", "doc-001", "AgentDB is a fast embedded database")
  fts_results = col.fts_search("embedded database", top_k: 10)
  # => [{"id"=>"mem1", "snippet"=>"...", "rank"=>-1.2}]

  # --- Memory graph --------------------------------------------------------
  db.graph_add_node("session-42",  "session",  { user: "alice" })
  db.graph_add_node("concept-ruby","concept",  { label: "Ruby" })
  db.graph_add_edge("session-42", "concept-ruby", "discussed", 0.9)

  neighbours = db.graph_neighbors("session-42", max_depth: 2, min_weight: 0.5)
  # => [{"id"=>"concept-ruby", "kind"=>"concept", "depth"=>1, "weight"=>0.9, ...}]

  # --- Hybrid query --------------------------------------------------------
  results = db.hybrid_query(
    "session-42",         # anchor node
    query_embedding,      # vector query
    1536,                 # embedding dimensions
    collection: "memories",
    graph_depth: 2,
    top_k: 10,
    alpha: 0.6            # 0.0 = pure graph, 1.0 = pure vector
  )

  # --- Conversations -------------------------------------------------------
  db.conversation_create("conv-1", title: "Support session")
  db.conversation_add_message("conv-1", "user",      "How do I use AgentDB?")
  db.conversation_add_message("conv-1", "assistant", "Just require 'agentdb'!")
  messages = db.conversation_messages("conv-1")

  # --- Workflows -----------------------------------------------------------
  db.workflow_create("wf-1", "Data pipeline", input: { source: "s3://bucket" })
  step_id = db.workflow_add_step("wf-1", "download")
  db.workflow_update_step(step_id, "running")
  db.workflow_update_step(step_id, "completed", output: { rows: 1_000 })
  db.workflow_complete("wf-1", output: { total_rows: 1_000 })
  workflow = db.workflow_get("wf-1")   # => Hash with status, steps, ...

  # --- Traces --------------------------------------------------------------
  trace_id = db.trace_add("thought", "I should retrieve recent memories",
                           session_id: "sess-42")
  child_id  = db.trace_add("action",  "Calling vector search",
                           session_id: "sess-42", parent_id: trace_id)
  tree = db.trace_get_tree(trace_id)

  # --- Stats ---------------------------------------------------------------
  puts db.stats
  # => {"collections"=>1, "vectors"=>1, "nodes"=>2, "edges"=>1, ...}

end
```

## Error handling

All errors are subclasses of `AgentDB::Error`:

| Class                          | Raised when                                    |
|--------------------------------|------------------------------------------------|
| `AgentDB::LibraryNotFoundError`| Shared library cannot be found at load time    |
| `AgentDB::DatabaseError`       | Database cannot be opened, or is already closed |
| `AgentDB::FFIError`            | A C function returns an error code / NULL      |

```ruby
begin
  db = AgentDB::Database.new("readonly_path/db.agentdb")
rescue AgentDB::DatabaseError => e
  warn "Could not open database: #{e.message}"
end
```

## API reference

### `AgentDB::Database`

| Method | Description |
|--------|-------------|
| `.new(path)` | Open or create a database |
| `.open(path) { \|db\| }` | Block form — auto-closes |
| `#close` | Release the native handle |
| `#closed?` | Returns `true` if closed |
| `#execute(sql)` | Run DDL/DML, returns affected-row count |
| `#query(sql)` | SELECT → `Array<Hash>` |
| `#query_params(sql, params)` | Parameterised SELECT |
| `#collection(name, dim)` | Returns an `AgentDB::Collection` |
| `#stats` | Database statistics Hash |
| `#graph_add_node(id, kind, data)` | Upsert a graph node |
| `#graph_add_edge(src, dst, rel, weight)` | Upsert a graph edge |
| `#graph_neighbors(id, ...)` | Traverse neighbours |
| `#graph_get_node(id)` | Fetch one node |
| `#graph_delete_node(id)` | Remove node + edges |
| `#graph_delete_edge(src, dst, rel)` | Remove specific edge |
| `#hybrid_query(anchor, emb, dim, ...)` | Hybrid graph+vector search |
| `#conversation_create(id, ...)` | Create a conversation |
| `#conversation_add_message(id, role, content, ...)` | Append a message |
| `#conversation_messages(id, limit:)` | Fetch messages |
| `#conversation_list` | List all conversations |
| `#conversation_delete(id)` | Delete conversation + messages |
| `#workflow_create(id, name, ...)` | Create a workflow |
| `#workflow_add_step(wf_id, name, ...)` | Append a step |
| `#workflow_update_step(step_id, status, ...)` | Update step status |
| `#workflow_complete(id, ...)` | Mark completed |
| `#workflow_fail(id, ...)` | Mark failed |
| `#workflow_get(id)` | Fetch workflow + steps |
| `#workflow_list(status:)` | List workflows |
| `#trace_add(type, content, ...)` | Record a trace |
| `#trace_get_by_session(session_id)` | Session trace list |
| `#trace_get_tree(root_id)` | Trace subtree |

### `AgentDB::Collection`

| Method | Description |
|--------|-------------|
| `#upsert(id, vector, metadata)` | Insert or update a vector |
| `#search(query, top_k:, filter:)` | ANN search |
| `#delete(id)` | Remove a vector |
| `#drop` | Drop the entire collection |
| `#reindex` | Rebuild HNSW index |
| `#fts_index(vec_id, col_id, text)` | Index a document |
| `#fts_search(query, top_k:)` | Full-text search |
| `#fts_delete(vec_id)` | Remove from FTS index |
| `#fts_optimize` | Merge FTS segments |

## Running the specs

```bash
# From the ruby/ directory:
bundle install
bundle exec rspec
```

The specs require the compiled shared library. They will fail to load if
`libagentdb` is not on the library path.

## License

This SDK is released under the [Unlicense](https://unlicense.org/).
