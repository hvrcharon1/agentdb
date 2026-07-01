/**
 * AgentDB — C API
 *
 * Single-file embedded database for AI agents.
 * SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs.
 *
 * Memory contract:
 *   - All strings returned by agentdb_* functions are heap-allocated.
 *     Free them with agentdb_free_string(), never with free() or delete.
 *   - Input const char* pointers are borrowed for the duration of the call.
 *   - AgentDbHandle* is opaque. Close with agentdb_close().
 *
 * https://github.com/hvrcharon1/agentdb
 */


#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct AgentDbHandle AgentDbHandle;

/**
 * Return the last error message as a UTF-8 C string, or NULL if none.
 * The returned pointer must be freed with `agentdb_free_string`.
 */
char *agentdb_last_error(void);

/**
 * Free a string previously returned by AgentDB.
 *
 * # Safety
 * `ptr` must be a pointer previously returned by an `agentdb_*` function
 * and must not have been freed already.
 */
void agentdb_free_string(char *ptr);

/**
 * Open or create an AgentDB database at `path`.
 * Use `":memory:"` for an in-memory database.
 *
 * Returns an opaque handle on success, or NULL on failure.
 * Check `agentdb_last_error()` on NULL.
 */
struct AgentDbHandle *agentdb_open(const char *path);

/**
 * Close and free an AgentDB handle.
 *
 * # Safety
 * `handle` must be a valid pointer previously returned by `agentdb_open`
 * and must not be used after this call.
 */
void agentdb_close(struct AgentDbHandle *handle);

/**
 * Execute a raw SQL statement (no parameters).
 *
 * Returns the number of rows affected, or -1 on error.
 */
int64_t agentdb_execute(struct AgentDbHandle *handle, const char *sql);

/**
 * Query and return all rows as a JSON array string.
 *
 * Returns a heap-allocated JSON string — free with `agentdb_free_string`.
 * Returns NULL on error; check `agentdb_last_error()`.
 */
char *agentdb_query_json(struct AgentDbHandle *handle, const char *sql);

/**
 * Query with positional parameters and return all rows as a JSON array string.
 *
 * `params_json` is a JSON array of parameter values (e.g. `["alice", 42]`).
 * Returns a heap-allocated JSON string — free with `agentdb_free_string`.
 * Returns NULL on error; check `agentdb_last_error()`.
 */
char *agentdb_query_json_params(struct AgentDbHandle *handle,
                                const char *sql,
                                const char *params_json);

/**
 * Upsert a single vector into `collection` (created if absent).
 *
 * `id`       — unique string identifier for this vector
 * `vector`   — pointer to `dim` f32 values
 * `dim`      — number of dimensions
 * `metadata` — JSON string (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_vector_upsert(struct AgentDbHandle *handle,
                              const char *collection,
                              const char *id,
                              const float *vector,
                              uintptr_t dim,
                              const char *metadata);

/**
 * Search a vector collection and return results as a JSON array.
 *
 * `query`      — pointer to `dim` f32 query values
 * `dim`        — number of dimensions
 * `top_k`      — maximum results to return
 * `filter_json`— MongoDB-style metadata filter JSON string (may be NULL)
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 * Returns NULL on error.
 */
char *agentdb_vector_search(struct AgentDbHandle *handle,
                            const char *collection,
                            const float *query,
                            uintptr_t dim,
                            uintptr_t top_k,
                            const char *filter_json);

