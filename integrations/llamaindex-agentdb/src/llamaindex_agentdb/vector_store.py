"""AgentDB-backed LlamaIndex VectorStore implementation."""

from __future__ import annotations

import json
import uuid
from typing import Any, Dict, List, Optional, cast

from llama_index.core.schema import BaseNode, TextNode
from llama_index.core.vector_stores.types import (
    BasePydanticVectorStore,
    VectorStoreQuery,
    VectorStoreQueryResult,
)


class AgentDBVectorStore(BasePydanticVectorStore):
    """A LlamaIndex ``BasePydanticVectorStore`` backed by AgentDB.

    AgentDB stores vectors in a named collection inside a single SQLite-based
    database file.  Each node's text, metadata, and embedding are persisted
    together so they can be fully reconstructed on retrieval.

    Args:
        db_path: Path to the AgentDB database file (created if absent).
        collection_name: Name of the vector collection inside the database.
        dimension: Dimensionality of the embedding vectors.

    Example::

        from llamaindex_agentdb import AgentDBVectorStore
        from llama_index.core import VectorStoreIndex, StorageContext

        vector_store = AgentDBVectorStore(
            db_path="agent.agentdb",
            collection_name="docs",
            dimension=1536,
        )
        storage_context = StorageContext.from_defaults(vector_store=vector_store)
        index = VectorStoreIndex.from_documents(documents, storage_context=storage_context)
    """

    # Pydantic fields — required by BasePydanticVectorStore
    stores_text: bool = True
    flat_metadata: bool = False

    # Private attributes declared as class-level annotations so Pydantic
    # ignores them and we manage them ourselves via __init__.
    _db_path: str
    _collection_name: str
    _dimension: int
    _db: Any
    _collection: Any

    def __init__(
        self,
        db_path: str,
        collection_name: str,
        dimension: int,
        **kwargs: Any,
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

        super().__init__(**kwargs)

        # Use object.__setattr__ to bypass Pydantic's field assignment guards
        # for private/non-field attributes.
        object.__setattr__(self, "_db_path", db_path)
        object.__setattr__(self, "_collection_name", collection_name)
        object.__setattr__(self, "_dimension", dimension)

        db = _agentdb.AgentDB.open(db_path)
        object.__setattr__(self, "_db", db)
        object.__setattr__(self, "_collection", db.collection(collection_name, dimension))

    # ------------------------------------------------------------------
    # Required BasePydanticVectorStore interface
    # ------------------------------------------------------------------

    @classmethod
    def class_name(cls) -> str:
        return "AgentDBVectorStore"

    @property
    def client(self) -> Any:
        """Return the underlying AgentDB database instance."""
        return self._db

    # ------------------------------------------------------------------
    # Write operations
    # ------------------------------------------------------------------

    def add(self, nodes: List[BaseNode], **kwargs: Any) -> List[str]:
        """Store a list of ``BaseNode`` objects with their embeddings.

        Each node must have an ``embedding`` set before calling this method.
        The node's text and metadata are serialised into the AgentDB metadata
        field so they can be reconstructed on retrieval.

        Args:
            nodes: List of ``BaseNode`` instances (typically ``TextNode``).

        Returns:
            List of IDs for the stored nodes.
        """
        if not nodes:
            return []

        entries = []
        ids: List[str] = []

        for node in nodes:
            node_id = node.node_id or str(uuid.uuid4())
            ids.append(node_id)

            embedding = node.get_embedding()
            if embedding is None:
                raise ValueError(
                    f"Node {node_id!r} has no embedding.  "
                    "Embed the nodes before calling add()."
                )

            # Serialise enough state to reconstruct the node on retrieval.
            # We store: the text content (for TextNode), and the node metadata.
            stored_meta: Dict[str, Any] = {}

            if isinstance(node, TextNode):
                stored_meta["_text"] = node.text
            else:
                # For non-text nodes store the JSON representation.
                try:
                    stored_meta["_text"] = node.get_content()
                except Exception:
                    stored_meta["_text"] = ""

            # Preserve LlamaIndex metadata under a nested key to avoid
            # collisions with our private "_text" / "_node_type" keys.
            stored_meta["_node_type"] = type(node).__name__
            stored_meta["_metadata"] = node.metadata or {}

            # Also keep ref_doc_id if present so delete() can find by doc ID.
            if node.ref_doc_id:
                stored_meta["_ref_doc_id"] = node.ref_doc_id

            entries.append(
                {
                    "id": node_id,
                    "vector": [float(v) for v in embedding],
                    "metadata": stored_meta,
                }
            )

        self._collection.upsert_batch(entries)
        return ids

    def delete(self, ref_doc_id: str, **kwargs: Any) -> None:
        """Delete all nodes whose ``ref_doc_id`` matches ``ref_doc_id``.

        AgentDB's collection does not natively support filtering deletes by
        metadata, so we perform a broad similarity search with the zero-vector
        to collect all nodes, then delete those whose stored ``_ref_doc_id``
        matches.

        For direct deletion by node ID, pass ``node_id=<id>`` as a keyword
        argument instead.

        Args:
            ref_doc_id: The document ID whose nodes should be removed.
        """
        # Allow callers to delete by node ID directly via a keyword argument.
        node_id: Optional[str] = kwargs.get("node_id")
        if node_id is not None:
            self._collection.delete(node_id)
            return

        # Scan all vectors and delete any whose _ref_doc_id matches.
        # We use the zero-vector as the query with a large top_k to retrieve
        # as many entries as possible.  This is a best-effort scan; callers
        # who store very large collections should maintain an external mapping.
        zero_vec = [0.0] * self._dimension
        try:
            results = self._collection.search(zero_vec, top_k=10_000)
        except Exception:
            results = []

        for result in results:
            meta = result.metadata or {}
            if meta.get("_ref_doc_id") == ref_doc_id:
                self._collection.delete(result.id)

    # ------------------------------------------------------------------
    # Query
    # ------------------------------------------------------------------

    def query(self, query: VectorStoreQuery, **kwargs: Any) -> VectorStoreQueryResult:
        """Perform a similarity search and return matching nodes.

        Args:
            query: A ``VectorStoreQuery`` containing the query embedding and
                parameters such as ``similarity_top_k``.

        Returns:
            A ``VectorStoreQueryResult`` with ``nodes``, ``similarities``, and
            ``ids`` aligned by index.
        """
        if query.query_embedding is None:
            raise ValueError(
                "query.query_embedding must be set before calling query().  "
                "Use an embedder to generate the query vector first."
            )

        top_k = query.similarity_top_k or 4

        raw_results = self._collection.search(
            [float(v) for v in query.query_embedding],
            top_k=top_k,
        )

        result_nodes: List[BaseNode] = []
        similarities: List[float] = []
        ids: List[str] = []

        for result in raw_results:
            meta = result.metadata or {}

            text = meta.get("_text", "")
            node_metadata: Dict[str, Any] = meta.get("_metadata", {})

            node = TextNode(
                id_=result.id,
                text=text,
                metadata=node_metadata,
            )

            result_nodes.append(node)
            similarities.append(float(result.score))
            ids.append(result.id)

        return VectorStoreQueryResult(
            nodes=result_nodes,
            similarities=similarities,
            ids=ids,
        )

    # ------------------------------------------------------------------
    # Utility
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return self._collection.count()

    def __repr__(self) -> str:
        return (
            f"AgentDBVectorStore("
            f"db_path={self._db_path!r}, "
            f"collection={self._collection_name!r}, "
            f"dim={self._dimension})"
        )
