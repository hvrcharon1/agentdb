//! # AgentDB — WASM bindings
//!
//! In-memory databases work today via wasm-pack. Persistent storage via OPFS
//! (Origin Private File System) is not yet implemented.
//!
//! ## Building (in-memory only)
//!
//! ```bash
//! cargo install wasm-pack
//! wasm-pack build --target web --features wasm
//! ```
//!
//! ## Usage in the browser
//!
//! ```js
//! import init, { WasmAgentDB } from './pkg/agentdb.js';
//! await init();
//! const db = WasmAgentDB.open_memory();
//! db.execute("CREATE TABLE notes (id TEXT PRIMARY KEY, body TEXT)");
//! const stats = JSON.parse(db.stats());
//! // { collections:0, vectors:0, nodes:0, edges:0, conversations:0, ... }
//! ```

use wasm_bindgen::prelude::*;

use crate::db::AgentDB;
use crate::vectors::{DistanceMetric, SearchOptions, VectorEntry};
use serde_json::Value;

/// WASM-exposed AgentDB handle.
///
/// All methods accept and return JSON strings to avoid complex
/// ownership issues across the WASM boundary.
#[wasm_bindgen]
pub struct WasmAgentDB {
    db: AgentDB,
}

#[wasm_bindgen]
impl WasmAgentDB {
    /// Open an in-memory AgentDB database.
    #[wasm_bindgen(js_name = open_memory)]
    pub fn open_memory() -> Result<WasmAgentDB, JsValue> {
        AgentDB::open(":memory:")
            .map(|db| WasmAgentDB { db })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute a raw SQL statement. Returns rows affected.
    pub fn execute(&self, sql: &str) -> Result<i64, JsValue> {
        self.db
            .execute(sql)
            .map(|n| n as i64)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Query and return all rows as a JSON array string.
    pub fn query_json(&self, sql: &str) -> Result<String, JsValue> {
        self.db
            .query_json(sql)
            .map(|rows| Value::Array(rows).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Upsert a vector. `metadata_json` may be an empty string.
    pub fn vector_upsert(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata_json: &str,
    ) -> Result<(), JsValue> {
        let meta: Option<Value> = if metadata_json.is_empty() {
            None
        } else {
            serde_json::from_str(metadata_json).ok()
        };
        let dim = vector.len();
        let col = self
            .db
            .vectors()
            .collection(collection, dim)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        col.upsert(VectorEntry {
            id: id.to_string(),
            vector,
            metadata: meta,
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search a vector collection. Returns a JSON array string.
    pub fn vector_search(
        &self,
        collection: &str,
        query: Vec<f32>,
        top_k: usize,
    ) -> Result<String, JsValue> {
        let dim = query.len();
        let col = self
            .db
            .vectors()
            .collection(collection, dim)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        col.search(
            &query,
            SearchOptions {
                top_k,
                metric: DistanceMetric::Cosine,
                filter: None,
            },
        )
        .map(|results| {
            let json: Vec<Value> = results
                .iter()
                .map(|r| serde_json::json!({ "id": r.id, "score": r.score }))
                .collect();
            Value::Array(json).to_string()
        })
        .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Add a node to the memory graph.
    pub fn graph_add_node(&self, id: &str, kind: &str, data_json: &str) -> Result<(), JsValue> {
        let data: Option<Value> = if data_json.is_empty() {
            None
        } else {
            serde_json::from_str(data_json).ok()
        };
        self.db
            .memory()
            .add_node(id, kind, data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Add a directed edge in the memory graph.
    pub fn graph_add_edge(
        &self,
        src: &str,
        dst: &str,
        relation: &str,
        weight: f64,
    ) -> Result<(), JsValue> {
        self.db
            .memory()
            .add_edge(src, dst, relation, weight)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Tool Registry ────────────────────────────────────────────────

    /// Register or update a tool. Returns JSON with tool ID.
    pub fn tool_register(
        &self,
        name: &str,
        description: &str,
        parameters_schema: &str,
        version: &str,
    ) -> Result<String, JsValue> {
        let desc = if description.is_empty() { None } else { Some(description.to_string()) };
        let schema: Option<Value> = if parameters_schema.is_empty() {
            None
        } else {
            serde_json::from_str(parameters_schema).ok()
        };
        let ver = if version.is_empty() { None } else { Some(version.to_string()) };
        self.db
            .tools()
            .register_tool(name, desc.as_deref(), schema, ver.as_deref())
            .map(|id| serde_json::json!({"id": id}).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// List all registered tools as a JSON array string.
    pub fn tool_list(&self) -> Result<String, JsValue> {
        self.db
            .tools()
            .list_tools()
            .map(|tools| {
                let arr: Vec<Value> = tools
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "id": t.id,
                            "name": t.name,
                            "description": t.description,
                            "parameters_schema": t.parameters_schema,
                            "version": t.version,
                        })
                    })
                    .collect();
                Value::Array(arr).to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Log a tool call. Returns JSON with tool call ID.
    pub fn tool_log_call(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &str,
        result: &str,
        error: &str,
        latency_ms: i64,
    ) -> Result<String, JsValue> {
        let sid = if session_id.is_empty() { None } else { Some(session_id) };
        let args: Option<Value> = if arguments.is_empty() {
            None
        } else {
            serde_json::from_str(arguments).ok()
        };
        let res: Option<Value> = if result.is_empty() {
            None
        } else {
            serde_json::from_str(result).ok()
        };
        let err = if error.is_empty() { None } else { Some(error) };
        self.db
            .tools()
            .log_tool_call(sid, tool_name, args, res, err, Some(latency_ms))
            .map(|id| serde_json::json!({"id": id}).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Audit Log ────────────────────────────────────────────────────

    /// Log an audit entry. Returns JSON with entry ID.
    pub fn audit_log(
        &self,
        actor: &str,
        action: &str,
        table_name: &str,
        record_id: &str,
        old_value: &str,
        new_value: &str,
        reason: &str,
    ) -> Result<String, JsValue> {
        let actor_opt = if actor.is_empty() { None } else { Some(actor) };
        let old: Option<Value> = if old_value.is_empty() {
            None
        } else {
            serde_json::from_str(old_value).ok()
        };
        let new_v: Option<Value> = if new_value.is_empty() {
            None
        } else {
            serde_json::from_str(new_value).ok()
        };
        let reason_opt = if reason.is_empty() { None } else { Some(reason) };
        self.db
            .audit()
            .log(actor_opt, action, table_name, record_id, old, new_v, reason_opt)
            .map(|id| serde_json::json!({"id": id}).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Query recent audit entries. Returns JSON array string.
    pub fn audit_query_recent(&self, limit: usize) -> Result<String, JsValue> {
        self.db
            .audit()
            .query_recent(limit)
            .map(|entries| {
                let arr: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "timestamp": e.timestamp,
                            "actor": e.actor,
                            "action": e.action,
                            "table_name": e.table_name,
                            "record_id": e.record_id,
                            "reason": e.reason,
                        })
                    })
                    .collect();
                Value::Array(arr).to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Context Window ───────────────────────────────────────────────

    /// Add a context entry. Returns JSON with entry ID.
    pub fn context_add(
        &self,
        session_id: &str,
        source_type: &str,
        source_id: &str,
        content_preview: &str,
        token_count: i64,
        relevance_score: f64,
        priority: i64,
    ) -> Result<String, JsValue> {
        let preview = if content_preview.is_empty() {
            None
        } else {
            Some(content_preview)
        };
        self.db
            .context()
            .add_entry(session_id, source_type, source_id, preview, token_count, relevance_score, priority)
            .map(|id| serde_json::json!({"id": id}).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Build a token-budgeted context window. Returns JSON array string.
    pub fn context_build_window(&self, session_id: &str, max_tokens: i64) -> Result<String, JsValue> {
        self.db
            .context()
            .build_window(session_id, max_tokens)
            .map(|entries| {
                let arr: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "session_id": e.session_id,
                            "source_type": e.source_type,
                            "source_id": e.source_id,
                            "content_preview": e.content_preview,
                            "token_count": e.token_count,
                            "relevance_score": e.relevance_score,
                            "priority": e.priority,
                        })
                    })
                    .collect();
                Value::Array(arr).to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Clear all context entries for a session.
    pub fn context_clear(&self, session_id: &str) -> Result<(), JsValue> {
        self.db
            .context()
            .clear_session(session_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Prompt Templates ─────────────────────────────────────────────

    /// Create a prompt template version. Returns JSON with template ID.
    pub fn prompt_create(
        &self,
        name: &str,
        template: &str,
        model_hint: &str,
        max_tokens: i64,
        metadata: &str,
    ) -> Result<String, JsValue> {
        let hint = if model_hint.is_empty() { None } else { Some(model_hint) };
        let max_t = if max_tokens <= 0 { None } else { Some(max_tokens) };
        let meta: Option<Value> = if metadata.is_empty() {
            None
        } else {
            serde_json::from_str(metadata).ok()
        };
        self.db
            .prompts()
            .create_template(name, template, hint, max_t, meta)
            .map(|id| serde_json::json!({"id": id}).to_string())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Render a prompt template. Returns the rendered string.
    pub fn prompt_render(&self, name: &str, vars_json: &str) -> Result<String, JsValue> {
        let vars: std::collections::HashMap<String, String> = if vars_json.is_empty() {
            std::collections::HashMap::new()
        } else {
            serde_json::from_str(vars_json).unwrap_or_default()
        };
        self.db
            .prompts()
            .render(name, &vars)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ── Data Labels (Privacy) ────────────────────────────────────────

    /// Tag a record with a label.
    pub fn label_tag(
        &self,
        table_name: &str,
        record_id: &str,
        label: &str,
        tagged_by: &str,
    ) -> Result<(), JsValue> {
        let by = if tagged_by.is_empty() { None } else { Some(tagged_by) };
        self.db
            .labels()
            .tag(table_name, record_id, label, by)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Remove a specific label from a record.
    pub fn label_untag(&self, table_name: &str, record_id: &str, label: &str) -> Result<(), JsValue> {
        self.db
            .labels()
            .untag(table_name, record_id, label)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get all labels for a record as a JSON array string.
    pub fn label_get(&self, table_name: &str, record_id: &str) -> Result<String, JsValue> {
        self.db
            .labels()
            .get_labels(table_name, record_id)
            .map(|labels| {
                let arr: Vec<Value> = labels
                    .iter()
                    .map(|l| {
                        serde_json::json!({
                            "table_name": l.table_name,
                            "record_id": l.record_id,
                            "label": l.label,
                            "tagged_by": l.tagged_by,
                            "tagged_at": l.tagged_at,
                        })
                    })
                    .collect();
                Value::Array(arr).to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Check if a record has a specific label.
    pub fn label_has(&self, table_name: &str, record_id: &str, label: &str) -> Result<bool, JsValue> {
        self.db
            .labels()
            .has_label(table_name, record_id, label)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return database stats as a JSON object string.
    pub fn stats(&self) -> Result<String, JsValue> {
        self.db
            .stats()
            .map(|s| {
                serde_json::json!({
                    "collections":      s.collections,
                    "vectors":          s.vectors,
                    "nodes":            s.nodes,
                    "edges":            s.edges,
                    "conversations":    s.conversations,
                    "messages":         s.messages,
                    "workflows":        s.workflows,
                    "workflowSteps":    s.workflow_steps,
                    "traces":           s.traces,
                    "tools":            s.tools,
                    "toolCalls":        s.tool_calls,
                    "auditEntries":     s.audit_entries,
                    "promptTemplates":  s.prompt_templates
                })
                .to_string()
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
