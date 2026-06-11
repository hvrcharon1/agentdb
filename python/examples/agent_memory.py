"""Quick-start: agent memory management with AgentDB.

Run with::

    cd python
    pip install -e .
    python examples/agent_memory.py

This script demonstrates the five AgentDB layers in 60 lines:
  1. Relational SQL (CREATE TABLE / INSERT / SELECT)
  2. Vector store (upsert + ANN search)
  3. Memory graph (nodes, edges, traversal)
  4. Statistics
"""

import agentdb

# Open an in-memory database (replace ':memory:' with a file path for persistence)
db = agentdb.AgentDB.open(":memory:")

# ── 1. Relational SQL ─────────────────────────────────────────────────
db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT)")
db.execute("INSERT INTO sessions VALUES ('s1', 'Research Sprint')")
db.execute("INSERT INTO sessions VALUES ('s2', 'Planning Session')")

rows = db.query_json("SELECT * FROM sessions ORDER BY id")
print("Sessions:")
for row in rows:
    print(f"  {row}")

# ── 2. Vector store ───────────────────────────────────────────────────
# Create (or open) a collection of 4-dimensional embeddings.
col = db.vectors.collection("thoughts", dim=4)

col.upsert("t1", [0.9, 0.1, 0.0, 0.0], metadata={"topic": "RL",  "score": 9})
col.upsert("t2", [0.1, 0.9, 0.0, 0.0], metadata={"topic": "CV",  "score": 7})
col.upsert("t3", [0.5, 0.5, 0.0, 0.0], metadata={"topic": "NLP", "score": 8})

query_vec = [0.85, 0.15, 0.0, 0.0]
results = col.search(query_vec, top_k=2)
print(f"\nNearest thoughts to {query_vec[:2]}...:")
for r in results:
    print(f"  id={r.id}  score={r.score:.4f}  meta={r.metadata}")

# ── 3. Memory graph ───────────────────────────────────────────────────
db.memory.add_node("s1", "session", data={"name": "Research Sprint"})
db.memory.add_node("t1", "thought", data={"topic": "RL"})
db.memory.add_node("t2", "thought", data={"topic": "CV"})
db.memory.add_node("t3", "thought", data={"topic": "NLP"})

db.memory.add_edge("s1", "t1", "recalled", weight=0.9)
db.memory.add_edge("s1", "t2", "recalled", weight=0.7)
db.memory.add_edge("s1", "t3", "recalled", weight=0.5)

neighbors = db.memory.neighbors("s1", max_depth=1)
print("\nThoughts recalled by session s1 (sorted by weight):")
for n in sorted(neighbors, key=lambda x: x.weight, reverse=True):
    print(f"  id={n.node.id}  kind={n.node.kind}  depth={n.depth}  weight={n.weight:.2f}")

# ── 4. Statistics ─────────────────────────────────────────────────────
stats = db.stats()
print(
    f"\nDB stats: collections={stats.collections} "
    f"vectors={stats.vectors} "
    f"nodes={stats.nodes} "
    f"edges={stats.edges}"
)
