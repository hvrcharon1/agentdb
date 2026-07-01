/*
 * AgentDB — C FFI Header
 *
 * Auto-generated from src/ffi.rs. Do not edit manually.
 * Regenerate with: cbindgen --config cbindgen.toml --output include/agentdb.h
 *
 * Memory contract:
 *   - Strings returned by agentdb_* functions are heap-allocated.
 *     Free them with agentdb_free_string() — never with free().
 *   - Input const char* pointers are borrowed for the call duration only.
 *   - AgentDbHandle is opaque. Close with agentdb_close().
 */

#ifndef AGENTDB_H
#define AGENTDB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque database handle. */
typedef struct AgentDbHandle AgentDbHandle;

/* ── Lifecycle ─────────────────────────────────────────────────────── */

AgentDbHandle* agentdb_open(const char* path);
void           agentdb_close(AgentDbHandle* handle);

/* ── Error handling ────────────────────────────────────────────────── */

char* agentdb_last_error(void);
void  agentdb_free_string(char* ptr);

/* ── SQL ───────────────────────────────────────────────────────────── */

int64_t agentdb_execute(AgentDbHandle* handle, const char* sql);
char*   agentdb_query_json(AgentDbHandle* handle, const char* sql);

/* ── Vector store ──────────────────────────────────────────────────── */

int   agentdb_vector_upsert(AgentDbHandle* handle, const char* collection,
          const char* id, const float* vector, size_t dim, const char* metadata);
char* agentdb_vector_search(AgentDbHandle* handle, const char* collection,
          const float* query, size_t dim, size_t top_k, const char* filter_json);

/* ── Memory graph ──────────────────────────────────────────────────── */

int   agentdb_graph_add_node(AgentDbHandle* handle, const char* id,
          const char* kind, const char* data_json);
int   agentdb_graph_add_edge(AgentDbHandle* handle, const char* src,
          const char* dst, const char* relation, double weight);
char* agentdb_graph_neighbors(AgentDbHandle* handle, const char* node_id,
          size_t max_depth, double min_weight, const char* relation);

/* ── Full-text search ──────────────────────────────────────────────── */

int   agentdb_fts_index(AgentDbHandle* handle, const char* collection,
          const char* vec_id, const char* collection_id, const char* text);
char* agentdb_fts_search(AgentDbHandle* handle, const char* collection,
          const char* query, size_t top_k);

/* ── Hybrid query ──────────────────────────────────────────────────── */

char* agentdb_hybrid_query(AgentDbHandle* handle, const char* anchor_node,
          const float* embedding, size_t dim, const char* collection,
          size_t graph_depth, size_t top_k, double alpha,
          const char* filter_json);

/* ── Stats ─────────────────────────────────────────────────────────── */

char* agentdb_stats(AgentDbHandle* handle);

/* ── Conversations ─────────────────────────────────────────────────── */

int   agentdb_conversation_create(AgentDbHandle* handle, const char* id,
          const char* title, const char* metadata);
char* agentdb_conversation_add_message(AgentDbHandle* handle,
          const char* conversation_id, const char* role,
          const char* content, const char* metadata);
char* agentdb_conversation_get_messages(AgentDbHandle* handle,
          const char* conversation_id, size_t limit);
char* agentdb_conversation_list(AgentDbHandle* handle);
int   agentdb_conversation_delete(AgentDbHandle* handle, const char* id);

/* ── Additional vector operations ──────────────────────────────────── */

int   agentdb_vector_delete(AgentDbHandle* handle, const char* collection,
          const char* id);
int   agentdb_drop_collection(AgentDbHandle* handle, const char* collection);
int   agentdb_reindex(AgentDbHandle* handle, const char* collection);

/* ── Additional graph operations ───────────────────────────────────── */

char* agentdb_graph_get_node(AgentDbHandle* handle, const char* id);
int   agentdb_graph_delete_node(AgentDbHandle* handle, const char* id);
int   agentdb_graph_delete_edge(AgentDbHandle* handle, const char* src,
          const char* dst, const char* relation);

/* ── Additional FTS operations ─────────────────────────────────────── */

int   agentdb_fts_delete(AgentDbHandle* handle, const char* collection,
          const char* vec_id);
int   agentdb_fts_optimize(AgentDbHandle* handle, const char* collection);

/* ── Workflows ─────────────────────────────────────────────────────── */

int   agentdb_workflow_create(AgentDbHandle* handle, const char* id,
          const char* name, const char* input, const char* metadata);
char* agentdb_workflow_add_step(AgentDbHandle* handle,
          const char* workflow_id, const char* name, const char* input);
int   agentdb_workflow_update_step(AgentDbHandle* handle,
          const char* step_id, const char* status,
          const char* output, const char* error);
int   agentdb_workflow_complete(AgentDbHandle* handle,
          const char* id, const char* output);
int   agentdb_workflow_fail(AgentDbHandle* handle,
          const char* id, const char* error);
char* agentdb_workflow_get(AgentDbHandle* handle, const char* id);
char* agentdb_workflow_list(AgentDbHandle* handle, const char* status_filter);

/* ── Traces ────────────────────────────────────────────────────────── */

char* agentdb_trace_add(AgentDbHandle* handle, const char* session_id,
          const char* parent_id, const char* trace_type,
          const char* content, const char* metadata);
char* agentdb_trace_get_by_session(AgentDbHandle* handle, const char* session_id);
char* agentdb_trace_get_tree(AgentDbHandle* handle, const char* root_id);

#ifdef __cplusplus
}
#endif

#endif /* AGENTDB_H */
