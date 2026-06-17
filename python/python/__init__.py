"""AgentDB — single-file embedded database for AI agents.

SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs.

Quick start::

    import agentdb
    import numpy as np

    db = agentdb.AgentDB.open(":memory:")

    # SQL
    db.execute("CREATE TABLE sessions (id TEXT PRIMARY KEY)")

    # Vectors
    col = db.vectors.collection("thoughts", dim=4)
    col.upsert("t1", [0.9, 0.1, 0.0, 0.0], metadata={"score": 9})
    results = col.search([0.9, 0.1, 0.0, 0.0], top_k=5)

    # Memory graph
    db.memory.add_node("s1", "session")
    db.memory.add_node("t1", "thought")
    db.memory.add_edge("s1", "t1", "recalled", weight=0.9)

    stats = db.stats()
    print(stats)  # {'collections': 1, 'vectors': 1, 'nodes': 2, 'edges': 1}
"""

from ._agentdb import AgentDB, Collection, SearchResult, FtsResult, HybridResult

__version__ = "0.3.2"
__all__ = ["AgentDB", "Collection", "SearchResult", "FtsResult", "HybridResult"]
