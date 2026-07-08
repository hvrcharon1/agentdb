"""pytest test suite for the AgentDB Python bindings (PyO3/maturin).

Run with:
    pytest python/tests/
or, from inside the python/ directory:
    pytest

The maturin-built wheel (or an editable install via `maturin develop`) must be
installed in the active Python environment before running these tests.
"""

import os
import tempfile

import pytest
import agentdb
from agentdb import AgentDB, Collection, SearchResult, FtsResult


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def db():
    """In-memory AgentDB instance, fresh for each test."""
    return AgentDB.open(":memory:")


@pytest.fixture
def tmp_db(tmp_path):
    """File-backed AgentDB instance using pytest's tmp_path fixture."""
    path = str(tmp_path / "test.db")
    return AgentDB.open(path)


# ---------------------------------------------------------------------------
# 1. AgentDB.open()
# ---------------------------------------------------------------------------

class TestOpen:
    def test_open_memory(self):
        db = AgentDB.open(":memory:")
        assert db is not None
        assert isinstance(db, AgentDB)

    def test_open_file(self, tmp_path):
        path = str(tmp_path / "agent.db")
        db = AgentDB.open(path)
        assert db is not None
        assert isinstance(db, AgentDB)
        assert os.path.exists(path)

    def test_open_file_persists(self, tmp_path):
        """Data written to a file DB should survive re-open."""
        path = str(tmp_path / "persist.db")
        db1 = AgentDB.open(path)
        db1.execute("CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT)")
        db1.execute("INSERT INTO kv VALUES ('hello', 'world')")

        db2 = AgentDB.open(path)
        rows = db2.query("SELECT v FROM kv WHERE k='hello'")
        assert len(rows) == 1
        assert rows[0]["v"] == "world"


# ---------------------------------------------------------------------------
# 2. db.execute() and db.query()
# ---------------------------------------------------------------------------

class TestSQL:
    def test_execute_create_and_insert(self, db):
        rows_affected = db.execute(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)"
        )
        # execute() returns the number of rows changed; DDL returns 0
        assert isinstance(rows_affected, int)

        db.execute("INSERT INTO items VALUES (1, 'alpha')")
        db.execute("INSERT INTO items VALUES (2, 'beta')")

    def test_query_returns_list_of_dicts(self, db):
        db.execute("CREATE TABLE t (x INTEGER, y TEXT)")
        db.execute("INSERT INTO t VALUES (42, 'hello')")
        db.execute("INSERT INTO t VALUES (7,  'world')")

        rows = db.query("SELECT x, y FROM t ORDER BY x")
        assert isinstance(rows, list)
        assert len(rows) == 2
        assert rows[0]["x"] == 7
        assert rows[1]["x"] == 42

    def test_query_empty_table(self, db):
        db.execute("CREATE TABLE empty_t (id INTEGER)")
        rows = db.query("SELECT * FROM empty_t")
        assert rows == []

    def test_query_aggregation(self, db):
        db.execute("CREATE TABLE nums (n INTEGER)")
        for i in range(1, 6):
            db.execute(f"INSERT INTO nums VALUES ({i})")
        rows = db.query("SELECT SUM(n) AS total FROM nums")
        assert rows[0]["total"] == 15


# ---------------------------------------------------------------------------
# 3. db.collection() — creating a vector collection
# ---------------------------------------------------------------------------

class TestCollection:
    def test_create_collection(self, db):
        col = db.collection("embeddings", 4)
        assert isinstance(col, Collection)

    def test_create_collection_different_dims(self, db):
        col8 = db.collection("eight", 8)
        col16 = db.collection("sixteen", 16)
        assert isinstance(col8, Collection)
        assert isinstance(col16, Collection)

    def test_collection_count_empty(self, db):
        col = db.collection("empty_col", 3)
        assert col.count() == 0

    def test_reopen_collection_same_name(self, db):
        """Calling collection() twice with the same name should be idempotent."""
        col_a = db.collection("shared", 4)
        col_b = db.collection("shared", 4)
        col_a.upsert("x1", [1.0, 0.0, 0.0, 0.0])
        assert col_b.count() == 1


# ---------------------------------------------------------------------------
# 4. collection.upsert() and collection.search()
# ---------------------------------------------------------------------------

