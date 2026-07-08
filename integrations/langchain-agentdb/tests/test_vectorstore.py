"""Tests for AgentDBVectorStore.

These tests use a mock embedding function so no real model or network call is
needed.  A temporary AgentDB database is created for each test and cleaned up
automatically.
"""

from __future__ import annotations

import os
import tempfile
import uuid
from typing import List
from unittest.mock import MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Fixtures and helpers
# ---------------------------------------------------------------------------

DIM = 8  # small dimension keeps tests fast


def make_random_vector(dim: int = DIM) -> List[float]:
    """Return a unit-normalised random vector of the given dimension."""
    try:
        import numpy as np

        v = np.random.randn(dim).astype(float)
        norm = np.linalg.norm(v)
        if norm == 0:
            v = np.ones(dim, dtype=float)
            norm = float(np.linalg.norm(v))
        return (v / norm).tolist()
    except ImportError:
        import math
        import random

        v = [random.gauss(0, 1) for _ in range(dim)]
        norm = math.sqrt(sum(x * x for x in v)) or 1.0
        return [x / norm for x in v]


class FixedEmbeddings:
    """Deterministic mock embeddings that return reproducible unit vectors.

    Each unique text always gets the *same* vector so similarity comparisons
    are predictable in tests.
    """

    def __init__(self, dim: int = DIM) -> None:
        self._dim = dim
        self._cache: dict[str, List[float]] = {}

    def _get_or_create(self, text: str) -> List[float]:
        if text not in self._cache:
            self._cache[text] = make_random_vector(self._dim)
        return self._cache[text]

    def embed_documents(self, texts: List[str]) -> List[List[float]]:
        return [self._get_or_create(t) for t in texts]

    def embed_query(self, text: str) -> List[float]:
        return self._get_or_create(text)


@pytest.fixture()
def tmp_db(tmp_path):
    """Return a path to a temporary AgentDB file."""
    return str(tmp_path / f"test_{uuid.uuid4().hex[:8]}.agentdb")


@pytest.fixture()
def embeddings():
    """Return a ``FixedEmbeddings`` instance."""
    return FixedEmbeddings(dim=DIM)


# ---------------------------------------------------------------------------
# We use pytest's import mechanism so the package source tree is importable
# without being installed.  The tests are expected to be run from the package
# root (integrations/langchain-agentdb/) or via `pytest` with the src layout
# handled by pyproject.toml / conftest path manipulation.
# ---------------------------------------------------------------------------


def _import_store():
    """Import AgentDBVectorStore, skipping if agentdb is unavailable."""
    try:
        from langchain_agentdb.vectorstore import AgentDBVectorStore

        return AgentDBVectorStore
    except ImportError as exc:
        pytest.skip(f"Required dependency not available: {exc}")


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestAgentDBVectorStoreInit:
    def test_init_opens_db(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="test",
            dimension=DIM,
            embedding=embeddings,
        )
        assert repr(store) == (
            f"AgentDBVectorStore(db_path={tmp_db!r}, collection='test', dim={DIM})"
        )

    def test_embeddings_property(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="test",
            dimension=DIM,
            embedding=embeddings,
        )
        assert store.embeddings is embeddings


class TestAddTexts:
    def test_add_texts_returns_ids(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        ids = store.add_texts(["Hello world", "AgentDB is fast"])
        assert len(ids) == 2
        # IDs should be non-empty strings.
        for doc_id in ids:
            assert isinstance(doc_id, str) and doc_id

    def test_add_texts_custom_ids(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        custom_ids = ["id-1", "id-2", "id-3"]
        returned = store.add_texts(
            ["foo", "bar", "baz"],
            ids=custom_ids,
        )
        assert returned == custom_ids

    def test_add_texts_with_metadata(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        metas = [{"source": "web"}, {"source": "book"}]
        ids = store.add_texts(["page 1", "page 2"], metadatas=metas)
        assert len(ids) == 2

    def test_add_texts_empty(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        result = store.add_texts([])
        assert result == []

    def test_add_texts_metadata_length_mismatch_raises(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        with pytest.raises(ValueError, match="metadatas length"):
            store.add_texts(["a", "b"], metadatas=[{"x": 1}])


class TestSimilaritySearch:
    def test_similarity_search_returns_documents(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        texts = ["The cat sat on the mat", "Dogs love to play fetch", "Python is great"]
        store.add_texts(texts)

        results = store.similarity_search("feline animals", k=2)
        assert len(results) <= 2
        for doc in results:
            assert hasattr(doc, "page_content")
            assert hasattr(doc, "metadata")
            assert isinstance(doc.page_content, str)

    def test_similarity_search_page_content_preserved(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        text = "Unique test sentence for content preservation"
        store.add_texts([text], ids=["unique-id"])

        results = store.similarity_search(text, k=1)
        assert len(results) == 1
        assert results[0].page_content == text

    def test_similarity_search_metadata_preserved(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        store.add_texts(
            ["metadata test"],
            metadatas=[{"author": "alice", "year": 2024}],
            ids=["meta-doc"],
        )
        results = store.similarity_search("metadata test", k=1)
        assert len(results) == 1
        assert results[0].metadata.get("author") == "alice"
        assert results[0].metadata.get("year") == 2024

    def test_similarity_search_k_limits_results(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        store.add_texts([f"Document number {i}" for i in range(10)])
        results = store.similarity_search("some query", k=3)
        assert len(results) <= 3

    def test_similarity_search_by_vector(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        store.add_texts(["vector search test"])
        query_vec = make_random_vector(DIM)
        results = store.similarity_search_by_vector(query_vec, k=1)
        assert len(results) == 1

    def test_similarity_search_with_score(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        store.add_texts(["scored search", "another document"])
        results = store.similarity_search_with_score("scored", k=2)
        assert len(results) <= 2
        for doc, score in results:
            assert hasattr(doc, "page_content")
            assert isinstance(score, float)


class TestFromTexts:
    def test_from_texts_classmethod(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore.from_texts(
            texts=["hello", "world"],
            embedding=embeddings,
            db_path=tmp_db,
            collection_name="fromtexts",
            dimension=DIM,
        )
        assert isinstance(store, AgentDBVectorStore)

    def test_from_texts_infers_dimension(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore.from_texts(
            texts=["infer dimension"],
            embedding=embeddings,
            db_path=tmp_db,
            collection_name="inferred",
            # dimension not provided — should be inferred
        )
        assert store._dimension == DIM

    def test_from_texts_empty_raises(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        with pytest.raises(ValueError, match="empty texts list"):
            AgentDBVectorStore.from_texts(
                texts=[],
                embedding=embeddings,
                db_path=tmp_db,
                collection_name="empty",
                # no dimension provided
            )


class TestDelete:
    def test_delete_removes_document(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        store.add_texts(["to be deleted"], ids=["del-id"])
        result = store.delete(["del-id"])
        assert result is True

    def test_delete_none_returns_none(self, tmp_db, embeddings):
        AgentDBVectorStore = _import_store()
        store = AgentDBVectorStore(
            db_path=tmp_db,
            collection_name="docs",
            dimension=DIM,
            embedding=embeddings,
        )
        assert store.delete(None) is None
        assert store.delete([]) is None
