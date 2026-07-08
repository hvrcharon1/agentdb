# llamaindex-agentdb

LlamaIndex integrations for [AgentDB](https://github.com/datacules/agentdb) — an embedded, AI-native database built on SQLite with a native HNSW vector index.

Provides:

- **`AgentDBVectorStore`** — A `BasePydanticVectorStore` backed by AgentDB's HNSW vector index.
- **`AgentDBChatStore`** — A `BaseChatStore` that persists conversation threads in AgentDB.

## Installation

```bash
pip install llamaindex-agentdb
```

AgentDB itself is a compiled Rust extension distributed as a wheel:

```bash
pip install datacules-agentdb
```

## Quick start — VectorStoreIndex

```python
from llama_index.core import VectorStoreIndex, StorageContext, Document
from llama_index.core.node_parser import SentenceSplitter
from llama_index.embeddings.openai import OpenAIEmbedding
from llamaindex_agentdb import AgentDBVectorStore

# 1. Create the AgentDB-backed vector store
vector_store = AgentDBVectorStore(
    db_path="agent.agentdb",
    collection_name="docs",
    dimension=1536,
)

# 2. Wire it into a StorageContext
storage_context = StorageContext.from_defaults(vector_store=vector_store)

# 3. Build a VectorStoreIndex from your documents
documents = [
    Document(text="AgentDB is a fast, embedded AI database built in Rust."),
    Document(text="LlamaIndex makes it easy to build RAG applications."),
]

index = VectorStoreIndex.from_documents(
    documents,
    storage_context=storage_context,
    embed_model=OpenAIEmbedding(),
)

# 4. Query
query_engine = index.as_query_engine()
response = query_engine.query("What is AgentDB?")
print(response)
```

### Load an existing index

```python
from llama_index.core import VectorStoreIndex, StorageContext
from llamaindex_agentdb import AgentDBVectorStore

vector_store = AgentDBVectorStore(
    db_path="agent.agentdb",
    collection_name="docs",
    dimension=1536,
)
storage_context = StorageContext.from_defaults(vector_store=vector_store)

# Load without re-ingesting documents
index = VectorStoreIndex.from_vector_store(
    vector_store,
    storage_context=storage_context,
)
query_engine = index.as_query_engine()
```

### Direct vector store usage

```python
from llama_index.core.schema import TextNode
from llama_index.core.vector_stores.types import VectorStoreQuery
from llamaindex_agentdb import AgentDBVectorStore

store = AgentDBVectorStore(
    db_path="agent.agentdb",
    collection_name="nodes",
    dimension=8,
)

# Add nodes (embeddings must be pre-computed)
node = TextNode(id_="doc-1", text="Hello world", embedding=[0.1] * 8)
store.add([node])

# Query
query = VectorStoreQuery(query_embedding=[0.1] * 8, similarity_top_k=3)
result = store.query(query)
for n, score in zip(result.nodes, result.similarities):
    print(f"{score:.4f}  {n.get_content()}")

# Delete by node ID
store.delete("unused-doc-id", node_id="doc-1")
```

## Quick start — ChatStore

```python
from llama_index.core.llms import ChatMessage, MessageRole
from llama_index.core.memory import ChatMemoryBuffer
from llamaindex_agentdb import AgentDBChatStore

# Create the store
chat_store = AgentDBChatStore(db_path="agent.agentdb")

# Use it as a persistent memory backend
memory = ChatMemoryBuffer.from_defaults(
    token_limit=3000,
    chat_store=chat_store,
    chat_store_key="user-session-1",
)

# Add messages
chat_store.add_message(
    "user-session-1",
    ChatMessage(role=MessageRole.USER, content="What is AgentDB?"),
)
chat_store.add_message(
    "user-session-1",
    ChatMessage(role=MessageRole.ASSISTANT, content="AgentDB is a fast embedded AI database."),
)

# Retrieve messages
messages = chat_store.get_messages("user-session-1")
for msg in messages:
    print(f"{msg.role.value}: {msg.content}")

# List all sessions
keys = chat_store.get_keys()
print("Sessions:", keys)

# Delete a session
chat_store.delete_messages("user-session-1")
```

## Configuration reference

### `AgentDBVectorStore`

| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `str` | Path to the AgentDB file.  Created if absent. |
| `collection_name` | `str` | Name of the vector collection. |
| `dimension` | `int` | Embedding dimensionality. |

### `AgentDBChatStore`

| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `str` | Path to the AgentDB file.  Created if absent. |

## Running the tests

```bash
cd integrations/llamaindex-agentdb
pip install -e ".[dev]"
pytest
```

## License

MIT — see the top-level [LICENSE](../../LICENSE) file.
