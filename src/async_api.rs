use crate::conversations::{Conversation, Message};
use crate::db::{AgentDB, DbStats};
use crate::error::Result;
use crate::fts::FtsResult;
use crate::hybrid::{HybridQuery, HybridResult};
use crate::memory::{TraversalOptions, TraversalResult};
use crate::traces::Trace;
use crate::vectors::{BatchEntry, Collection, SearchOptions, SearchResult, VectorEntry};
use crate::workflows::Workflow;
use serde_json::Value;
use std::sync::Arc;
use tokio::task;

/// Async wrapper around [`AgentDB`] that offloads blocking SQLite I/O
/// to Tokio's blocking thread pool via [`tokio::task::spawn_blocking`].
///
/// All methods are `Send + Sync` and safe to use from any async context.
#[derive(Clone)]
pub struct AsyncAgentDB {
    inner: Arc<AgentDB>,
}

impl AsyncAgentDB {
    /// Open or create an AgentDB database asynchronously.
    pub async fn open(path: &str) -> Result<Self> {
        let path = path.to_string();
        let db = task::spawn_blocking(move || AgentDB::open(&path))
            .await
            .expect("spawn_blocking join")?;
        Ok(Self {
            inner: Arc::new(db),
        })
    }

    /// Execute a raw SQL statement.
    pub async fn execute(&self, sql: &str) -> Result<usize> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || db.execute(&sql))
            .await
            .expect("spawn_blocking join")
    }

    /// Execute a batch of semicolon-separated SQL statements atomically.
    pub async fn execute_batch(&self, sql: &str) -> Result<()> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || db.execute_batch(&sql))
            .await
            .expect("spawn_blocking join")
    }

    /// Query and return rows as JSON values.
    pub async fn query_json(&self, sql: &str) -> Result<Vec<Value>> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || db.query_json(&sql))
            .await
            .expect("spawn_blocking join")
    }

    /// Return database-wide statistics.
    pub async fn stats(&self) -> Result<DbStats> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.stats())
            .await
            .expect("spawn_blocking join")
    }

    /// Access an async vector collection handle.
    pub fn vectors(&self) -> AsyncVectorStore {
        AsyncVectorStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async memory graph layer.
    pub fn memory(&self) -> AsyncMemoryGraph {
        AsyncMemoryGraph {
            inner: self.inner.clone(),
        }
    }

    /// Access the async full-text search layer.
    pub fn fts(&self) -> AsyncFullTextStore {
        AsyncFullTextStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async conversation layer.
    pub fn conversations(&self) -> AsyncConversationStore {
        AsyncConversationStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async workflow layer.
    pub fn workflows(&self) -> AsyncWorkflowStore {
        AsyncWorkflowStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async trace layer.
    pub fn traces(&self) -> AsyncTraceStore {
        AsyncTraceStore {
            inner: self.inner.clone(),
        }
    }

    /// Run a hybrid graph + vector query.
    pub async fn hybrid_query(
        &self,
        anchor_node: &str,
        embedding: Vec<f32>,
        collection: &str,
        graph_depth: usize,
        top_k: usize,
        alpha: f64,
        filter: Option<Value>,
    ) -> Result<Vec<HybridResult>> {
        let db = self.inner.clone();
        let anchor = anchor_node.to_string();
        let col = collection.to_string();
        task::spawn_blocking(move || {
            let q = HybridQuery {
                anchor_node: &anchor,
                embedding: &embedding,
                collection: &col,
                graph_depth,
                top_k,
                alpha,
                filter,
            };
            db.hybrid_query(q)
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Flush dirty indexes and close gracefully.
    pub async fn close(self) -> Result<()> {
        let db = Arc::try_unwrap(self.inner).unwrap_or_else(|arc| {
            panic!(
                "AsyncAgentDB::close called while {} other references exist",
                Arc::strong_count(&arc) - 1
            )
        });
        task::spawn_blocking(move || db.close())
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Vector Store ──────────────────────────────────────────────────

/// Async wrapper for vector operations.
pub struct AsyncVectorStore {
    inner: Arc<AgentDB>,
}

impl AsyncVectorStore {
    /// Get or create a named collection and return an async handle.
    pub async fn collection(&self, name: &str, dim: usize) -> Result<AsyncCollection> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || {
            db.vectors()
                .collection(&name, dim)
                .map(|c| AsyncCollection { inner: Arc::new(c) })
        })
        .await
        .expect("spawn_blocking join")
    }
}

/// Async wrapper around a single vector [`Collection`].
#[derive(Clone)]
pub struct AsyncCollection {
    inner: Arc<Collection>,
}

impl AsyncCollection {
    /// Upsert a single vector.
    pub async fn upsert(&self, entry: VectorEntry) -> Result<()> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.upsert(entry))
            .await
            .expect("spawn_blocking join")
    }

    /// Batch upsert multiple vectors atomically.
    pub async fn upsert_batch(&self, entries: Vec<BatchEntry>) -> Result<usize> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.upsert_batch(entries))
            .await
            .expect("spawn_blocking join")
    }

    /// ANN search.
    pub async fn search(
        &self,
        query: Vec<f32>,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.search(&query, options))
            .await
            .expect("spawn_blocking join")
    }

    /// Number of vectors in this collection.
    pub async fn count(&self) -> Result<i64> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.count())
            .await
            .expect("spawn_blocking join")
    }

    /// Rebuild the HNSW index.
    pub async fn reindex(&self) -> Result<()> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.reindex())
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Memory Graph ──────────────────────────────────────────────────

