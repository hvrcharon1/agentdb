use crate::audit::AuditEntry;
use crate::context::ContextEntry;
use crate::conversations::{Conversation, Message};
use crate::db::{AgentDB, DbStats};
use crate::error::Result;
use crate::fts::FtsResult;
use crate::hybrid::{HybridQuery, HybridResult, TriModalQuery, TriModalResult};
use crate::labels::DataLabel;
use crate::memory::{TraversalOptions, TraversalResult};
use crate::prompts::PromptTemplate;
use crate::tools::{Tool, ToolCall};
use crate::traces::Trace;
use crate::vectors::{
    BatchEntry, Collection, DistanceMetric, SearchOptions, SearchResult, VectorEntry,
};
use crate::workflows::Workflow;
use serde_json::Value;
use std::collections::HashMap;
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))??;
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Execute a batch of semicolon-separated SQL statements atomically.
    pub async fn execute_batch(&self, sql: &str) -> Result<()> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || db.execute_batch(&sql))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Query and return rows as JSON values.
    pub async fn query_json(&self, sql: &str) -> Result<Vec<Value>> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || db.query_json(&sql))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Query with parameters and return rows as JSON values.
    pub async fn query_json_params(&self, sql: &str, params: Vec<String>) -> Result<Vec<Value>> {
        let db = self.inner.clone();
        let sql = sql.to_string();
        task::spawn_blocking(move || {
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            db.query_json_params(&sql, &param_refs)
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Return database-wide statistics.
    pub async fn stats(&self) -> Result<DbStats> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.stats())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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

    /// Access the async tool registry layer.
    pub fn tools(&self) -> AsyncToolStore {
        AsyncToolStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async audit log layer.
    pub fn audit(&self) -> AsyncAuditStore {
        AsyncAuditStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async context window layer.
    pub fn context(&self) -> AsyncContextStore {
        AsyncContextStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async prompt templates layer.
    pub fn prompts(&self) -> AsyncPromptStore {
        AsyncPromptStore {
            inner: self.inner.clone(),
        }
    }

    /// Access the async data labels layer.
    pub fn labels(&self) -> AsyncLabelStore {
        AsyncLabelStore {
            inner: self.inner.clone(),
        }
    }

    /// Run a tri-modal graph + vector + FTS query.
    pub async fn tri_modal_query(&self, query: TriModalQuery) -> Result<Vec<TriModalResult>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.tri_modal_query(&query))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Flush dirty indexes and close gracefully.
    ///
    /// Returns `Err(InvalidArgument)` if other `AsyncAgentDB` clones still
    /// hold a reference to the same database — the caller must drop all clones
    /// before calling `close()`.
    pub async fn close(self) -> Result<()> {
        let db = Arc::try_unwrap(self.inner).map_err(|arc| {
            crate::error::AgentDbError::InvalidArgument(format!(
                "AsyncAgentDB::close called while {} other reference(s) exist",
                Arc::strong_count(&arc) - 1
            ))
        })?;
        task::spawn_blocking(move || db.close())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get or create a named collection with an explicit distance metric.
    pub async fn collection_with_metric(
        &self,
        name: &str,
        dim: usize,
        metric: DistanceMetric,
    ) -> Result<AsyncCollection> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || {
            db.vectors()
                .collection_with_metric(&name, dim, metric)
                .map(|c| AsyncCollection { inner: Arc::new(c) })
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// List all collections as `(name, dim, count)` tuples.
    pub async fn list_collections(&self) -> Result<Vec<(String, usize, i64)>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.vectors().list_collections())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Drop a collection and all its vectors.
    pub async fn drop_collection(&self, name: &str) -> Result<()> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.vectors().drop_collection(&name))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Batch upsert multiple vectors atomically.
    pub async fn upsert_batch(&self, entries: Vec<BatchEntry>) -> Result<usize> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.upsert_batch(entries))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Number of vectors in this collection.
    pub async fn count(&self) -> Result<i64> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.count())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Rebuild the HNSW index.
    pub async fn reindex(&self) -> Result<()> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.reindex())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a vector by ID.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let col = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || col.delete(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Upsert a vector and index its text content atomically.
    pub async fn upsert_with_text(&self, entry: VectorEntry, text: String) -> Result<()> {
        let col = self.inner.clone();
        task::spawn_blocking(move || col.upsert_with_text(entry, &text))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Add or update a directed edge.
    pub async fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> Result<()> {
        let db = self.inner.clone();
        let src = src.to_string();
        let dst = dst.to_string();
        let relation = relation.to_string();
        task::spawn_blocking(move || db.memory().add_edge(&src, &dst, &relation, weight))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get a single node by ID.
    pub async fn get_node(&self, id: &str) -> Result<crate::memory::Node> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.memory().get_node(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a node and its edges.
    pub async fn delete_node(&self, id: &str) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.memory().delete_node(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a specific edge.
    pub async fn delete_edge(&self, src: &str, dst: &str, relation: &str) -> Result<()> {
        let db = self.inner.clone();
        let src = src.to_string();
        let dst = dst.to_string();
        let relation = relation.to_string();
        task::spawn_blocking(move || db.memory().delete_edge(&src, &dst, &relation))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Return all nodes of a given kind.
    pub async fn nodes_by_kind(&self, kind: &str) -> Result<Vec<crate::memory::Node>> {
        let db = self.inner.clone();
        let kind = kind.to_string();
        task::spawn_blocking(move || db.memory().nodes_by_kind(&kind))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a text entry from the FTS index.
    pub async fn delete_text(&self, collection: &str, id: &str) -> Result<()> {
        let db = self.inner.clone();
        let collection = collection.to_string();
        let id = id.to_string();
        task::spawn_blocking(move || db.fts().delete_text(&collection, &id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Optimize the FTS5 index for a collection.
    pub async fn optimize(&self, collection: &str) -> Result<()> {
        let db = self.inner.clone();
        let collection = collection.to_string();
        task::spawn_blocking(move || db.fts().optimize(&collection))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// List all conversations.
    pub async fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.conversations().list_conversations())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a conversation and all its messages.
    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.conversations().delete_conversation(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Full-text search over message content.
    pub async fn search_messages(
        &self,
        query: &str,
        top_k: usize,
        conversation_id: Option<&str>,
    ) -> Result<Vec<crate::conversations::MessageSearchResult>> {
        let db = self.inner.clone();
        let q = query.to_string();
        let cid = conversation_id.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.conversations()
                .search_messages(&q, top_k, cid.as_deref())
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Workflows ─────────────────────────────────────────────────────

/// Async wrapper for workflow operations.
pub struct AsyncWorkflowStore {
    inner: Arc<AgentDB>,
}

impl AsyncWorkflowStore {
    /// Create a new workflow.
    pub async fn create_workflow(
        &self,
        id: &str,
        name: &str,
        input: Option<Value>,
        metadata: Option<Value>,
    ) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        let name = name.to_string();
        task::spawn_blocking(move || db.workflows().create_workflow(&id, &name, input, metadata))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Mark a workflow as completed.
    pub async fn complete_workflow(&self, id: &str, output: Option<Value>) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.workflows().complete_workflow(&id, output))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Mark a workflow as failed.
    pub async fn fail_workflow(&self, id: &str, error: Option<&str>) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        let error = error.map(|s| s.to_string());
        task::spawn_blocking(move || db.workflows().fail_workflow(&id, error.as_deref()))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get a workflow and its steps.
    pub async fn get_workflow(&self, id: &str) -> Result<Workflow> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.workflows().get_workflow(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// List workflows with optional status filter.
    pub async fn list_workflows(&self, status_filter: Option<&str>) -> Result<Vec<Workflow>> {
        let db = self.inner.clone();
        let status = status_filter.map(|s| s.to_string());
        task::spawn_blocking(move || db.workflows().list_workflows(status.as_deref()))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
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
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get traces for a session with optional pagination.
    pub async fn get_traces(
        &self,
        session_id: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Trace>> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        task::spawn_blocking(move || db.traces().get_traces(&sid, limit, offset))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get a trace subtree rooted at `root_id`.
    pub async fn get_trace_tree(&self, root_id: &str) -> Result<Vec<Trace>> {
        let db = self.inner.clone();
        let rid = root_id.to_string();
        task::spawn_blocking(move || db.traces().get_trace_tree(&rid))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Tool Store ───────────────────────────────────────────────────

/// Async wrapper for tool registry operations.
pub struct AsyncToolStore {
    inner: Arc<AgentDB>,
}

impl AsyncToolStore {
    /// Register or update a tool definition.
    pub async fn register_tool(
        &self,
        name: &str,
        description: Option<&str>,
        parameters_schema: Option<Value>,
        version: Option<&str>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let name = name.to_string();
        let desc = description.map(|s| s.to_string());
        let ver = version.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.tools()
                .register_tool(&name, desc.as_deref(), parameters_schema, ver.as_deref())
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get a tool by name.
    pub async fn get_tool(&self, name: &str) -> Result<Tool> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.tools().get_tool(&name))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// List all registered tools.
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.tools().list_tools())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a tool by name.
    pub async fn delete_tool(&self, name: &str) -> Result<()> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.tools().delete_tool(&name))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Log a tool call invocation.
    pub async fn log_tool_call(
        &self,
        session_id: Option<&str>,
        tool_name: &str,
        arguments: Option<Value>,
        result: Option<Value>,
        error: Option<&str>,
        latency_ms: Option<i64>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let sid = session_id.map(|s| s.to_string());
        let name = tool_name.to_string();
        let err = error.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.tools().log_tool_call(
                sid.as_deref(),
                &name,
                arguments,
                result,
                err.as_deref(),
                latency_ms,
            )
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get tool calls with optional filters.
    pub async fn get_tool_calls(
        &self,
        session_id: Option<&str>,
        tool_name: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<ToolCall>> {
        let db = self.inner.clone();
        let sid = session_id.map(|s| s.to_string());
        let name = tool_name.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.tools()
                .get_tool_calls(sid.as_deref(), name.as_deref(), limit)
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Audit Store ──────────────────────────────────────────────────

/// Async wrapper for audit log operations.
pub struct AsyncAuditStore {
    inner: Arc<AgentDB>,
}

impl AsyncAuditStore {
    /// Append an entry to the immutable audit log.
    #[allow(clippy::too_many_arguments)]
    pub async fn log(
        &self,
        actor: Option<&str>,
        action: &str,
        table_name: &str,
        record_id: &str,
        old_value: Option<Value>,
        new_value: Option<Value>,
        reason: Option<&str>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let actor = actor.map(|s| s.to_string());
        let action = action.to_string();
        let table = table_name.to_string();
        let record = record_id.to_string();
        let reason = reason.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.audit().log(
                actor.as_deref(),
                &action,
                &table,
                &record,
                old_value,
                new_value,
                reason.as_deref(),
            )
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Query audit entries by record.
    pub async fn query_by_record(
        &self,
        table_name: &str,
        record_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        task::spawn_blocking(move || db.audit().query_by_record(&table, &record, limit))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Query audit entries by actor.
    pub async fn query_by_actor(
        &self,
        actor: &str,
        limit: Option<usize>,
    ) -> Result<Vec<AuditEntry>> {
        let db = self.inner.clone();
        let actor = actor.to_string();
        task::spawn_blocking(move || db.audit().query_by_actor(&actor, limit))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Query recent audit entries.
    pub async fn query_recent(&self, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.audit().query_recent(limit))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Context Store ────────────────────────────────────────────────

/// Async wrapper for context window operations.
pub struct AsyncContextStore {
    inner: Arc<AgentDB>,
}

impl AsyncContextStore {
    /// Add an entry to the context window.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_entry(
        &self,
        session_id: &str,
        source_type: &str,
        source_id: &str,
        content_preview: Option<&str>,
        token_count: i64,
        relevance_score: f64,
        priority: i64,
    ) -> Result<String> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        let st = source_type.to_string();
        let si = source_id.to_string();
        let cp = content_preview.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.context().add_entry(
                &sid,
                &st,
                &si,
                cp.as_deref(),
                token_count,
                relevance_score,
                priority,
            )
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Build a token-budgeted context window.
    pub async fn build_window(
        &self,
        session_id: &str,
        max_tokens: i64,
    ) -> Result<Vec<ContextEntry>> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        task::spawn_blocking(move || db.context().build_window(&sid, max_tokens))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get all context entries for a session.
    pub async fn get_entries(&self, session_id: &str) -> Result<Vec<ContextEntry>> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        task::spawn_blocking(move || db.context().get_entries(&sid))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Clear all context entries for a session.
    pub async fn clear_session(&self, session_id: &str) -> Result<()> {
        let db = self.inner.clone();
        let sid = session_id.to_string();
        task::spawn_blocking(move || db.context().clear_session(&sid))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Remove a single context entry by ID.
    pub async fn remove_entry(&self, id: &str) -> Result<()> {
        let db = self.inner.clone();
        let id = id.to_string();
        task::spawn_blocking(move || db.context().remove_entry(&id))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Prompt Store ─────────────────────────────────────────────────

/// Async wrapper for prompt template operations.
pub struct AsyncPromptStore {
    inner: Arc<AgentDB>,
}

impl AsyncPromptStore {
    /// Create a new version of a prompt template.
    pub async fn create_template(
        &self,
        name: &str,
        template: &str,
        model_hint: Option<&str>,
        max_tokens: Option<i64>,
        metadata: Option<Value>,
    ) -> Result<String> {
        let db = self.inner.clone();
        let name = name.to_string();
        let tmpl = template.to_string();
        let hint = model_hint.map(|s| s.to_string());
        task::spawn_blocking(move || {
            db.prompts()
                .create_template(&name, &tmpl, hint.as_deref(), max_tokens, metadata)
        })
        .await
        .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get the latest version of a template by name.
    pub async fn get_template(&self, name: &str) -> Result<PromptTemplate> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.prompts().get_template(&name))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// List all prompt templates.
    pub async fn list_templates(&self) -> Result<Vec<PromptTemplate>> {
        let db = self.inner.clone();
        task::spawn_blocking(move || db.prompts().list_templates())
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Render a prompt template with variable substitution.
    pub async fn render(&self, name: &str, vars: HashMap<String, String>) -> Result<String> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.prompts().render(&name, &vars))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Delete a template and all its versions.
    pub async fn delete_template(&self, name: &str) -> Result<()> {
        let db = self.inner.clone();
        let name = name.to_string();
        task::spawn_blocking(move || db.prompts().delete_template(&name))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}

// ── Async Label Store ──────────────────────────────────────────────────

/// Async wrapper for data label (privacy classification) operations.
pub struct AsyncLabelStore {
    inner: Arc<AgentDB>,
}

impl AsyncLabelStore {
    /// Tag a record with a label.
    pub async fn tag(
        &self,
        table_name: &str,
        record_id: &str,
        label: &str,
        tagged_by: Option<&str>,
    ) -> Result<()> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        let lbl = label.to_string();
        let by = tagged_by.map(|s| s.to_string());
        task::spawn_blocking(move || db.labels().tag(&table, &record, &lbl, by.as_deref()))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Remove a specific label from a record.
    pub async fn untag(&self, table_name: &str, record_id: &str, label: &str) -> Result<()> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        let lbl = label.to_string();
        task::spawn_blocking(move || db.labels().untag(&table, &record, &lbl))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Get all labels for a record.
    pub async fn get_labels(&self, table_name: &str, record_id: &str) -> Result<Vec<DataLabel>> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        task::spawn_blocking(move || db.labels().get_labels(&table, &record))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Check if a record has a specific label.
    pub async fn has_label(&self, table_name: &str, record_id: &str, label: &str) -> Result<bool> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        let lbl = label.to_string();
        task::spawn_blocking(move || db.labels().has_label(&table, &record, &lbl))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Find all records with a given label.
    pub async fn find_by_label(&self, label: &str, limit: Option<usize>) -> Result<Vec<DataLabel>> {
        let db = self.inner.clone();
        let lbl = label.to_string();
        task::spawn_blocking(move || db.labels().find_by_label(&lbl, limit))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }

    /// Clear all labels for a record.
    pub async fn clear_record(&self, table_name: &str, record_id: &str) -> Result<()> {
        let db = self.inner.clone();
        let table = table_name.to_string();
        let record = record_id.to_string();
        task::spawn_blocking(move || db.labels().clear_record(&table, &record))
            .await
            .map_err(|e| crate::error::AgentDbError::InvalidArgument(e.to_string()))?
    }
}
