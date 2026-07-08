"""Tests for AgentDBVectorStore.

These tests use pre-built deterministic embeddings — no real model or network
call is required.  A temporary AgentDB database is created for each test and
cleaned up automatically via pytest's ``tmp_path`` fixture.
"""

from __future__ import annotations

import math
import random
import uuid
from typing import List, Optional

import pytest


# ---------------------------------------------------------------------------
# Dimension constant
# ---------------------------------------------------------------------------

DIM = 8  # small dimension keeps tests fast


# ---------------------------------------------------------------------------
# Vector helpers
# ---------------------------------------------------------------------------

def make_random_vector(dim: int = DIM, seed: Optional[int] = None) -> List[float]:
    """Return a unit-normalised random vector of the given dimension."""
    try:
        import numpy as np

        rng = np.random.RandomState(seed) if seed is not None else np.random
        v = rng.randn(dim).astype(float)
        norm = float(np.linalg.norm(v)) or 1.0
        return (v / norm).tolist()
    except ImportError:
        rng = random.Random(seed) if seed is not None else random
        v = [rng.gauss(0, 1) for _ in range(dim)]
        norm = math.sqrt(sum(x * x for x in v)) or 1.0
        return [x / norm for x in v]


# ---------------------------------------------------------------------------
# Import helpers — skip gracefully when agentdb or llama-index aren't installed
# ---------------------------------------------------------------------------

def _import_vector_store():
    """Return ``AgentDBVectorStore`` or skip the test if deps are absent."""
    try:
        from llamaindex_agentdb.vector_store import AgentDBVectorStore
        return AgentDBVectorStore
    except ImportError as exc:
        pytest.skip(f"Required dependency not available: {exc}")


def _make_text_node(text: str, embedding: List[float], node_id: Optional[str] = None):
    """Build a ``TextNode`` with an embedding attached."""
    try:
        from llama_index.core.schema import TextNode
    except ImportError as exc:
        pytest.skip(f"llama-index-core not available: {exc}")

    node = TextNode(
        id_=node_id or str(uuid.uuid4()),
        text=text,
        embedding=embedding,
    )
    return node


def _make_query(embedding: List[float], top_k: int = 4):
    """Build a ``VectorStoreQuery``."""
    try:
        from llama_index.core.vector_stores.types import VectorStoreQuery
    except ImportError as exc:
        pytest.skip(f"llama-index-core not available: {exc}")

    return VectorStoreQuery(query_embedding=embedding, similarity_top_k=top_k)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture()
def tmp_db(tmp_path):
    """Temporary AgentDB file path, unique per test."""
    return str(tmp_path / f"test_{uuid.uuid4().hex[:8]}.agentdb")


@pytest.fixture()
def store(tmp_db):
    """An empty ``AgentDBVectorStore`` connected to a fresh database."""
    AgentDBVectorStore = _import_vector_store()
    return AgentDBVectorStore(
        db_path=tmp_db,
        collection_name="test_collection",
        dimension=DIM,
    )


# ---------------------------------------------------------------------------
# Initialisation
# ---------------------------------------------------------------------------

class TestInit:
    def test_repr(self, store, tmp_db):
        assert "AgentDBVectorStore" in repr(store)
        assert "test_collection" in repr(store)

    def test_class_name(self, store):
        assert store.class_name() == "AgentDBVectorStore"

    def test_stores_text_flag(self, store):
        assert store.stores_text is True

    def test_client_property(self, store):
        # client should return the underlying AgentDB instance (non-None)
        assert store.client is not None


# ---------------------------------------------------------------------------
# add()
# ---------------------------------------------------------------------------

class TestAdd:
    def test_add_single_node_returns_id(self, store):
        node = _make_text_node("Hello AgentDB", make_random_vector(), node_id="n1")
        ids = store.add([node])
        assert ids == ["n1"]

    def test_add_multiple_nodes(self, store):
        nodes = [
            _make_text_node(f"Document {i}", make_random_vector(seed=i), node_id=f"doc-{i}")
            for i in range(5)
        ]
        ids = store.add(nodes)
        assert len(ids) == 5
        assert ids == [f"doc-{i}" for i in range(5)]

    def test_add_empty_list(self, store):
        ids = store.add([])
        assert ids == []

    def test_add_auto_generates_id_when_missing(self, store):
        """Nodes without an explicit ID get a UUID assigned."""
        try:
            from llama_index.core.schema import TextNode
        except ImportError as exc:
            pytest.skip(str(exc))

        node = TextNode(text="auto-id node", embedding=make_random_vector())
        # node_id is auto-assigned by LlamaIndex; it should be a non-empty string
        ids = store.add([node])
        assert len(ids) == 1
        assert isinstance(ids[0], str) and ids[0]

    def test_add_node_without_embedding_raises(self, store):
        try:
            from llama_index.core.schema import TextNode
        except ImportError as exc:
            pytest.skip(str(exc))

        node = TextNode(id_="no-embed", text="no embedding")
        with pytest.raises((ValueError, Exception)):
            store.add([node])

    def test_len_increases_after_add(self, store):
        assert len(store) == 0
        for i in range(3):
            node = _make_text_node(f"node {i}", make_random_vector(seed=i), node_id=f"len-{i}")
            store.add([node])
        assert len(store) == 3