/// Async wrapper for memory graph operations.
pub struct AsyncMemoryGraph {
    inner: Arc<AgentDB>,
}

impl AsyncMemoryGraph {
    /// Add or update a node.
    pub async fn add_node(&self, id: &str, kind: &str, data: Option<Value>) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        let kind = kind.to_string();
        task::spawn_blocking(move || db.memory().add_node(&id, &kind, data))
            .await
            .expect("spawn_blocking join")
    }

    /// Add or update a directed edge.
    pub async fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> Result<()> {
        let db = self.inner.clone();
        let src = src.to_string();
        let dst = dst.to_string();
        let relation = relation.to_string();
        task::spawn_blocking(move || db.memory().add_edge(&src, &dst, &relation, weight))
            .await
            .expect("spawn_blocking join")
    }

    /// Traverse the graph from a node.
    pub async fn neighbors(
        &self,
        node_id: &str,
        opts: TraversalOptions,
    ) -> Result<Vec<TraversalResult>> {
        let db = self.inner.clone();
        let node_id = node_id.to_string();
        task::spawn_blocking(move || db.memory().neighbors(&node_id, opts))
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Full-Text Search ──────────────────────────────────────────────

/// Async wrapper for FTS operations.
pub struct AsyncFullTextStore {
    inner: Arc<AgentDB>,
}

impl AsyncFullTextStore {
    /// Index a text document.
    pub async fn index_text(
        &self,
        collection: &str,
        id: &str,
        collection_id: &str,
        text: &str,
    ) -> Result<()> {
        let db = self.inner.clone();
        let collection = collection.to_string();
        let id = id.to_string();
        let collection_id = collection_id.to_string();
        let text = text.to_string();
        task::spawn_blocking(move || db.fts().index_text(&collection, &id, &collection_id, &text))
            .await
            .expect("spawn_blocking join")
    }

    /// Full-text search.
    pub async fn search(
        &self,
        collection: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<FtsResult>> {
        let db = self.inner.clone();
        let collection = collection.to_string();
        let query = query.to_string();
        task::spawn_blocking(move || db.fts().search(&collection, &query, top_k))
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Conversations ─────────────────────────────────────────────────

/// Async wrapper for conversation operations.
pub struct AsyncConversationStore {
    inner: Arc<AgentDB>,
}

impl AsyncConversationStore {
    /// Create a new conversation.
    pub async fn create_conversation(
        &self,
        id: &str,
        title: Option<&str>,
        metadata: Option<Value>,
    ) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        let title = title.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.conversations()
                .create_conversation(&id, title.as_deref(), metadata)
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Append a message to a conversation.
    pub async fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let cid = conversation_id.to_string();
        let role = role.to_string();
        let content = content.to_string();
        task::spawn_blocking(move || {
            db.conversations()
                .add_message(&cid, &role, &content, metadata)
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Get messages for a conversation.
    pub async fn get_messages(
        &self,
        conversation_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        let db = self.inner.clone();
        let cid = conversation_id.to_string();
        task::spawn_blocking(move || db.conversations().get_messages(&cid, limit))
            .await
            .expect("spawn_blocking join")
    }

    /// List all conversations.
    pub async fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.conversations().list_conversations())
            .await
            .expect("spawn_blocking join")
    }

    /// Delete a conversation and all its messages.
    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.conversations().delete_conversation(&id))
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Workflows ─────────────────────────────────────────────────────

/// Async wrapper for workflow operations.
pub struct AsyncWorkflowStore {
    inner: Arc<AgentDB>,
}

impl AsyncWorkflowStore {
    /// Create a new workflow.
    pub async fn create_workflow(&self, id: &str, name: &str, input: Option<Value>) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        let name = name.to_string();
        task::spawn_blocking(move || db.workflows().create_workflow(&id, &name, input))
            .await
            .expect("spawn_blocking join")
    }

    /// Append a step to a workflow.
    pub async fn add_step(
        &self,
        workflow_id: &str,
        name: &str,
        input: Option<Value>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let wid = workflow_id.to_string();
        let name = name.to_string();
        task::spawn_blocking(move || db.workflows().add_step(&wid, &name, input))
            .await
            .expect("spawn_blocking join")
    }

    /// Update a step's status/output/error.
    pub async fn update_step(
        &self,
        step_id: &str,
        status: &str,
        output: Option<Value>,
        error: Option<&str>,
    ) -> Result<()> {
        let db = self.inner.clone();
        let sid = step_id.to_string();
        let status = status.to_string();
        let error = error.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.workflows()
                .update_step(&sid, &status, output, error.as_deref())
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Mark a workflow as completed.
    pub async fn complete_workflow(&self, id: &str, output: Option<Value>) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.workflows().complete_workflow(&id, output))
            .await
            .expect("spawn_blocking join")
    }

    /// Get a workflow and its steps.
    pub async fn get_workflow(&self, id: &str) -> Result<Workflow> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.workflows().get_workflow(&id))
            .await
            .expect("spawn_blocking join")
    }

    /// List workflows with optional status filter.
    pub async fn list_workflows(&self, status_filter: Option<&str>) -> Result<Vec<Workflow>> {
        let db = self.inner.clone();
        let status = status_filter.map(|s| s.to_string());
        task::spawn_blocking(move || db.workflows().list_workflows(status.as_deref()))
            .await
            .expect("spawn_blocking join")
    }
}

