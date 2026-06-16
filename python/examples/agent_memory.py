"""
agent_memory.py — AgentDB quick-start example (Python)

Demonstrates:
  • SQL layer         — create table, insert, query
  • Vector store      — upsert embeddings and run ANN search
  • Memory graph      — add nodes, directed edges, and traverse
  • Database stats    — inspect counts across all layers

Run:
    # 1. Build the wheel (one-time)
    pip install maturin
    cd python && maturin develop

    # 2. Run this example
    python python/examples/agent_memory.py
"""

import random
import agentdb


def main() -> None:
    print(f"AgentDB {agentdb.__version__}\n")

    # -------------------------------------------------------------------------
    # 1. Open a database
    #    Use ":memory:" for ephemeral in-process storage (no file created).
    #    Pass a file path for persistent storage: AgentDB.open("agent.db")
    # -------------------------------------------------------------------------
    db = agentdb.AgentDB.open(":memory:")

    # -------------------------------------------------------------------------
    # 2. SQL layer — full SQLite engine available
    # -------------------------------------------------------------------------
    db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY, label TEXT)")
    db.execute("INSERT INTO sessions VALUES ('s1', 'planning session')")
    db.execute("INSERT INTO sessions VALUES ('s2', 'review session')")
    print("SQL layer ready.")

    # -------------------------------------------------------------------------
    # 3. Vector store — HNSW ANN search
    # -------------------------------------------------------------------------
    DIM = 8
    col = db.vectors.collection("thoughts", dim=DIM)

    thoughts = {
        "t1": "agent planning phase",
        "t2": "memory consolidation",
        "t3": "tool selection step",
        "t4": "final review loop",
    }

    random.seed(42)
    for tid, label in thoughts.items():
        # In production these would be real model embeddings, e.g. from
        # openai.embeddings.create() or sentence-transformers.
        vec = [random.gauss(0, 1) for _ in range(DIM)]
        col.upsert(tid, vec, metadata={"label": label})

    print(f"Inserted {col.count()} vectors into 'thoughts'")

    # Nearest-neighbour search — find the 3 most similar thoughts
    query_vec = [random.gauss(0, 1) for _ in range(DIM)]
    results = col.search(query_vec, top_k=3)
    print("\nVector search (top 3):")
    for r in results:
        print(f"  id={r.id}  score={r.score:.4f}  metadata={r.metadata}")

    # -------------------------------------------------------------------------
    # 4. Memory graph — typed nodes + directed weighted edges
    # -------------------------------------------------------------------------
    db.memory.add_node("s1", "session")
    db.memory.add_node("t1", "thought")
    db.memory.add_node("t2", "thought")
    db.memory.add_node("t3", "thought")

    # Edges express semantic relationships between agent memories
    db.memory.add_edge("s1", "t1", "recalled",  weight=0.9)
    db.memory.add_edge("s1", "t2", "recalled",  weight=0.7)
    db.memory.add_edge("t1", "t3", "leads_to",  weight=0.8)

    print("\nMemory graph nodes and edges added.")

    # -------------------------------------------------------------------------
    # 5. Database statistics
    # -------------------------------------------------------------------------
    stats = db.stats()
    print(f"\nDatabase stats: {stats}")
    # Expected: {'collections': 1, 'vectors': 4, 'nodes': 4, 'edges': 3}


if __name__ == "__main__":
    main()
