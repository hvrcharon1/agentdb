"""AgentDB-backed LangChain VectorStore implementation."""

from __future__ import annotations

import uuid
from typing import Any, Callable, Iterable, List, Optional, Sequence, Tuple, Type

from langchain_core.documents import Document
from langchain_core.embeddings import Embeddings
from langchain_core.vectorstores import VectorStore


class AgentDBVectorStore(VectorStore):
    """A LangChain ``VectorStore`` backed by AgentDB.

    AgentDB stores vectors in a named collection inside a single SQLite-based
    database file. Each document is stored as a vector entry with the original
    text and any caller-supplied metadata serialised into the entry's metadata
    field (under the ``"text"`` key so it can be recovered on retrieval).

    Args:
        db_path: Path to the AgentDB database file (created if absent).
        collection_name: Name of the vector collection inside the database.
        dimension: Dimensionality of the embedding vectors.
        embedding: A LangChain ``Embeddings`` instance used to embed texts and
            query strings.  Must produce vectors whose length equals
            ``dimension``.

    Example::

        from langchain_agentdb import AgentDBVectorStore
        from langchain_openai import OpenAIEmbeddings

        store = AgentDBVectorStore(
            db_path="agent.agentdb",
            collection_name="docs",
            dimension=1536,
            embedding=OpenAIEmbeddings(),
        )
        store.add_texts(["Hello world", "AgentDB is fast"])
        results = store.similarity_search("fast database", k=3)
    """

    def __init__(
        self,
        db_path: str,
        collection_name: str,
        dimension: int,
        embedding: Embeddings,
    ) -> None:
        # Import here so the package can be imported even when the Rust
        # extension is not yet compiled (useful for type-checking tooling).
        try:
            import agentdb as _agentdb
        except ImportError as exc:  # pragma: no cover
            raise ImportError(
                "The 'datacules-agentdb' package is required.  "
                "Install it with: pip install datacules-agentdb"
            ) from exc

        self._db_path = db_path
        self._collection_name = collection_name
        self._dimension = dimension
        self._embedding = embedding

        self._db = _agentdb.AgentDB.open(db_path)
        self._collection = self._db.collection(collection_name, dimension)

    # ------------------------------------------------------------------
    # Core write operations
    # ------------------------------------------------------------------

    def add_texts(
        self,
        texts: Iterable[str],
        metadatas: Optional[List[dict]] = None,
        ids: Optional[List[str]] = None,
        **kwargs: Any,
    ) -> List[str]:
        """Embed ``texts`` and store them in the collection.

        Args:
            texts: Iterable of strings to embed and store.
            metadatas: Optional list of metadata dicts, one per text.
            ids: Optional list of string IDs.  Auto-generated (UUID4) when
                omitted.

        Returns:
            List of IDs for the stored vectors.
        """
        text_list = list(texts)
        if not text_list:
            return []

        if ids is None:
            ids = [str(uuid.uuid4()) for _ in text_list]

        if metadatas is None:
            metadatas = [{} for _ in text_list]
        elif len(metadatas) != len(text_list):
            raise ValueError(
                f"metadatas length ({len(metadatas)}) must match texts length "
                f"({len(text_list)})"
            )

        vectors = self._embedding.embed_documents(text_list)

        entries = []
        for doc_id, text, vector, meta in zip(ids, text_list, vectors, metadatas):
            # Merge the original text into the stored metadata so we can
            # reconstruct Document objects on retrieval.
            stored_meta = {"text": text, **meta}
            entries.append(
                {
                    "id": doc_id,
                    "vector": [float(v) for v in vector],
                    "metadata": stored_meta,
                }
            )

        self._collection.upsert_batch(entries)
        return ids

    def add_documents(
        self,
        documents: List[Document],
        ids: Optional[List[str]] = None,
        **kwargs: Any,
    ) -> List[str]:
        """Embed and store ``Document`` objects.

        Args:
            documents: List of ``Document`` instances.
            ids: Optional list of string IDs.

        Returns:
            List of IDs for the stored documents.
        """
        texts = [doc.page_content for doc in documents]
        metadatas = [doc.metadata for doc in documents]
        return self.add_texts(texts, metadatas=metadatas, ids=ids, **kwargs)

    # ------------------------------------------------------------------
    # Similarity search
    # ------------------------------------------------------------------

    def similarity_search(
        self,
        query: str,
        k: int = 4,
        filter: Optional[dict] = None,
        **kwargs: Any,
    ) -> List[Document]:
        """Return the ``k`` most similar documents to ``query``.

        Args:
            query: The natural-language query string.
            k: Number of results to return.
            filter: Optional metadata filter dict passed to AgentDB.

        Returns:
            List of ``Document`` objects sorted by relevance (most similar
            first).
        """
        query_vector = self._embedding.embed_query(query)
        return self.similarity_search_by_vector(query_vector, k=k, filter=filter, **kwargs)

    def similarity_search_by_vector(
        self,
        embedding: List[float],
        k: int = 4,
        filter: Optional[dict] = None,
        **kwargs: Any,
    ) -> List[Document]:
        """Return the ``k`` most similar documents to the given embedding vector.

        Args:
            embedding: A pre-computed query embedding.
            k: Number of results to return.
            filter: Optional metadata filter dict passed to AgentDB.

        Returns:
            List of ``Document`` objects sorted by relevance (most similar
            first).
        """
        results = self._collection.search(
            [float(v) for v in embedding],
            top_k=k,
            filter=filter,
        )
        documents: List[Document] = []
        for result in results:
            meta = result.metadata or {}
            # Extract the stored text; fall back gracefully if absent.
            text = meta.pop("text", "")
            # Attach the similarity score so callers can inspect it if desired.
            meta["_score"] = result.score
            meta["_id"] = result.id
            documents.append(Document(page_content=text, metadata=meta))
        return documents

    def similarity_search_with_score(
        self,
        query: str,
        k: int = 4,
        filter: Optional[dict] = None,
        **kwargs: Any,
    ) -> List[Tuple[Document, float]]:
        """Return documents together with their similarity scores.

        Args:
            query: The natural-language query string.
            k: Number of results to return.
            filter: Optional metadata filter dict.

        Returns:
            List of ``(Document, score)`` tuples sorted by score descending.
        """
        query_vector = self._embedding.embed_query(query)
        results = self._collection.search(
            [float(v) for v in query_vector],
            top_k=k,
            filter=filter,
        )
        docs_and_scores: List[Tuple[Document, float]] = []
        for result in results:
            meta = result.metadata or {}
            text = meta.pop("text", "")
            meta["_id"] = result.id
            docs_and_scores.append(
                (Document(page_content=text, metadata=meta), result.score)
            )
        return docs_and_scores

    # ------------------------------------------------------------------
    # Classmethods required by VectorStore ABC
    # ------------------------------------------------------------------

    @classmethod
    def from_texts(
        cls: Type["AgentDBVectorStore"],
        texts: List[str],
        embedding: Embeddings,
        metadatas: Optional[List[dict]] = None,
        db_path: str = "agentdb.db",
        collection_name: str = "default",
        dimension: Optional[int] = None,
        ids: Optional[List[str]] = None,
        **kwargs: Any,
    ) -> "AgentDBVectorStore":
        """Create an ``AgentDBVectorStore`` from a list of texts.

        The ``dimension`` parameter is inferred automatically by embedding the
        first text when not provided.

        Args:
            texts: Texts to embed and store.
            embedding: Embeddings instance.
            metadatas: Optional metadata list.
            db_path: Path to the database file.
            collection_name: Collection name inside the database.
            dimension: Embedding dimension (inferred from a probe embed when
                ``None``).
            ids: Optional list of document IDs.

        Returns:
            An initialised ``AgentDBVectorStore`` populated with the given
            texts.
        """
        if dimension is None:
            if not texts:
                raise ValueError(
                    "Cannot infer embedding dimension from an empty texts list. "
                    "Pass dimension= explicitly."
                )
            probe = embedding.embed_query(texts[0])
            dimension = len(probe)

        store = cls(
            db_path=db_path,
            collection_name=collection_name,
            dimension=dimension,
            embedding=embedding,
        )
        store.add_texts(texts, metadatas=metadatas, ids=ids)
        return store

    @classmethod
    def from_documents(
        cls: Type["AgentDBVectorStore"],
        documents: List[Document],
        embedding: Embeddings,
        db_path: str = "agentdb.db",
        collection_name: str = "default",
        dimension: Optional[int] = None,
        ids: Optional[List[str]] = None,
        **kwargs: Any,
    ) -> "AgentDBVectorStore":
        """Create an ``AgentDBVectorStore`` from ``Document`` objects.

        Args:
            documents: Documents to embed and store.
            embedding: Embeddings instance.
            db_path: Path to the database file.
            collection_name: Collection name inside the database.
            dimension: Embedding dimension (inferred when ``None``).
            ids: Optional list of document IDs.

        Returns:
            An initialised ``AgentDBVectorStore`` populated with the documents.
        """
        texts = [doc.page_content for doc in documents]
        metadatas = [doc.metadata for doc in documents]
        return cls.from_texts(
            texts=texts,
            embedding=embedding,
            metadatas=metadatas,
            db_path=db_path,
            collection_name=collection_name,
            dimension=dimension,
            ids=ids,
            **kwargs,
        )

    # ------------------------------------------------------------------
    # Utility
    # ------------------------------------------------------------------

    @property
    def embeddings(self) -> Embeddings:
        """The ``Embeddings`` instance used by this store."""
        return self._embedding

    def delete(self, ids: Optional[List[str]] = None, **kwargs: Any) -> Optional[bool]:
        """Delete documents from the collection by ID.

        Args:
            ids: List of document IDs to delete.

        Returns:
            ``True`` if any deletions were requested, ``None`` otherwise.
        """
        if not ids:
            return None
        for doc_id in ids:
            self._collection.delete(doc_id)
        return True

    def __len__(self) -> int:
        return self._collection.count()

    def __repr__(self) -> str:
        return (
            f"AgentDBVectorStore("
            f"db_path={self._db_path!r}, "
            f"collection={self._collection_name!r}, "
            f"dim={self._dimension})"
        )