# ---------------------------------------------------------------------------
# query()
# ---------------------------------------------------------------------------

class TestQuery:
    def test_query_returns_result_object(self, store):
        try:
            from llama_index.core.vector_stores.types import VectorStoreQueryResult
        except ImportError as exc:
            pytest.skip(str(exc))

        node = _make_text_node("searchable text", make_random_vector(seed=42), node_id="q1")
        store.add([node])

        query = _make_query(make_random_vector(seed=99), top_k=1)
        result = store.query(query)

        assert isinstance(result, VectorStoreQueryResult)
        assert isinstance(result.nodes, list)
        assert isinstance(result.similarities, list)
        assert isinstance(result.ids, list)

    def test_query_aligned_lengths(self, store):
        nodes = [
            _make_text_node(f"text {i}", make_random_vector(seed=i), node_id=f"align-{i}")
            for i in range(4)
        ]
        store.add(nodes)

        query = _make_query(make_random_vector(seed=7), top_k=3)
        result = store.query(query)

        assert len(result.nodes) == len(result.similarities) == len(result.ids)

    def test_query_top_k_limits_results(self, store):
        for i in range(10):
            node = _make_text_node(f"item {i}", make_random_vector(seed=i), node_id=f"topk-{i}")
            store.add([node])

        query = _make_query(make_random_vector(seed=1), top_k=3)
        result = store.query(query)
        assert len(result.nodes) <= 3

    def test_query_reconstructs_text(self, store):
        text = "The quick brown fox"
        vec = make_random_vector(seed=55)
        store.add([_make_text_node(text, vec, node_id="fox")])

        # Search with the same vector — cosine similarity should be 1.0 (top hit)
        query = _make_query(vec, top_k=1)
        result = store.query(query)

        assert len(result.nodes) == 1
        assert result.nodes[0].get_content() == text

    def test_query_preserves_metadata(self, store):
        try:
            from llama_index.core.schema import TextNode
        except ImportError as exc:
            pytest.skip(str(exc))

        node = TextNode(
            id_="meta-node",
            text="node with metadata",
            embedding=make_random_vector(seed=10),
            metadata={"author": "alice", "year": 2024},
        )
        store.add([node])

        query = _make_query(make_random_vector(seed=10), top_k=1)
        result = store.query(query)

        assert len(result.nodes) == 1
        meta = result.nodes[0].metadata
        assert meta.get("author") == "alice"
        assert meta.get("year") == 2024

    def test_query_similarities_are_floats(self, store):
        node = _make_text_node("float check", make_random_vector(seed=20), node_id="float1")
        store.add([node])

        query = _make_query(make_random_vector(seed=21), top_k=1)
        result = store.query(query)

        for score in result.similarities:
            assert isinstance(score, float)

    def test_query_without_embedding_raises(self, store):
        try:
            from llama_index.core.vector_stores.types import VectorStoreQuery
        except ImportError as exc:
            pytest.skip(str(exc))

        query = VectorStoreQuery(similarity_top_k=4)  # no embedding
        with pytest.raises(ValueError, match="query_embedding"):
            store.query(query)


# ---------------------------------------------------------------------------
# delete()
# ---------------------------------------------------------------------------

class TestDelete:
    def test_delete_by_node_id(self, store):
        node = _make_text_node("to be deleted", make_random_vector(seed=30), node_id="del-1")
        store.add([node])
        assert len(store) == 1

        store.delete("irrelevant-doc-id", node_id="del-1")
        assert len(store) == 0

    def test_delete_nonexistent_does_not_raise(self, store):
        # Deleting a node that was never inserted should not raise.
        store.delete("no-such-doc", node_id="no-such-node")

    def test_delete_by_ref_doc_id(self, store):
        try:
            from llama_index.core.schema import TextNode, RelatedNodeInfo, NodeRelationship
        except ImportError as exc:
            pytest.skip(str(exc))

        doc_id = "parent-doc-1"
        nodes = []
        for i in range(3):
            n = TextNode(
                id_=f"chunk-{i}",
                text=f"chunk {i}",
                embedding=make_random_vector(seed=100 + i),
            )
            # Set the ref_doc_id to simulate nodes belonging to the same document
            n.relationships[NodeRelationship.SOURCE] = RelatedNodeInfo(node_id=doc_id)
            nodes.append(n)

        store.add(nodes)
        assert len(store) == 3

        store.delete(doc_id)
        # All three chunks referencing doc_id should be removed
        assert len(store) == 0

    def test_add_query_delete_cycle(self, store):
        vec = make_random_vector(seed=200)
        node = _make_text_node("cycle test", vec, node_id="cycle-1")
        store.add([node])

        query = _make_query(vec, top_k=1)
        result = store.query(query)
        assert len(result.nodes) == 1

        store.delete("any-doc", node_id="cycle-1")

        result_after = store.query(query)
        assert len(result_after.nodes) == 0