class TestVectorSearch:
    def _make_collection(self, db):
        col = db.collection("vecs", 4)
        col.upsert("a", [1.0, 0.0, 0.0, 0.0], metadata={"label": "A"})
        col.upsert("b", [0.0, 1.0, 0.0, 0.0], metadata={"label": "B"})
        col.upsert("c", [0.0, 0.0, 1.0, 0.0], metadata={"label": "C"})
        col.upsert("d", [0.0, 0.0, 0.0, 1.0], metadata={"label": "D"})
        return col

    def test_upsert_increases_count(self, db):
        col = self._make_collection(db)
        assert col.count() == 4

    def test_upsert_overwrite(self, db):
        col = db.collection("vecs", 4)
        col.upsert("x", [1.0, 0.0, 0.0, 0.0])
        col.upsert("x", [0.0, 1.0, 0.0, 0.0])  # overwrite
        assert col.count() == 1

    def test_search_returns_search_results(self, db):
        col = self._make_collection(db)
        results = col.search([1.0, 0.0, 0.0, 0.0], top_k=2)
        assert isinstance(results, list)
        assert len(results) == 2
        assert all(isinstance(r, SearchResult) for r in results)

    def test_search_top_result_is_nearest(self, db):
        col = self._make_collection(db)
        results = col.search([1.0, 0.0, 0.0, 0.0], top_k=4)
        assert results[0].id == "a"

    def test_search_result_fields(self, db):
        col = self._make_collection(db)
        results = col.search([1.0, 0.0, 0.0, 0.0], top_k=1)
        r = results[0]
        assert hasattr(r, "id")
        assert hasattr(r, "score")
        assert hasattr(r, "metadata")
        assert isinstance(r.score, float)
        assert r.metadata["label"] == "A"

    def test_search_top_k_limits_results(self, db):
        col = self._make_collection(db)
        results = col.search([1.0, 0.0, 0.0, 0.0], top_k=2)
        assert len(results) <= 2

    def test_search_no_metadata(self, db):
        col = db.collection("plain", 2)
        col.upsert("p1", [1.0, 0.0])
        results = col.search([1.0, 0.0], top_k=1)
        assert results[0].id == "p1"

    def test_delete_vector(self, db):
        col = db.collection("del_test", 2)
        col.upsert("z1", [1.0, 0.0])
        col.upsert("z2", [0.0, 1.0])
        assert col.count() == 2
        col.delete("z1")
        assert col.count() == 1

    def test_upsert_batch(self, db):
        col = db.collection("batch", 3)
        entries = [
            {"id": "b1", "vector": [1.0, 0.0, 0.0]},
            {"id": "b2", "vector": [0.0, 1.0, 0.0], "metadata": {"tag": "second"}},
            {"id": "b3", "vector": [0.0, 0.0, 1.0]},
        ]
        inserted = col.upsert_batch(entries)
        assert inserted == 3
        assert col.count() == 3


# ---------------------------------------------------------------------------
# 5. db.conversations() — create conversation, add message, get messages
# ---------------------------------------------------------------------------

class TestConversations:
    def test_create_conversation(self, db):
        db.create_conversation("conv1", title="Test chat")

    def test_list_conversations(self, db):
        db.create_conversation("conv1", title="First")
        db.create_conversation("conv2", title="Second")
        convos = db.list_conversations()
        assert isinstance(convos, list)
        assert len(convos) == 2
        ids = {c["id"] for c in convos}
        assert "conv1" in ids
        assert "conv2" in ids

    def test_add_message_returns_id(self, db):
        db.create_conversation("conv1")
        msg_id = db.add_message("conv1", "user", "Hello!")
        assert isinstance(msg_id, str)
        assert len(msg_id) > 0

    def test_get_messages_returns_list(self, db):
        db.create_conversation("conv1")
        db.add_message("conv1", "user", "Hello!")
        db.add_message("conv1", "assistant", "Hi there!")
        msgs = db.get_messages("conv1")
        assert isinstance(msgs, list)
        assert len(msgs) == 2

    def test_get_messages_fields(self, db):
        db.create_conversation("conv1")
        db.add_message("conv1", "user", "Test message", metadata={"token_count": 3})
        msgs = db.get_messages("conv1")
        m = msgs[0]
        assert m["role"] == "user"
        assert m["content"] == "Test message"
        assert m["conversation_id"] == "conv1"
        assert "id" in m
        assert "created_at" in m

    def test_get_messages_with_limit(self, db):
        db.create_conversation("conv1")
        for i in range(5):
            db.add_message("conv1", "user", f"Message {i}")
        msgs = db.get_messages("conv1", limit=3)
        assert len(msgs) <= 3

    def test_delete_conversation(self, db):
        db.create_conversation("conv_del")
        db.delete_conversation("conv_del")
        convos = db.list_conversations()
        assert all(c["id"] != "conv_del" for c in convos)

    def test_conversation_with_metadata(self, db):
        meta = {"agent": "assistant-v2", "session": "abc"}
        db.create_conversation("conv_meta", title="Meta chat", metadata=meta)
        convos = db.list_conversations()
        conv = next(c for c in convos if c["id"] == "conv_meta")
        assert conv["title"] == "Meta chat"

    def test_search_messages(self, db):
        db.create_conversation("conv_fts")
        db.add_message("conv_fts", "user", "The quick brown fox")
        db.add_message("conv_fts", "assistant", "Jumped over the lazy dog")
        results = db.search_messages("fox", top_k=5)
        assert isinstance(results, list)


