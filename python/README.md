# AgentDB Python SDK

Single-file embedded database for AI agents. SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs + Conversations + Workflows + Reasoning Traces — all in one `.agentdb` file.

## Installation

```bash
pip install datacules-agentdb
```

Requires Python 3.9+. No external database server needed.

## Quick Start

```python
import agentdb

# Open or create a database (use ":memory:" for in-process only)
db = agentdb.AgentDB.open("agent.agentdb")

# SQL — full SQLite syntax
db.execute("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, ts TEXT)")
db.execute("INSERT INTO sessions VALUES ('s1', '2026-06-19')")
rows = db.query("SELECT * FROM sessions")
print(rows)  # [{'id': 's1', 'ts': '2026-06-19'}]

# Vector search — HNSW index, cosine similarity
col = db.vectors.collection("thoughts", dim=4)
col.upsert("t1", [0.9, 0.1, 0.0, 0.0], metadata={"topic": "memory"})
col.upsert("t2", [0.1, 0.9, 0.0, 0.0], metadata={"topic": "reasoning"})
results = col.search([0.85, 0.15, 0.0, 0.0], top_k=1)
print(results[0].id)  # 't1'

# Full-text search
db.fts.index("docs", "d1", "s1", "AgentDB stores agent memory efficiently")
hits = db.fts.search("docs", "memory", top_k=5)

# Memory graph — associative knowledge store
db.memory.add_node("s1", "session")
db.memory.add_node("t1", "thought")
db.memory.add_edge("s1", "t1", "recalled", weight=0.9)
neighbors = db.memory.traverse("s1", max_depth=2)

# Conversations
conv_id = db.conversations.create_conversation(title="Chat with user")
db.conversations.add_message(conv_id, "user", "What is AgentDB?")
db.conversations.add_message(conv_id, "assistant", "A single-file AI database.")
messages = db.conversations.get_messages(conv_id)

# Workflows
wf_id = db.workflows.create_workflow("summarize", input_data='{"url":"..."}')
step_id = db.workflows.add_step(wf_id, "fetch")
db.workflows.update_step(step_id, "completed", output='{"text":"..."}')
db.workflows.complete_workflow(wf_id)

# Reasoning traces
trace_id = db.traces.add_trace("s1", "thought", "Should I use vector search here?")
db.traces.add_trace("s1", "observation", "Query matches 3 results.", parent_id=trace_id)

# Stats
print(db.stats())
# {'collections': 1, 'vectors': 2, 'nodes': 2, 'edges': 1}
```

## API Reference

See the [full documentation](https://github.com/hvrcharon1/agentdb#api-reference) in the main README.

## License

Unlicense — public domain. See [LICENSE](https://github.com/hvrcharon1/agentdb/blob/main/LICENSE).
