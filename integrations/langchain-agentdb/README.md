# langchain-agentdb

LangChain integrations for [AgentDB](https://github.com/datacules/agentdb) — an embedded, AI-native database built on SQLite.

Provides:

- **`AgentDBVectorStore`** — A `langchain_core.vectorstores.VectorStore` backed by AgentDB's HNSW vector index.
- **`AgentDBChatMessageHistory`** — A `langchain_core.chat_history.BaseChatMessageHistory` that persists conversation threads in AgentDB.
- **`AgentDBChatMemory`** — A `langchain_core.memory.BaseMemory` wrapper for use with older LCEL chain patterns.

## Installation

```bash
pip install langchain-agentdb
```

AgentDB itself is a compiled Rust extension distributed as a wheel:

```bash
pip install datacules-agentdb
```

## Quick start — VectorStore

```python
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

for doc in results:
    print(doc.page_content, doc.metadata)
```

### From a list of documents

```python
from langchain_core.documents import Document

docs = [
    Document(page_content="AgentDB stores vectors natively", metadata={"source": "docs"}),
    Document(page_content="LangChain makes LLM apps easy", metadata={"source": "blog"}),
]

store = AgentDBVectorStore.from_documents(
    documents=docs,
    embedding=OpenAIEmbeddings(),
    db_path="agent.agentdb",
    collection_name="docs",
)
```

### Similarity search with scores

```python
results = store.similarity_search_with_score("vector database", k=5)
for doc, score in results:
    print(f"{score:.4f}  {doc.page_content}")
```

### Pre-computed embeddings

```python
query_vector = my_model.encode("fast database").tolist()
results = store.similarity_search_by_vector(query_vector, k=3)
```

## Quick start — Chat Memory

### With `RunnableWithMessageHistory` (recommended)

```python
from langchain_agentdb import AgentDBChatMessageHistory
from langchain_core.runnables.history import RunnableWithMessageHistory
from langchain_openai import ChatOpenAI

llm = ChatOpenAI()

chain_with_history = RunnableWithMessageHistory(
    llm,
    lambda session_id: AgentDBChatMessageHistory(
        db_path="agent.agentdb",
        conversation_id=session_id,
    ),
    input_messages_key="input",
    history_messages_key="history",
)

response = chain_with_history.invoke(
    {"input": "What is AgentDB?"},
    config={"configurable": {"session_id": "user-session-1"}},
)
```

### Direct use

```python
from langchain_agentdb import AgentDBChatMessageHistory

history = AgentDBChatMessageHistory(
    db_path="agent.agentdb",
    conversation_id="session-42",
    title="Support chat",
)

history.add_user_message("What is AgentDB?")
history.add_ai_message("AgentDB is a fast embedded AI database written in Rust.")

for msg in history.messages:
    print(f"{msg.type}: {msg.content}")

# Clear the conversation
history.clear()
```

### Legacy `BaseMemory` wrapper

```python
from langchain_agentdb import AgentDBChatMemory
from langchain.chains import ConversationChain
from langchain_openai import OpenAI

memory = AgentDBChatMemory(
    db_path="agent.agentdb",
    conversation_id="session-42",
    return_messages=False,
)

chain = ConversationChain(llm=OpenAI(), memory=memory)
chain.predict(input="Tell me about AgentDB.")
```

## Configuration reference

### `AgentDBVectorStore`

| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `str` | Path to the AgentDB file.  Created if absent. |
| `collection_name` | `str` | Name of the vector collection. |
| `dimension` | `int` | Embedding dimensionality. |
| `embedding` | `Embeddings` | LangChain embeddings instance. |

### `AgentDBChatMessageHistory`

| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `str` | Path to the AgentDB file. |
| `conversation_id` | `str` | Unique conversation ID.  Auto-generated (UUID4) when omitted. |
| `title` | `str \| None` | Optional human-readable title for the conversation. |

## Running the tests

```bash
cd integrations/langchain-agentdb
pip install -e ".[dev]"
pytest
```

## License

MIT — see the top-level [LICENSE](../../LICENSE) file.