# ---------------------------------------------------------------------------
# 6. db.workflows() — create workflow, add step, complete workflow
# ---------------------------------------------------------------------------

class TestWorkflows:
    def test_create_workflow(self, db):
        db.create_workflow("wf1", "data-pipeline")

    def test_list_workflows(self, db):
        db.create_workflow("wf1", "pipeline-a")
        db.create_workflow("wf2", "pipeline-b")
        workflows = db.list_workflows()
        assert isinstance(workflows, list)
        assert len(workflows) == 2

    def test_add_workflow_step_returns_id(self, db):
        db.create_workflow("wf1", "my-workflow")
        step_id = db.add_workflow_step("wf1", "fetch-data")
        assert isinstance(step_id, str)
        assert len(step_id) > 0

    def test_get_workflow_structure(self, db):
        db.create_workflow("wf1", "multi-step", input={"source": "s3://bucket"})
        step_id = db.add_workflow_step("wf1", "fetch", input={"url": "http://example.com"})
        db.add_workflow_step("wf1", "transform")

        wf = db.get_workflow("wf1")
        assert wf["id"] == "wf1"
        assert wf["name"] == "multi-step"
        assert wf["status"] == "running"
        assert isinstance(wf["steps"], list)
        assert len(wf["steps"]) == 2

    def test_update_workflow_step(self, db):
        db.create_workflow("wf1", "test")
        step_id = db.add_workflow_step("wf1", "step-one")
        db.update_workflow_step(step_id, "completed", output={"result": 42})
        wf = db.get_workflow("wf1")
        step = next(s for s in wf["steps"] if s["id"] == step_id)
        assert step["status"] == "completed"

    def test_complete_workflow(self, db):
        db.create_workflow("wf1", "finishing-wf")
        db.complete_workflow("wf1", output={"summary": "done"})
        wf = db.get_workflow("wf1")
        assert wf["status"] == "completed"

    def test_fail_workflow(self, db):
        db.create_workflow("wf_fail", "failing-wf")
        db.fail_workflow("wf_fail", error="out of memory")
        wf = db.get_workflow("wf_fail")
        assert wf["status"] == "failed"

    def test_list_workflows_by_status(self, db):
        db.create_workflow("wf_run", "runner")
        db.create_workflow("wf_done", "finisher")
        db.complete_workflow("wf_done")
        running = db.list_workflows(status="running")
        completed = db.list_workflows(status="completed")
        assert any(w["id"] == "wf_run" for w in running)
        assert any(w["id"] == "wf_done" for w in completed)


# ---------------------------------------------------------------------------
# 7. db.traces() — add trace, get traces
# ---------------------------------------------------------------------------

