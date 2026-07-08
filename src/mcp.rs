//! # MCP (Model Context Protocol) Server Interface
//!
//! Implements the MCP JSON-RPC transport for AgentDB, exposing the database
//! as an MCP-compatible tool server. Supports:
//!
//! - `initialize` / `initialized` handshake
//! - `tools/list` — enumerate all AgentDB capabilities as MCP tools
//! - `tools/call` — invoke any AgentDB operation
//! - `resources/list` / `resources/read` — expose database stats and collections
//!
//! ## Usage
//!
//! ```rust,no_run
//! use agentdb::{AgentDB, mcp::McpServer};
//!
//! let db = AgentDB::open("agent.db").unwrap();
//! let server = McpServer::new(db);
//!
//! // Process a JSON-RPC request (from stdin, HTTP, WebSocket, etc.)
//! let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
//! let response = server.handle_message(request);
//! println!("{}", response);
//! ```

use crate::db::AgentDB;
use serde_json::{json, Value};
use std::collections::HashMap;

/// MCP server wrapping an AgentDB instance.
pub struct McpServer {
    db: AgentDB,
}

impl McpServer {
    /// Create a new MCP server backed by the given database.
    pub fn new(db: AgentDB) -> Self {
        Self { db }
    }

    /// Handle a single JSON-RPC message string and return the response.
    ///
    /// Returns `None` for notifications (messages without an `id` field),
    /// per JSON-RPC 2.0 spec: servers MUST NOT reply to notifications.
    pub fn handle_message(&self, input: &str) -> Option<String> {
        let req: Value = match serde_json::from_str(input) {
            Ok(v) => v,
            Err(e) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": format!("Parse error: {e}") }
                    })
                    .to_string(),
                );
            }
        };

        let is_notification = req.get("id").is_none_or(|v| v.is_null());
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req
            .get("params")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        // Notifications: no response per JSON-RPC 2.0 / MCP spec
        if is_notification {
            match method {
                "initialized" | "notifications/cancelled" | "notifications/progress" => {}
                _ => {}
            }
            return None;
        }

        let result = match method {
            "initialize" => self.handle_initialize(&params),
            "ping" => Ok(json!({})),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(&params),
            "prompts/list" => self.handle_prompts_list(),
            "prompts/get" => self.handle_prompts_get(&params),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };

        Some(match result {
            Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }).to_string(),
            Err((code, msg)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
                    .to_string()
            }
        })
    }

    fn handle_initialize(&self, _params: &Value) -> std::result::Result<Value, (i32, String)> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false }
            },
            "serverInfo": {
                "name": "agentdb",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    fn handle_tools_list(&self) -> std::result::Result<Value, (i32, String)> {
        Ok(json!({ "tools": self.tool_definitions() }))
    }

    fn handle_tools_call(&self, params: &Value) -> std::result::Result<Value, (i32, String)> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or((-32602, "Missing 'name' parameter".to_string()))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        let result = self.dispatch_tool(name, &arguments)?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": result.to_string()
            }]
        }))
    }

    fn handle_resources_list(&self) -> std::result::Result<Value, (i32, String)> {
        Ok(json!({
            "resources": [
                {
                    "uri": "agentdb://stats",
                    "name": "Database Statistics",
                    "description": "Current AgentDB database statistics",
                    "mimeType": "application/json"
                }
            ]
        }))
    }

    fn handle_resources_read(&self, params: &Value) -> std::result::Result<Value, (i32, String)> {
        let uri = params
            .get("uri")
            .and_then(|u| u.as_str())
            .ok_or((-32602, "Missing 'uri' parameter".to_string()))?;

        match uri {
            "agentdb://stats" => {
                let stats = self
                    .db
                    .stats()
                    .map_err(|e| (-32000, format!("Stats error: {e}")))?;
                Ok(json!({
                    "contents": [{
                        "uri": "agentdb://stats",
                        "mimeType": "application/json",
                        "text": serde_json::to_string(&stats).unwrap_or_default()
                    }]
                }))
            }
            _ => Err((-32002, format!("Resource not found: {uri}"))),
        }
    }

    fn handle_prompts_list(&self) -> std::result::Result<Value, (i32, String)> {
        let templates = self
            .db
            .prompts()
            .list_templates()
            .map_err(|e| (-32000, e.to_string()))?;
        let prompts: Vec<Value> = templates
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": format!("Prompt template '{}' v{}", t.name, t.version),
                    "arguments": [{
                        "name": "vars",
                        "description": "JSON object of template variables for {{placeholder}} substitution",
                        "required": false
                    }]
                })
            })
            .collect();
        // Deduplicate by name (list_templates returns all versions)
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<Value> = prompts
            .into_iter()
            .filter(|p| seen.insert(p["name"].as_str().unwrap_or("").to_string()))
            .collect();
        Ok(json!({ "prompts": unique }))
    }

    fn handle_prompts_get(&self, params: &Value) -> std::result::Result<Value, (i32, String)> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or((-32602, "Missing 'name' parameter".to_string()))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));
        let vars: HashMap<String, String> = args
            .get("vars")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let rendered = self
            .db
            .prompts()
            .render(name, &vars)
            .map_err(|e| (-32000, e.to_string()))?;
        Ok(json!({
            "description": format!("Rendered prompt template '{name}'"),
            "messages": [{
                "role": "user",
                "content": { "type": "text", "text": rendered }
            }]
        }))
    }

    fn dispatch_tool(&self, name: &str, args: &Value) -> std::result::Result<Value, (i32, String)> {
        let err = |e: crate::error::AgentDbError| (-32000, e.to_string());

        match name {
            "execute" => {
                let sql = get_str(args, "sql")?;
                let n = self.db.execute(sql).map_err(err)?;
                Ok(json!({ "rows_affected": n }))
            }
            "query" => {
                let sql = get_str(args, "sql")?;
                let rows = self.db.query_json(sql).map_err(err)?;
                Ok(Value::Array(rows))
            }
            "vector_upsert" => {
                let collection = get_str(args, "collection")?;
                let id = get_str(args, "id")?;
                let vector: Vec<f32> = args
                    .get("vector")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or((-32602, "Missing 'vector' array".to_string()))?;
                let metadata = args.get("metadata").cloned();
                let dim = vector.len();
                let col = self.db.vectors().collection(collection, dim).map_err(err)?;
                col.upsert(crate::vectors::VectorEntry {
                    id: id.to_string(),
                    vector,
                    metadata,
                })
                .map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "vector_search" => {
                let collection = get_str(args, "collection")?;
                let query: Vec<f32> = args
                    .get("query")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .ok_or((-32602, "Missing 'query' array".to_string()))?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let filter = args.get("filter").cloned();
                let dim = query.len();
                let col = self.db.vectors().collection(collection, dim).map_err(err)?;
                let results = col
                    .search(
                        &query,
                        crate::vectors::SearchOptions {
                            top_k,
                            metric: crate::vectors::DistanceMetric::Cosine,
                            filter,
                        },
                    )
                    .map_err(err)?;
                let arr: Vec<Value> = results
                    .iter()
                    .map(|r| json!({"id": r.id, "score": r.score, "metadata": r.metadata}))
                    .collect();
                Ok(Value::Array(arr))
            }
            "graph_add_node" => {
                let id = get_str(args, "id")?;
                let kind = get_str(args, "kind")?;
                let data = args.get("data").cloned();
                self.db.memory().add_node(id, kind, data).map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "graph_add_edge" => {
                let src = get_str(args, "src")?;
                let dst = get_str(args, "dst")?;
                let relation = get_str(args, "relation")?;
                let weight = args.get("weight").and_then(|v| v.as_f64()).unwrap_or(1.0);
                self.db
                    .memory()
                    .add_edge(src, dst, relation, weight)
                    .map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "graph_neighbors" => {
                let node_id = get_str(args, "node_id")?;
                let max_depth =
                    args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                let min_weight = args
                    .get("min_weight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let relation = args.get("relation").and_then(|v| v.as_str());
                let results = self
                    .db
                    .memory()
                    .neighbors(
                        node_id,
                        crate::memory::TraversalOptions {
                            max_depth,
                            min_weight: Some(min_weight),
                            relation: relation.map(|s| s.to_string()),
                        },
                    )
                    .map_err(err)?;
                let arr: Vec<Value> = results
                    .iter()
                    .map(|r| {
                        json!({"id": r.node.id, "kind": r.node.kind, "depth": r.depth, "weight": r.weight, "data": r.node.data})
                    })
                    .collect();
                Ok(Value::Array(arr))
            }
            "tool_register" => {
                let tool_name = get_str(args, "name")?;
                let description = args.get("description").and_then(|v| v.as_str());
                let schema = args.get("parameters_schema").cloned();
                let version = args.get("version").and_then(|v| v.as_str());
                let id = self
                    .db
                    .tools()
                    .register_tool(tool_name, description, schema, version)
                    .map_err(err)?;
                Ok(json!({ "id": id }))
            }
            "tool_list" => {
                let tools = self.db.tools().list_tools().map_err(err)?;
                let arr: Vec<Value> = tools
                    .iter()
                    .map(|t| {
                        json!({
                            "id": t.id, "name": t.name,
                            "description": t.description,
                            "parameters_schema": t.parameters_schema,
                            "version": t.version
                        })
                    })
                    .collect();
                Ok(Value::Array(arr))
            }
            "tool_log_call" => {
                let tool_name = get_str(args, "tool_name")?;
                let session_id = args.get("session_id").and_then(|v| v.as_str());
                let arguments = args.get("arguments").cloned();
                let result = args.get("result").cloned();
                let error = args.get("error").and_then(|v| v.as_str());
                let latency_ms = args.get("latency_ms").and_then(|v| v.as_i64()).unwrap_or(0);
                let id = self
                    .db
                    .tools()
                    .log_tool_call(
                        session_id,
                        tool_name,
                        arguments,
                        result,
                        error,
                        Some(latency_ms),
                    )
                    .map_err(err)?;
                Ok(json!({ "id": id }))
            }
            "audit_log" => {
                let action = get_str(args, "action")?;
                let table_name = get_str(args, "table_name")?;
                let record_id = get_str(args, "record_id")?;
                let actor = args.get("actor").and_then(|v| v.as_str());
                let old_value = args.get("old_value").cloned();
                let new_value = args.get("new_value").cloned();
                let reason = args.get("reason").and_then(|v| v.as_str());
                let id = self
                    .db
                    .audit()
                    .log(
                        actor, action, table_name, record_id, old_value, new_value, reason,
                    )
                    .map_err(err)?;
                Ok(json!({ "id": id }))
            }
            "audit_query_recent" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
                let entries = self.db.audit().query_recent(Some(limit)).map_err(err)?;
                let arr: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id, "timestamp": e.timestamp, "actor": e.actor,
                            "action": e.action, "table_name": e.table_name,
                            "record_id": e.record_id, "reason": e.reason
                        })
                    })
                    .collect();
                Ok(Value::Array(arr))
            }
            "context_add" => {
                let session_id = get_str(args, "session_id")?;
                let source_type = get_str(args, "source_type")?;
                let source_id = get_str(args, "source_id")?;
                let content_preview = args.get("content_preview").and_then(|v| v.as_str());
                let token_count = args
                    .get("token_count")
                    .and_then(|v| v.as_i64())
                    .ok_or((-32602, "Missing 'token_count'".to_string()))?;
                let relevance_score = args
                    .get("relevance_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5);
                let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
                let id = self
                    .db
                    .context()
                    .add_entry(
                        session_id,
                        source_type,
                        source_id,
                        content_preview,
                        token_count,
                        relevance_score,
                        priority,
                    )
                    .map_err(err)?;
                Ok(json!({ "id": id }))
            }
            "context_build_window" => {
                let session_id = get_str(args, "session_id")?;
                let max_tokens = args
                    .get("max_tokens")
                    .and_then(|v| v.as_i64())
                    .ok_or((-32602, "Missing 'max_tokens'".to_string()))?;
                let entries = self
                    .db
                    .context()
                    .build_window(session_id, max_tokens)
                    .map_err(err)?;
                let arr: Vec<Value> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id, "source_type": e.source_type,
                            "source_id": e.source_id, "content_preview": e.content_preview,
                            "token_count": e.token_count, "priority": e.priority
                        })
                    })
                    .collect();
                Ok(Value::Array(arr))
            }
            "context_clear" => {
                let session_id = get_str(args, "session_id")?;
                self.db.context().clear_session(session_id).map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "prompt_create" => {
                let name = get_str(args, "name")?;
                let template = get_str(args, "template")?;
                let model_hint = args.get("model_hint").and_then(|v| v.as_str());
                let max_tokens = args.get("max_tokens").and_then(|v| v.as_i64());
                let metadata = args.get("metadata").cloned();
                let id = self
                    .db
                    .prompts()
                    .create_template(name, template, model_hint, max_tokens, metadata)
                    .map_err(err)?;
                Ok(json!({ "id": id }))
            }
            "prompt_render" => {
                let name = get_str(args, "name")?;
                let vars: HashMap<String, String> = args
                    .get("vars")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let rendered = self.db.prompts().render(name, &vars).map_err(err)?;
                Ok(json!({ "text": rendered }))
            }
            "label_tag" => {
                let table_name = get_str(args, "table_name")?;
                let record_id = get_str(args, "record_id")?;
                let label = get_str(args, "label")?;
                let tagged_by = args.get("tagged_by").and_then(|v| v.as_str());
                self.db
                    .labels()
                    .tag(table_name, record_id, label, tagged_by)
                    .map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "label_untag" => {
                let table_name = get_str(args, "table_name")?;
                let record_id = get_str(args, "record_id")?;
                let label = get_str(args, "label")?;
                self.db
                    .labels()
                    .untag(table_name, record_id, label)
                    .map_err(err)?;
                Ok(json!({ "ok": true }))
            }
            "label_get" => {
                let table_name = get_str(args, "table_name")?;
                let record_id = get_str(args, "record_id")?;
                let labels = self
                    .db
                    .labels()
                    .get_labels(table_name, record_id)
                    .map_err(err)?;
                let arr: Vec<Value> = labels
                    .iter()
                    .map(|l| {
                        json!({
                            "label": l.label, "tagged_by": l.tagged_by,
                            "tagged_at": l.tagged_at
                        })
                    })
                    .collect();
                Ok(Value::Array(arr))
            }
            "label_has" => {
                let table_name = get_str(args, "table_name")?;
                let record_id = get_str(args, "record_id")?;
                let label = get_str(args, "label")?;
                let has = self
                    .db
                    .labels()
                    .has_label(table_name, record_id, label)
                    .map_err(err)?;
                Ok(json!({ "has": has }))
            }
            "stats" => {
                let stats = self.db.stats().map_err(err)?;
                Ok(json!({
                    "collections": stats.collections,
                    "vectors": stats.vectors,
                    "nodes": stats.nodes,
                    "edges": stats.edges,
                    "conversations": stats.conversations,
                    "messages": stats.messages,
                    "workflows": stats.workflows,
                    "workflow_steps": stats.workflow_steps,
                    "traces": stats.traces,
                    "tools": stats.tools,
                    "tool_calls": stats.tool_calls,
                    "audit_entries": stats.audit_entries,
                    "prompt_templates": stats.prompt_templates
                }))
            }
            _ => Err((-32601, format!("Unknown tool: {name}"))),
        }
    }

    fn tool_definitions(&self) -> Value {
        json!([
            tool_def(
                "execute",
                "Execute a raw SQL statement (DDL/DML)",
                json!({
                    "type": "object",
                    "properties": { "sql": { "type": "string", "description": "SQL statement" } },
                    "required": ["sql"]
                })
            ),
            tool_def(
                "query",
                "Execute a SELECT and return rows as JSON",
                json!({
                    "type": "object",
                    "properties": { "sql": { "type": "string", "description": "SELECT statement" } },
                    "required": ["sql"]
                })
            ),
            tool_def(
                "vector_upsert",
                "Insert or update a vector embedding",
                json!({
                    "type": "object",
                    "properties": {
                        "collection": { "type": "string" },
                        "id": { "type": "string" },
                        "vector": { "type": "array", "items": { "type": "number" } },
                        "metadata": { "type": "object" }
                    },
                    "required": ["collection", "id", "vector"]
                })
            ),
            tool_def(
                "vector_search",
                "Approximate nearest-neighbor search",
                json!({
                    "type": "object",
                    "properties": {
                        "collection": { "type": "string" },
                        "query": { "type": "array", "items": { "type": "number" } },
                        "top_k": { "type": "integer", "default": 10 },
                        "filter": { "type": "object" }
                    },
                    "required": ["collection", "query"]
                })
            ),
            tool_def(
                "graph_add_node",
                "Add or update a memory graph node",
                json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "kind": { "type": "string" },
                        "data": { "type": "object" }
                    },
                    "required": ["id", "kind"]
                })
            ),
            tool_def(
                "graph_add_edge",
                "Add or update a directed graph edge",
                json!({
                    "type": "object",
                    "properties": {
                        "src": { "type": "string" },
                        "dst": { "type": "string" },
                        "relation": { "type": "string" },
                        "weight": { "type": "number", "default": 1.0 }
                    },
                    "required": ["src", "dst", "relation"]
                })
            ),
            tool_def(
                "graph_neighbors",
                "Traverse the memory graph from a node",
                json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string" },
                        "max_depth": { "type": "integer", "default": 2 },
                        "min_weight": { "type": "number", "default": 0.0 },
                        "relation": { "type": "string" }
                    },
                    "required": ["node_id"]
                })
            ),
            tool_def(
                "tool_register",
                "Register or update a tool definition",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "parameters_schema": { "type": "object" },
                        "version": { "type": "string" }
                    },
                    "required": ["name"]
                })
            ),
            tool_def(
                "tool_list",
                "List all registered tools",
                json!({
                    "type": "object", "properties": {}
                })
            ),
            tool_def(
                "tool_log_call",
                "Log a tool invocation",
                json!({
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string" },
                        "session_id": { "type": "string" },
                        "arguments": { "type": "object" },
                        "result": { "type": "object" },
                        "error": { "type": "string" },
                        "latency_ms": { "type": "integer" }
                    },
                    "required": ["tool_name"]
                })
            ),
            tool_def(
                "audit_log",
                "Append an entry to the audit log",
                json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string" },
                        "table_name": { "type": "string" },
                        "record_id": { "type": "string" },
                        "actor": { "type": "string" },
                        "old_value": { "type": "object" },
                        "new_value": { "type": "object" },
                        "reason": { "type": "string" }
                    },
                    "required": ["action", "table_name", "record_id"]
                })
            ),
            tool_def(
                "audit_query_recent",
                "Query recent audit log entries",
                json!({
                    "type": "object",
                    "properties": { "limit": { "type": "integer", "default": 100 } }
                })
            ),
            tool_def(
                "context_add",
                "Add an entry to the context window",
                json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string" },
                        "source_type": { "type": "string" },
                        "source_id": { "type": "string" },
                        "content_preview": { "type": "string" },
                        "token_count": { "type": "integer" },
                        "relevance_score": { "type": "number" },
                        "priority": { "type": "integer" }
                    },
                    "required": ["session_id", "source_type", "source_id", "token_count"]
                })
            ),
            tool_def(
                "context_build_window",
                "Build a token-budgeted context window",
                json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string" },
                        "max_tokens": { "type": "integer" }
                    },
                    "required": ["session_id", "max_tokens"]
                })
            ),
            tool_def(
                "context_clear",
                "Clear all context entries for a session",
                json!({
                    "type": "object",
                    "properties": { "session_id": { "type": "string" } },
                    "required": ["session_id"]
                })
            ),
            tool_def(
                "prompt_create",
                "Create a new prompt template version",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "template": { "type": "string" },
                        "model_hint": { "type": "string" },
                        "max_tokens": { "type": "integer" },
                        "metadata": { "type": "object" }
                    },
                    "required": ["name", "template"]
                })
            ),
            tool_def(
                "prompt_render",
                "Render a prompt template with variables",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "vars": { "type": "object", "additionalProperties": { "type": "string" } }
                    },
                    "required": ["name"]
                })
            ),
            tool_def(
                "label_tag",
                "Tag a record with a classification label",
                json!({
                    "type": "object",
                    "properties": {
                        "table_name": { "type": "string" },
                        "record_id": { "type": "string" },
                        "label": { "type": "string" },
                        "tagged_by": { "type": "string" }
                    },
                    "required": ["table_name", "record_id", "label"]
                })
            ),
            tool_def(
                "label_untag",
                "Remove a label from a record",
                json!({
                    "type": "object",
                    "properties": {
                        "table_name": { "type": "string" },
                        "record_id": { "type": "string" },
                        "label": { "type": "string" }
                    },
                    "required": ["table_name", "record_id", "label"]
                })
            ),
            tool_def(
                "label_get",
                "Get all labels for a record",
                json!({
                    "type": "object",
                    "properties": {
                        "table_name": { "type": "string" },
                        "record_id": { "type": "string" }
                    },
                    "required": ["table_name", "record_id"]
                })
            ),
            tool_def(
                "label_has",
                "Check if a record has a specific label",
                json!({
                    "type": "object",
                    "properties": {
                        "table_name": { "type": "string" },
                        "record_id": { "type": "string" },
                        "label": { "type": "string" }
                    },
                    "required": ["table_name", "record_id", "label"]
                })
            ),
            tool_def(
                "stats",
                "Get database-wide statistics",
                json!({
                    "type": "object", "properties": {}
                })
            ),
        ])
    }
}

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn get_str<'a>(args: &'a Value, key: &str) -> std::result::Result<&'a str, (i32, String)> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or((-32602, format!("Missing required parameter: '{key}'")))
}
