"""LangChain integrations for AgentDB.

Exports:

- ``AgentDBVectorStore`` — A ``langchain_core.vectorstores.VectorStore``
  implementation backed by AgentDB's native HNSW vector index.

- ``AgentDBChatMessageHistory`` — A ``langchain_core.chat_history.BaseChatMessageHistory``
  implementation that persists conversation messages in AgentDB.

- ``AgentDBChatMemory`` — A ``langchain_core.memory.BaseMemory`` wrapper
  around ``AgentDBChatMessageHistory`` for use with older LCEL / chain
  patterns.  ``None`` when the installed ``langchain-core`` version does not
  expose ``BaseMemory``.
"""

from langchain_agentdb.memory import AgentDBChatMemory, AgentDBChatMessageHistory
from langchain_agentdb.vectorstore import AgentDBVectorStore

__all__ = [
    "AgentDBVectorStore",
    "AgentDBChatMessageHistory",
    "AgentDBChatMemory",
]

__version__ = "0.1.0"