class TestTraces:
    def test_add_trace_returns_id(self, db):
        trace_id = db.add_trace("llm_call", "Sent prompt to model", session_id="sess1")
        assert isinstance(trace_id, str)
        assert len(trace_id) > 0

    def test_get_traces_returns_list(self, db):
        db.add_trace("llm_call", "First call", session_id="sess1")
        db.add_trace("tool_call", "Used search tool", session_id="sess1")
        traces = db.get_traces("sess1")
        assert isinstance(traces, list)
        assert len(traces) == 2

    def test_trace_fields(self, db):
        db.add_trace("llm_call", "Hello from model", session_id="sess1")
        traces = db.get_traces("sess1")
        t = traces[0]
        assert t["trace_type"] == "llm_call"
        assert t["content"] == "Hello from model"
        assert t["session_id"] == "sess1"
        assert "id" in t
        assert "created_at" in t

    def test_trace_with_metadata(self, db):
        db.add_trace(
            "llm_call",
            "Response text",
            session_id="sess2",
            metadata={"model": "claude-3", "tokens": 150},
        )
        traces = db.get_traces("sess2")
        assert len(traces) == 1

    def test_trace_parent_child(self, db):
        parent_id = db.add_trace("session", "Root trace", session_id="sess3")
        child_id = db.add_trace(
            "llm_call", "Child call", session_id="sess3", parent_id=parent_id
        )
        traces = db.get_traces("sess3")
        child = next(t for t in traces if t["id"] == child_id)
        assert child["parent_id"] == parent_id

    def test_get_traces_isolated_by_session(self, db):
        db.add_trace("event", "Session A trace", session_id="sessA")
        db.add_trace("event", "Session B trace", session_id="sessB")
        traces_a = db.get_traces("sessA")
        traces_b = db.get_traces("sessB")
        assert len(traces_a) == 1
        assert len(traces_b) == 1

    def test_get_trace_tree(self, db):
        root_id = db.add_trace("session", "Root", session_id="tree_sess")
        db.add_trace("llm_call", "Child 1", session_id="tree_sess", parent_id=root_id)
        db.add_trace("tool_call", "Child 2", session_id="tree_sess", parent_id=root_id)
        tree = db.get_trace_tree(root_id)
        assert isinstance(tree, list)
        assert len(tree) >= 1


# ---------------------------------------------------------------------------
# 8. db.memory() — add node, add edge, neighbors
# ---------------------------------------------------------------------------

class TestMemoryGraph:
    def test_add_node(self, db):
        db.add_node("n1", "concept")

    def test_add_node_with_data(self, db):
        db.add_node("n1", "concept", data={"description": "machine learning"})

    def test_add_edge(self, db):
        db.add_node("n1", "concept")
        db.add_node("n2", "concept")
        db.add_edge("n1", "n2", "related_to", weight=0.8)

    def test_neighbors_returns_list(self, db):
        db.add_node("root", "session")
        db.add_node("c1", "concept")
        db.add_node("c2", "concept")
        db.add_edge("root", "c1", "has", weight=0.9)
        db.add_edge("root", "c2", "has", weight=0.5)

        neighbors = db.neighbors("root", max_depth=1)
        assert isinstance(neighbors, list)
        assert len(neighbors) == 2

    def test_neighbors_fields(self, db):
        db.add_node("hub", "session")
        db.add_node("leaf", "concept")
        db.add_edge("hub", "leaf", "points_to", weight=0.7)

        neighbors = db.neighbors("hub", max_depth=1)
        assert len(neighbors) == 1
        n = neighbors[0]
        assert "id" in n
        assert "kind" in n
        assert "depth" in n
        assert "weight" in n
        assert n["id"] == "leaf"
        assert n["kind"] == "concept"
        assert n["depth"] == 1

    def test_neighbors_max_depth(self, db):
        # chain: root -> mid -> leaf
        db.add_node("root", "a")
        db.add_node("mid", "b")
        db.add_node("leaf", "c")
        db.add_edge("root", "mid", "link", weight=1.0)
        db.add_edge("mid", "leaf", "link", weight=1.0)

        shallow = db.neighbors("root", max_depth=1)
        deep = db.neighbors("root", max_depth=2)
        assert len(shallow) == 1  # only "mid"
        assert len(deep) == 2     # "mid" and "leaf"

    def test_neighbors_min_weight_filter(self, db):
        db.add_node("hub", "session")
        db.add_node("strong", "concept")
        db.add_node("weak", "concept")
        db.add_edge("hub", "strong", "link", weight=0.9)
        db.add_edge("hub", "weak", "link", weight=0.1)

        filtered = db.neighbors("hub", max_depth=1, min_weight=0.5)
        ids = [n["id"] for n in filtered]
        assert "strong" in ids
        assert "weak" not in ids

    def test_stats_counts_nodes_and_edges(self, db):
        db.add_node("n1", "t")
        db.add_node("n2", "t")
        db.add_edge("n1", "n2", "rel", weight=1.0)
        stats = db.stats()
        assert stats["nodes"] == 2
        assert stats["edges"] == 1