/**
 * Add or update a node in the memory graph.
 *
 * `id`       — unique node identifier
 * `kind`     — node type label (e.g. "session", "concept")
 * `data_json`— JSON metadata string (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_graph_add_node(struct AgentDbHandle *handle,
                               const char *id,
                               const char *kind,
                               const char *data_json);

/**
 * Add or update a directed weighted edge in the memory graph.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_graph_add_edge(struct AgentDbHandle *handle,
                               const char *src,
                               const char *dst,
                               const char *relation,
                               double weight);

/**
 * Traverse the memory graph from `node_id` and return results as JSON.
 *
 * `max_depth`  — maximum hops from the anchor node
 * `min_weight` — minimum edge weight to traverse (0.0 = all edges)
 * `relation`   — optional edge relation filter (NULL = all relations)
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_graph_neighbors(struct AgentDbHandle *handle,
                              const char *node_id,
                              uintptr_t max_depth,
                              double min_weight,
                              const char *relation);

/**
 * Index a text document for full-text search.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_fts_index(struct AgentDbHandle *handle,
                          const char *collection,
                          const char *vec_id,
                          const char *collection_id,
                          const char *text);

/**
 * Full-text search over a collection, returning results as JSON.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_fts_search(struct AgentDbHandle *handle,
                         const char *collection,
                         const char *query,
                         uintptr_t top_k);

/**
 * Run a hybrid graph + vector query and return results as JSON.
 *
 * `anchor_node`  — graph traversal start node id
 * `embedding`    — pointer to `dim` f32 query values
 * `dim`          — embedding dimensions
 * `collection`   — vector collection name
 * `graph_depth`  — max hops from anchor
 * `top_k`        — results to return
 * `alpha`        — 0.0 = pure graph, 1.0 = pure vector
 * `filter_json`  — optional MongoDB-style metadata filter JSON (may be NULL)
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_hybrid_query(struct AgentDbHandle *handle,
                           const char *anchor_node,
                           const float *embedding,
                           uintptr_t dim,
                           const char *collection,
                           uintptr_t graph_depth,
                           uintptr_t top_k,
                           double alpha,
                           const char *filter_json);

/**
 * Return database statistics as a JSON object.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_stats(struct AgentDbHandle *handle);

/**
 * Create a new conversation.
 *
 * `id`       — unique conversation identifier
 * `title`    — optional title (may be NULL)
 * `metadata` — optional JSON string (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_conversation_create(struct AgentDbHandle *handle,
                                    const char *id,
                                    const char *title,
                                    const char *metadata);

/**
 * Add a message to an existing conversation.
 *
 * Returns the new message ID as a heap-allocated string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_conversation_add_message(struct AgentDbHandle *handle,
                                       const char *conversation_id,
                                       const char *role,
                                       const char *content,
                                       const char *metadata);

/**
 * Get messages for a conversation as a JSON array.
 *
 * `limit` — maximum messages to return (0 = all).
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_conversation_get_messages(struct AgentDbHandle *handle,
                                        const char *conversation_id,
                                        uintptr_t limit);

/**
 * List all conversations as a JSON array.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_conversation_list(struct AgentDbHandle *handle);

/**
 * Delete a conversation and all its messages.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_conversation_delete(struct AgentDbHandle *handle, const char *id);

/**
 * Create a new workflow in `pending` status.
 *
 * `id`       — unique workflow identifier
 * `name`     — human-readable workflow name
 * `input`    — optional JSON input (may be NULL)
 * `metadata` — optional JSON metadata (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_workflow_create(struct AgentDbHandle *handle,
                                const char *id,
                                const char *name,
                                const char *input,
                                const char *metadata);

/**
 * Add a step to an existing workflow.
 *
 * Returns the step ID as a heap-allocated string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_workflow_add_step(struct AgentDbHandle *handle,
                                const char *workflow_id,
                                const char *name,
                                const char *input);

/**
 * Update a workflow step's status, output, and/or error.
 *
 * `status` — new status string ("running", "completed", "failed")
 * `output` — optional JSON output (may be NULL)
 * `error`  — optional error message (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_workflow_update_step(struct AgentDbHandle *handle,
                                     const char *step_id,
                                     const char *status,
                                     const char *output,
                                     const char *error);

/**
 * Mark a workflow as completed with optional output.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_workflow_complete(struct AgentDbHandle *handle, const char *id, const char *output);

/**
 * Mark a workflow as failed with an optional error message.
 *
 * `error` — optional error string (may be NULL)
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_workflow_fail(struct AgentDbHandle *handle, const char *id, const char *error);

/**
 * Get a workflow and its steps as a JSON object.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_workflow_get(struct AgentDbHandle *handle, const char *id);

/**
 * List workflows as a JSON array, optionally filtered by status.
 *
 * `status_filter` — status to filter by (may be NULL for all).
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_workflow_list(struct AgentDbHandle *handle, const char *status_filter);

/**
 * Record a new reasoning trace entry.
 *
 * Returns the trace ID as a heap-allocated string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_trace_add(struct AgentDbHandle *handle,
                        const char *session_id,
                        const char *parent_id,
                        const char *trace_type,
                        const char *content,
                        const char *metadata);

/**
 * Get all traces for a session as a JSON array.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_trace_get_by_session(struct AgentDbHandle *handle, const char *session_id);

/**
 * Delete a single vector from a collection by ID.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_vector_delete(struct AgentDbHandle *handle,
                              const char *collection,
                              const char *id,
                              uintptr_t dim);

/**
 * Drop an entire vector collection and all its vectors.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_drop_collection(struct AgentDbHandle *handle, const char *collection);

/**
 * Rebuild the HNSW index for a collection from stored vectors.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_reindex(struct AgentDbHandle *handle, const char *collection, uintptr_t dim);

/**
 * Get a graph node as a JSON object, or NULL if not found.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_graph_get_node(struct AgentDbHandle *handle, const char *id);

/**
 * Delete a graph node (and all its connected edges via CASCADE).
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_graph_delete_node(struct AgentDbHandle *handle, const char *id);

/**
 * Delete the directed edge from `src` to `dst` with the given `relation`.
 *
 * Returns 0 on success, -1 on error (including edge not found).
 */
int32_t agentdb_graph_delete_edge(struct AgentDbHandle *handle,
                                  const char *src,
                                  const char *dst,
                                  const char *relation);

