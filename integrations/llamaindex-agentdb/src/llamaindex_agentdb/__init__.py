"""LlamaIndex integrations for AgentDB.

Exports:

- ``AgentDBVectorStore`` — A ``llama_index.core.vector_stores.types.BasePydanticVectorStore``
  implementation backed by AgentDB's native HNSW vector index.

- ``AgentDBChatStore`` — A ``llama_index.core.storage.chat_store.BaseChatStore``
  implementation that persists conversation messages in AgentDB.
"""

from llamaindex_agentdb.chat_store import AgentDBChatStore
from llamaindex_agentdb.vector_store import AgentDBVectorStore

__all__ = [
    "AgentDBVectorStore",
    "AgentDBChatStore",
]

__version__ = "0.1.0"