# ---------------------------------------------------------------------------
# 9. db.fts() — index text, search text
# ---------------------------------------------------------------------------

class TestFullTextSearch:
    def test_fts_index(self, db):
        db.fts_index("docs", "doc1", "doc1", "The quick brown fox jumps over the lazy dog")

    def test_fts_search_returns_results(self, db):
        db.fts_index("docs", "doc1", "doc1", "The quick brown fox jumps over the lazy dog")
        db.fts_index("docs", "doc2", "doc2", "A cat sat on the mat")
        db.fts_index("docs", "doc3", "doc3", "Machine learning enables intelligent systems")

        results = db.fts_search("docs", "fox", 5)
        assert isinstance(results, list)
        assert len(results) >= 1

    def test_fts_search_result_fields(self, db):
        db.fts_index("docs", "doc1", "doc1", "The quick brown fox")
        results = db.fts_search("docs", "fox", 5)
        r = results[0]
        assert isinstance(r, FtsResult)
        assert hasattr(r, "id")
        assert hasattr(r, "snippet")
        assert hasattr(r, "rank")
        assert r.id == "doc1"
        assert isinstance(r.rank, float)

    def test_fts_search_top_k(self, db):
        for i in range(10):
            db.fts_index("docs", f"doc{i}", f"doc{i}", f"Document number {i} about topics")
        results = db.fts_search("docs", "topics", 3)
        assert len(results) <= 3

    def test_fts_search_no_match(self, db):
        db.fts_index("docs", "doc1", "doc1", "hello world")
        results = db.fts_search("docs", "zxqwerty12345", 5)
        assert isinstance(results, list)
        assert len(results) == 0

    def test_fts_upsert_with_text(self, db):
        """Collection.upsert_with_text should index in FTS automatically."""
        col = db.collection("combined", 3)
        col.upsert_with_text(
            "item1",
            [1.0, 0.0, 0.0],
            "neural network training techniques",
            metadata={"category": "ml"},
        )
        results = db.fts_search("combined", "neural", 5)
        assert isinstance(results, list)
        assert len(results) >= 1


# ---------------------------------------------------------------------------
# 10. db.stats() — verify stat fields exist
# ---------------------------------------------------------------------------

class TestStats:
    EXPECTED_FIELDS = [
        "collections",
        "vectors",
        "nodes",
        "edges",
        "conversations",
        "messages",
        "workflows",
        "workflow_steps",
        "traces",
    ]

    def test_stats_returns_dict(self, db):
        stats = db.stats()
        assert isinstance(stats, dict)

    def test_stats_has_all_fields(self, db):
        stats = db.stats()
        for field in self.EXPECTED_FIELDS:
            assert field in stats, f"Missing field: {field}"

    def test_stats_values_are_numeric(self, db):
        stats = db.stats()
        for field in self.EXPECTED_FIELDS:
            assert isinstance(stats[field], (int, float)), (
                f"Field {field!r} is not numeric: {stats[field]!r}"
            )

    def test_stats_empty_db(self, db):
        stats = db.stats()
        for field in self.EXPECTED_FIELDS:
            assert stats[field] == 0, f"Expected 0 for {field} in empty DB, got {stats[field]}"

    def test_stats_reflect_data(self, db):
        # Add one of each resource type
        col = db.collection("s_col", 2)
        col.upsert("v1", [1.0, 0.0])

        db.add_node("sn1", "concept")
        db.add_node("sn2", "concept")
        db.add_edge("sn1", "sn2", "link", weight=1.0)

        db.create_conversation("sc1")
        db.add_message("sc1", "user", "hello")

        db.create_workflow("swf1", "test-wf")
        db.add_workflow_step("swf1", "step-one")

        db.add_trace("event", "trace content", session_id="ss1")

        stats = db.stats()
        assert stats["collections"] >= 1
        assert stats["vectors"] >= 1
        assert stats["nodes"] >= 2
        assert stats["edges"] >= 1
        assert stats["conversations"] >= 1
        assert stats["messages"] >= 1
        assert stats["workflows"] >= 1
        assert stats["workflow_steps"] >= 1
        assert stats["traces"] >= 1