/**
 * Delete a document from the FTS index.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_fts_delete(struct AgentDbHandle *handle,
                           const char *collection,
                           const char *vec_id);

/**
 * Merge FTS index segments for faster queries (optimize).
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_fts_optimize(struct AgentDbHandle *handle, const char *collection);

/**
 * Register or update a tool definition.
 *
 * `name`              — unique tool name
 * `description`       — human-readable description (may be NULL)
 * `parameters_schema` — JSON Schema string (may be NULL)
 * `version`           — semver version string (may be NULL, defaults to "1.0.0")
 *
 * Returns the tool ID as a heap-allocated string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_tool_register(struct AgentDbHandle *handle,
                            const char *name,
                            const char *description,
                            const char *parameters_schema,
                            const char *version);

/**
 * List all registered tools as a JSON array.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_tool_list(struct AgentDbHandle *handle);

/**
 * Log a tool call invocation.
 *
 * `session_id`    — optional session context (may be NULL)
 * `tool_name`     — name of the tool called
 * `arguments`     — JSON arguments (may be NULL)
 * `result`        — JSON result (may be NULL)
 * `error`         — error message (may be NULL)
 * `latency_ms`    — execution time in milliseconds (-1 if unknown)
 *
 * Returns the tool call ID as a heap-allocated string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_tool_log_call(struct AgentDbHandle *handle,
                            const char *session_id,
                            const char *tool_name,
                            const char *arguments,
                            const char *result,
                            const char *error,
                            int64_t latency_ms);

/**
 * Append an entry to the immutable audit log.
 *
 * `actor`      — who performed the action (may be NULL)
 * `action`     — action type (e.g. "insert", "update", "delete")
 * `table_name` — target table
 * `record_id`  — target record ID
 * `old_value`  — JSON of previous state (may be NULL)
 * `new_value`  — JSON of new state (may be NULL)
 * `reason`     — human-readable reason (may be NULL)
 *
 * Returns the audit entry ID, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_audit_log(struct AgentDbHandle *handle,
                        const char *actor,
                        const char *action,
                        const char *table_name,
                        const char *record_id,
                        const char *old_value,
                        const char *new_value,
                        const char *reason);

/**
 * Query recent audit log entries as a JSON array.
 *
 * `limit` — max entries to return (0 = default 100).
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_audit_query_recent(struct AgentDbHandle *handle, uintptr_t limit);

/**
 * Add an entry to the context window for a session.
 *
 * Returns the entry ID, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_context_add(struct AgentDbHandle *handle,
                          const char *session_id,
                          const char *source_type,
                          const char *source_id,
                          const char *content_preview,
                          int64_t token_count,
                          double relevance_score,
                          int64_t priority);

/**
 * Build a token-budgeted context window for a session.
 *
 * Returns entries as a JSON array, filling up to `max_tokens`.
 * Free with `agentdb_free_string`.
 */
char *agentdb_context_build_window(struct AgentDbHandle *handle,
                                   const char *session_id,
                                   int64_t max_tokens);

/**
 * Clear all context entries for a session.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_context_clear(struct AgentDbHandle *handle, const char *session_id);

/**
 * Create a new version of a prompt template.
 *
 * `name`      — template name (versions auto-increment per name)
 * `template`  — template body with {{placeholder}} syntax
 * `model_hint`— suggested model (may be NULL)
 * `max_tokens`— suggested max tokens (-1 if not set)
 * `metadata`  — optional JSON metadata (may be NULL)
 *
 * Returns the template ID, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_prompt_create(struct AgentDbHandle *handle,
                            const char *name,
                            const char *template_,
                            const char *model_hint,
                            int64_t max_tokens,
                            const char *metadata);

/**
 * Render a prompt template with variable substitution.
 *
 * `name`      — template name (uses latest version)
 * `vars_json` — JSON object of key-value pairs for {{placeholder}} substitution
 *
 * Returns the rendered string, or NULL on error.
 * Free with `agentdb_free_string`.
 */
char *agentdb_prompt_render(struct AgentDbHandle *handle, const char *name, const char *vars_json);

/**
 * Tag a record with a privacy/classification label.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_label_tag(struct AgentDbHandle *handle,
                          const char *table_name,
                          const char *record_id,
                          const char *label,
                          const char *tagged_by);

/**
 * Remove a specific label from a record.
 *
 * Returns 0 on success, -1 on error.
 */
int32_t agentdb_label_untag(struct AgentDbHandle *handle,
                            const char *table_name,
                            const char *record_id,
                            const char *label);

/**
 * Get all labels for a record as a JSON array.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_label_get(struct AgentDbHandle *handle,
                        const char *table_name,
                        const char *record_id);

/**
 * Check if a record has a specific label.
 *
 * Returns 1 if true, 0 if false, -1 on error.
 */
int32_t agentdb_label_has(struct AgentDbHandle *handle,
                          const char *table_name,
                          const char *record_id,
                          const char *label);

/**
 * Get a trace subtree as a JSON array.
 *
 * Returns heap-allocated JSON string — free with `agentdb_free_string`.
 */
char *agentdb_trace_get_tree(struct AgentDbHandle *handle, const char *root_id);