// ── Async Traces ────────────────────────────────────────────────────────

/// Async wrapper for trace operations.
pub struct AsyncTraceStore {
    inner: Arc<AgentDB>,
}

impl AsyncTraceStore {
    /// Record a new trace entry.
    pub async fn add_trace(
        &self,
        session_id: Option<&str>,
        parent_id: Option<&str>,
        trace_type: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let sid = session_id.map(|s| s.to_string());
        let pid = parent_id.map(|s| s.to_string());
        let tt = trace_type.to_string();
        let content = content.to_string();
        task::spawn_blocking(move || {
            db.traces()
                .add_trace(sid.as_deref(), pid.as_deref(), &tt, &content, metadata)
        })
        .await
        .expect("spawn_blocking join")
    }

    /// Get all traces for a session.
    pub async fn get_traces(&self, session_id: &str) -> Result<Vec<Trace>> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        task::spawn_blocking(move || db.traces().get_traces(&sid))
            .await
            .expect("spawn_blocking join")
    }

    /// Get a trace subtree rooted at `root_id`.
    pub async fn get_trace_tree(&self, root_id: &str) -> Result<Vec<Trace>> {
        let db = self.inner.clone();
        let rid = root_id.to_string();
        task::spawn_blocking(move || db.traces().get_trace_tree(&rid))
            .await
            .expect("spawn_blocking join")
    }
}
