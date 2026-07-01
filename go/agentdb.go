// Package agentdb provides Go bindings for the AgentDB embedded database.
//
// AgentDB combines SQLite, a vector store, a memory graph, and full-text
// search into a single embeddable engine for AI agent workloads.
//
// Prerequisites:
//
//	cargo build --release --features ffi --lib
//	# produces target/release/libagentdb.so (Linux)
//	#             target/release/libagentdb.dylib (macOS)
//	#             target/release/agentdb.dll    (Windows)
//
// Place the shared library and agentdb.h on your compiler/linker search path,
// then build normally:
//
//	go build ./...
package agentdb

/*
#cgo LDFLAGS: -lagentdb
#include "../include/agentdb.h"
#include <stdlib.h>
*/
import "C"

import (
	"encoding/json"
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

// ── Error helpers ────────────────────────────────────────────────────────────

// lastError retrieves the pending error string from the native library and
// clears it. Returns a non-nil error if one is set, otherwise a fallback.
func lastError(fallback string) error {
	ptr := C.agentdb_last_error()
	if ptr == nil {
		return errors.New(fallback)
	}
	msg := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return errors.New(msg)
}

// ── Types ────────────────────────────────────────────────────────────────────

// DB is an open AgentDB database handle. It is safe to call from multiple
// goroutines as long as the underlying SQLite connection is compiled in
// serialized or WAL mode (the default for libagentdb).
//
// Always call Close when finished to release native resources.
type DB struct {
	handle *C.AgentDbHandle
}

// Stats holds the snapshot statistics returned by [DB.Stats].
type Stats struct {
	Collections     int64 `json:"collections"`
	Vectors         int64 `json:"vectors"`
	Nodes           int64 `json:"nodes"`
	Edges           int64 `json:"edges"`
	Conversations   int64 `json:"conversations"`
	Messages        int64 `json:"messages"`
	Workflows       int64 `json:"workflows"`
	WorkflowSteps   int64 `json:"workflow_steps"`
	Traces          int64 `json:"traces"`
	Tools           int64 `json:"tools"`
	ToolCalls       int64 `json:"tool_calls"`
	AuditEntries    int64 `json:"audit_entries"`
	PromptTemplates int64 `json:"prompt_templates"`
}

// VectorResult is one entry returned by [DB.VectorSearch].
type VectorResult struct {
	ID       string          `json:"id"`
	Score    float64         `json:"score"`
	Metadata json.RawMessage `json:"metadata"`
}

// GraphNode is one entry returned by [DB.GraphNeighbors].
type GraphNode struct {
	ID     string          `json:"id"`
	Kind   string          `json:"kind"`
	Depth  int             `json:"depth"`
	Weight float64         `json:"weight"`
	Data   json.RawMessage `json:"data"`
}

// FTSResult is one entry returned by [DB.FTSSearch].
type FTSResult struct {
	ID      string  `json:"id"`
	Snippet string  `json:"snippet"`
	Rank    float64 `json:"rank"`
}

// HybridResult is one entry returned by [DB.HybridQuery].
type HybridResult struct {
	ID          string  `json:"id"`
	RankScore   float64 `json:"rank_score"`
	VectorScore float64 `json:"vector_score"`
	GraphWeight float64 `json:"graph_weight"`
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

// Open opens or creates an AgentDB database at path.
// Use ":memory:" for an in-memory database that disappears when closed.
//
// The returned *DB has a finalizer set so it will be closed on GC, but you
// should call Close explicitly to control when resources are released.
func Open(path string) (*DB, error) {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))

	handle := C.agentdb_open(cpath)
	if handle == nil {
		return nil, lastError("agentdb_open returned nil")
	}

	db := &DB{handle: handle}
	runtime.SetFinalizer(db, (*DB).Close)
	return db, nil
}

// Close releases all resources held by the database. After Close returns the
// DB must not be used. Calling Close more than once is safe.
func (db *DB) Close() {
	if db.handle != nil {
		C.agentdb_close(db.handle)
		db.handle = nil
		runtime.SetFinalizer(db, nil)
	}
}

// ── SQL ──────────────────────────────────────────────────────────────────────

// Execute runs a raw SQL statement (no parameters) and returns the number of
// rows affected. It is suitable for DDL and DML that does not return rows.
func (db *DB) Execute(sql string) (int64, error) {
	csql := C.CString(sql)
	defer C.free(unsafe.Pointer(csql))

	n := C.agentdb_execute(db.handle, csql)
	if n == -1 {
		return -1, lastError("agentdb_execute failed")
	}
	return int64(n), nil
}

// QueryJSON executes a SELECT statement and returns all rows encoded as a
// JSON array of objects. Each object's keys match the column names.
func (db *DB) QueryJSON(sql string) (string, error) {
	csql := C.CString(sql)
	defer C.free(unsafe.Pointer(csql))

	ptr := C.agentdb_query_json(db.handle, csql)
	if ptr == nil {
		return "", lastError("agentdb_query_json failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ── Vector store ─────────────────────────────────────────────────────────────

// VectorUpsert inserts or updates a vector in collection. metadata is an
// optional JSON object; pass nil to omit it.
func (db *DB) VectorUpsert(collection, id string, vector []float32, metadata []byte) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	var cmeta *C.char
	if metadata != nil {
		cmeta = C.CString(string(metadata))
		defer C.free(unsafe.Pointer(cmeta))
	}

	if len(vector) == 0 {
		return errors.New("vector must not be empty")
	}
	cvec := (*C.float)(unsafe.Pointer(&vector[0]))

	rc := C.agentdb_vector_upsert(db.handle, ccol, cid, cvec, C.ulong(len(vector)), cmeta)
	if rc != 0 {
		return lastError("agentdb_vector_upsert failed")
	}
	return nil
}

// VectorSearch finds the top-k most similar vectors to query in collection.
// filterJSON is an optional MongoDB-style metadata filter (pass nil to skip).
func (db *DB) VectorSearch(collection string, query []float32, topK int, filterJSON []byte) ([]VectorResult, error) {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))

	var cfilter *C.char
	if filterJSON != nil {
		cfilter = C.CString(string(filterJSON))
		defer C.free(unsafe.Pointer(cfilter))
	}

	if len(query) == 0 {
		return nil, errors.New("query vector must not be empty")
	}
	cq := (*C.float)(unsafe.Pointer(&query[0]))

	ptr := C.agentdb_vector_search(db.handle, ccol, cq, C.ulong(len(query)), C.ulong(topK), cfilter)
	if ptr == nil {
		return nil, lastError("agentdb_vector_search failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)

	var results []VectorResult
	if err := json.Unmarshal([]byte(raw), &results); err != nil {
		return nil, fmt.Errorf("agentdb: parse vector search results: %w", err)
	}
	return results, nil
}

// ── Memory graph ─────────────────────────────────────────────────────────────

// GraphAddNode upserts a node into the memory graph. dataJSON is an optional
// JSON metadata object (pass nil to omit).
func (db *DB) GraphAddNode(id, kind string, dataJSON []byte) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))
	ckind := C.CString(kind)
	defer C.free(unsafe.Pointer(ckind))

	var cdata *C.char
	if dataJSON != nil {
		cdata = C.CString(string(dataJSON))
		defer C.free(unsafe.Pointer(cdata))
	}

	rc := C.agentdb_graph_add_node(db.handle, cid, ckind, cdata)
	if rc != 0 {
		return lastError("agentdb_graph_add_node failed")
	}
	return nil
}

// GraphAddEdge adds or updates a directed weighted edge (src → dst) with the
// given relation label and weight.
func (db *DB) GraphAddEdge(src, dst, relation string, weight float64) error {
	csrc := C.CString(src)
	defer C.free(unsafe.Pointer(csrc))
	cdst := C.CString(dst)
	defer C.free(unsafe.Pointer(cdst))
	crel := C.CString(relation)
	defer C.free(unsafe.Pointer(crel))

	rc := C.agentdb_graph_add_edge(db.handle, csrc, cdst, crel, C.double(weight))
	if rc != 0 {
		return lastError("agentdb_graph_add_edge failed")
	}
	return nil
}

// GraphNeighbors traverses the memory graph from nodeID up to maxDepth hops,
// returning only edges whose weight is >= minWeight (use 0.0 for all edges).
// Pass an empty string for relation to traverse all edge types.
func (db *DB) GraphNeighbors(nodeID string, maxDepth int, minWeight float64, relation string) ([]GraphNode, error) {
	cid := C.CString(nodeID)
	defer C.free(unsafe.Pointer(cid))

	var crel *C.char
	if relation != "" {
		crel = C.CString(relation)
		defer C.free(unsafe.Pointer(crel))
	}

	ptr := C.agentdb_graph_neighbors(db.handle, cid, C.ulong(maxDepth), C.double(minWeight), crel)
	if ptr == nil {
		return nil, lastError("agentdb_graph_neighbors failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)

	var results []GraphNode
	if err := json.Unmarshal([]byte(raw), &results); err != nil {
		return nil, fmt.Errorf("agentdb: parse graph neighbors: %w", err)
	}
	return results, nil
}

// ── Full-text search ──────────────────────────────────────────────────────────

// FTSIndex adds or updates a text document in collection. vecID and
// collectionID are correlation keys that tie the FTS entry back to a vector.
func (db *DB) FTSIndex(collection, vecID, collectionID, text string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))
	cvid := C.CString(vecID)
	defer C.free(unsafe.Pointer(cvid))
	ccid := C.CString(collectionID)
	defer C.free(unsafe.Pointer(ccid))
	ctxt := C.CString(text)
	defer C.free(unsafe.Pointer(ctxt))

	rc := C.agentdb_fts_index(db.handle, ccol, cvid, ccid, ctxt)
	if rc != 0 {
		return lastError("agentdb_fts_index failed")
	}
	return nil
}

// FTSSearch runs a full-text query against collection, returning up to topK
// results with snippet highlights.
func (db *DB) FTSSearch(collection, query string, topK int) ([]FTSResult, error) {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))
	cq := C.CString(query)
	defer C.free(unsafe.Pointer(cq))

	ptr := C.agentdb_fts_search(db.handle, ccol, cq, C.ulong(topK))
	if ptr == nil {
		return nil, lastError("agentdb_fts_search failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)

	var results []FTSResult
	if err := json.Unmarshal([]byte(raw), &results); err != nil {
		return nil, fmt.Errorf("agentdb: parse fts results: %w", err)
	}
	return results, nil
}

// ── Hybrid query ──────────────────────────────────────────────────────────────

// HybridQuery blends a graph traversal from anchorNode with a vector
// similarity search in collection. alpha controls the blend:
// 0.0 = pure graph ranking, 1.0 = pure vector ranking.
// filterJSON is an optional MongoDB-style metadata filter (pass nil to skip).
func (db *DB) HybridQuery(anchorNode string, embedding []float32, collection string, graphDepth, topK int, alpha float64, filterJSON []byte) ([]HybridResult, error) {
	canchor := C.CString(anchorNode)
	defer C.free(unsafe.Pointer(canchor))
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))

	var cfilter *C.char
	if filterJSON != nil {
		cfilter = C.CString(string(filterJSON))
		defer C.free(unsafe.Pointer(cfilter))
	}

	if len(embedding) == 0 {
		return nil, errors.New("embedding must not be empty")
	}
	cemb := (*C.float)(unsafe.Pointer(&embedding[0]))

	ptr := C.agentdb_hybrid_query(
		db.handle, canchor,
		cemb, C.ulong(len(embedding)),
		ccol,
		C.ulong(graphDepth), C.ulong(topK),
		C.double(alpha),
		cfilter,
	)
	if ptr == nil {
		return nil, lastError("agentdb_hybrid_query failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)

	var results []HybridResult
	if err := json.Unmarshal([]byte(raw), &results); err != nil {
		return nil, fmt.Errorf("agentdb: parse hybrid results: %w", err)
	}
	return results, nil
}

// ── Stats ─────────────────────────────────────────────────────────────────────

// Stats returns a snapshot of database statistics (collections, vectors,
// nodes, edges).
func (db *DB) Stats() (Stats, error) {
	ptr := C.agentdb_stats(db.handle)
	if ptr == nil {
		return Stats{}, lastError("agentdb_stats failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)

	var s Stats
	if err := json.Unmarshal([]byte(raw), &s); err != nil {
		return Stats{}, fmt.Errorf("agentdb: parse stats: %w", err)
	}
	return s, nil
}

// ── Conversations ───────────────────────────────────────────────────────────

// ConversationCreate creates a new conversation thread. title and metadata are
// optional (pass "" to omit).
func (db *DB) ConversationCreate(id, title string, metadataJSON []byte) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	var ctitle *C.char
	if title != "" {
		ctitle = C.CString(title)
		defer C.free(unsafe.Pointer(ctitle))
	}

	var cmeta *C.char
	if metadataJSON != nil {
		cmeta = C.CString(string(metadataJSON))
		defer C.free(unsafe.Pointer(cmeta))
	}

	rc := C.agentdb_conversation_create(db.handle, cid, ctitle, cmeta)
	if rc != 0 {
		return lastError("agentdb_conversation_create failed")
	}
	return nil
}

// ConversationAddMessage appends a message to a conversation. Returns the new message ID.
func (db *DB) ConversationAddMessage(conversationID, role, content string, metadataJSON []byte) (string, error) {
	ccid := C.CString(conversationID)
	defer C.free(unsafe.Pointer(ccid))
	crole := C.CString(role)
	defer C.free(unsafe.Pointer(crole))
	ccontent := C.CString(content)
	defer C.free(unsafe.Pointer(ccontent))

	var cmeta *C.char
	if metadataJSON != nil {
		cmeta = C.CString(string(metadataJSON))
		defer C.free(unsafe.Pointer(cmeta))
	}

	ptr := C.agentdb_conversation_add_message(db.handle, ccid, crole, ccontent, cmeta)
	if ptr == nil {
		return "", lastError("agentdb_conversation_add_message failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ConversationGetMessages returns messages for a conversation as a JSON array.
// Pass limit=0 for all messages.
func (db *DB) ConversationGetMessages(conversationID string, limit int) (string, error) {
	ccid := C.CString(conversationID)
	defer C.free(unsafe.Pointer(ccid))

	ptr := C.agentdb_conversation_get_messages(db.handle, ccid, C.ulong(limit))
	if ptr == nil {
		return "", lastError("agentdb_conversation_get_messages failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ConversationList returns all conversations as a JSON array.
func (db *DB) ConversationList() (string, error) {
	ptr := C.agentdb_conversation_list(db.handle)
	if ptr == nil {
		return "", lastError("agentdb_conversation_list failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ConversationDelete deletes a conversation and all its messages.
func (db *DB) ConversationDelete(id string) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	rc := C.agentdb_conversation_delete(db.handle, cid)
	if rc != 0 {
		return lastError("agentdb_conversation_delete failed")
	}
	return nil
}

// ── Workflows ───────────────────────────────────────────────────────────────

// WorkflowCreate creates a new workflow. inputJSON and metadataJSON are optional (pass nil to omit).
func (db *DB) WorkflowCreate(id, name string, inputJSON, metadataJSON []byte) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))

	var cinput *C.char
	if inputJSON != nil {
		cinput = C.CString(string(inputJSON))
		defer C.free(unsafe.Pointer(cinput))
	}

	var cmeta *C.char
	if metadataJSON != nil {
		cmeta = C.CString(string(metadataJSON))
		defer C.free(unsafe.Pointer(cmeta))
	}

	rc := C.agentdb_workflow_create(db.handle, cid, cname, cinput, cmeta)
	if rc != 0 {
		return lastError("agentdb_workflow_create failed")
	}
	return nil
}

// WorkflowAddStep adds a step to a workflow. Returns the new step ID.
func (db *DB) WorkflowAddStep(workflowID, name string, inputJSON []byte) (string, error) {
	cwid := C.CString(workflowID)
	defer C.free(unsafe.Pointer(cwid))
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))

	var cinput *C.char
	if inputJSON != nil {
		cinput = C.CString(string(inputJSON))
		defer C.free(unsafe.Pointer(cinput))
	}

	ptr := C.agentdb_workflow_add_step(db.handle, cwid, cname, cinput)
	if ptr == nil {
		return "", lastError("agentdb_workflow_add_step failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// WorkflowUpdateStep updates a step's status, output, and/or error.
func (db *DB) WorkflowUpdateStep(stepID, status string, outputJSON []byte, errMsg string) error {
	csid := C.CString(stepID)
	defer C.free(unsafe.Pointer(csid))
	cstatus := C.CString(status)
	defer C.free(unsafe.Pointer(cstatus))

	var coutput *C.char
	if outputJSON != nil {
		coutput = C.CString(string(outputJSON))
		defer C.free(unsafe.Pointer(coutput))
	}

	var cerr *C.char
	if errMsg != "" {
		cerr = C.CString(errMsg)
		defer C.free(unsafe.Pointer(cerr))
	}

	rc := C.agentdb_workflow_update_step(db.handle, csid, cstatus, coutput, cerr)
	if rc != 0 {
		return lastError("agentdb_workflow_update_step failed")
	}
	return nil
}

// WorkflowComplete marks a workflow as completed with optional output.
func (db *DB) WorkflowComplete(id string, outputJSON []byte) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	var coutput *C.char
	if outputJSON != nil {
		coutput = C.CString(string(outputJSON))
		defer C.free(unsafe.Pointer(coutput))
	}

	rc := C.agentdb_workflow_complete(db.handle, cid, coutput)
	if rc != 0 {
		return lastError("agentdb_workflow_complete failed")
	}
	return nil
}

// WorkflowGet retrieves a workflow and its steps as a JSON object.
func (db *DB) WorkflowGet(id string) (string, error) {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	ptr := C.agentdb_workflow_get(db.handle, cid)
	if ptr == nil {
		return "", lastError("agentdb_workflow_get failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// WorkflowList lists workflows as a JSON array. Pass "" for statusFilter to get all.
func (db *DB) WorkflowList(statusFilter string) (string, error) {
	var cfilter *C.char
	if statusFilter != "" {
		cfilter = C.CString(statusFilter)
		defer C.free(unsafe.Pointer(cfilter))
	}

	ptr := C.agentdb_workflow_list(db.handle, cfilter)
	if ptr == nil {
		return "", lastError("agentdb_workflow_list failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ── Traces ──────────────────────────────────────────────────────────────────

// TraceAdd records a reasoning trace entry. Returns the new trace ID.
// sessionID and parentID are optional (pass "" to omit).
func (db *DB) TraceAdd(sessionID, parentID, traceType, content string, metadataJSON []byte) (string, error) {
	var csid *C.char
	if sessionID != "" {
		csid = C.CString(sessionID)
		defer C.free(unsafe.Pointer(csid))
	}

	var cpid *C.char
	if parentID != "" {
		cpid = C.CString(parentID)
		defer C.free(unsafe.Pointer(cpid))
	}

	ctt := C.CString(traceType)
	defer C.free(unsafe.Pointer(ctt))
	ccontent := C.CString(content)
	defer C.free(unsafe.Pointer(ccontent))

	var cmeta *C.char
	if metadataJSON != nil {
		cmeta = C.CString(string(metadataJSON))
		defer C.free(unsafe.Pointer(cmeta))
	}

	ptr := C.agentdb_trace_add(db.handle, csid, cpid, ctt, ccontent, cmeta)
	if ptr == nil {
		return "", lastError("agentdb_trace_add failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// TraceGetBySession returns all traces for a session as a JSON array.
func (db *DB) TraceGetBySession(sessionID string) (string, error) {
	csid := C.CString(sessionID)
	defer C.free(unsafe.Pointer(csid))

	ptr := C.agentdb_trace_get_by_session(db.handle, csid)
	if ptr == nil {
		return "", lastError("agentdb_trace_get_by_session failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// TraceGetTree returns a trace subtree as a JSON array.
func (db *DB) TraceGetTree(rootID string) (string, error) {
	crid := C.CString(rootID)
	defer C.free(unsafe.Pointer(crid))

	ptr := C.agentdb_trace_get_tree(db.handle, crid)
	if ptr == nil {
		return "", lastError("agentdb_trace_get_tree failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ── Additional vector operations ────────────────────────────────────────────

// VectorDelete removes a vector by id from collection.
func (db *DB) VectorDelete(collection, id string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	rc := C.agentdb_vector_delete(db.handle, ccol, cid)
	if rc != 0 {
		return lastError("agentdb_vector_delete failed")
	}
	return nil
}

// DropCollection drops a vector collection and all its data.
func (db *DB) DropCollection(collection string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))

	rc := C.agentdb_drop_collection(db.handle, ccol)
	if rc != 0 {
		return lastError("agentdb_drop_collection failed")
	}
	return nil
}

// Reindex forces a full HNSW index rebuild for collection.
func (db *DB) Reindex(collection string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))

	rc := C.agentdb_reindex(db.handle, ccol)
	if rc != 0 {
		return lastError("agentdb_reindex failed")
	}
	return nil
}

// ── Additional graph operations ─────────────────────────────────────────────

// GraphGetNode returns a single graph node as a JSON object, or "" if not found.
func (db *DB) GraphGetNode(id string) (string, error) {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	ptr := C.agentdb_graph_get_node(db.handle, cid)
	if ptr == nil {
		return "", lastError("agentdb_graph_get_node failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// GraphDeleteNode removes a node (and its incident edges) from the graph.
func (db *DB) GraphDeleteNode(id string) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	rc := C.agentdb_graph_delete_node(db.handle, cid)
	if rc != 0 {
		return lastError("agentdb_graph_delete_node failed")
	}
	return nil
}

// GraphDeleteEdge removes a directed edge (src → dst) with the given relation.
func (db *DB) GraphDeleteEdge(src, dst, relation string) error {
	csrc := C.CString(src)
	defer C.free(unsafe.Pointer(csrc))
	cdst := C.CString(dst)
	defer C.free(unsafe.Pointer(cdst))
	crel := C.CString(relation)
	defer C.free(unsafe.Pointer(crel))

	rc := C.agentdb_graph_delete_edge(db.handle, csrc, cdst, crel)
	if rc != 0 {
		return lastError("agentdb_graph_delete_edge failed")
	}
	return nil
}

// ── Additional FTS operations ────────────────────────────────────────────────

// FTSDelete removes a document from the FTS index.
func (db *DB) FTSDelete(collection, vecID string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))
	cvid := C.CString(vecID)
	defer C.free(unsafe.Pointer(cvid))

	rc := C.agentdb_fts_delete(db.handle, ccol, cvid)
	if rc != 0 {
		return lastError("agentdb_fts_delete failed")
	}
	return nil
}

// FTSOptimize merges FTS index segments for better query performance.
func (db *DB) FTSOptimize(collection string) error {
	ccol := C.CString(collection)
	defer C.free(unsafe.Pointer(ccol))

	rc := C.agentdb_fts_optimize(db.handle, ccol)
	if rc != 0 {
		return lastError("agentdb_fts_optimize failed")
	}
	return nil
}

// ── Additional workflow operations ───────────────────────────────────────────

// WorkflowFail marks a workflow as failed with an optional error message.
func (db *DB) WorkflowFail(id, errMsg string) error {
	cid := C.CString(id)
	defer C.free(unsafe.Pointer(cid))

	var cerr *C.char
	if errMsg != "" {
		cerr = C.CString(errMsg)
		defer C.free(unsafe.Pointer(cerr))
	}

	rc := C.agentdb_workflow_fail(db.handle, cid, cerr)
	if rc != 0 {
		return lastError("agentdb_workflow_fail failed")
	}
	return nil
}

// ── Tool Registry ──────────────────────────────────────────────────────────

// ToolRegister registers or updates a tool definition. Returns the tool ID.
func (db *DB) ToolRegister(name, description string, parametersSchemaJSON []byte, version string) (string, error) {
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))

	var cdesc *C.char
	if description != "" {
		cdesc = C.CString(description)
		defer C.free(unsafe.Pointer(cdesc))
	}

	var cschema *C.char
	if parametersSchemaJSON != nil {
		cschema = C.CString(string(parametersSchemaJSON))
		defer C.free(unsafe.Pointer(cschema))
	}

	var cver *C.char
	if version != "" {
		cver = C.CString(version)
		defer C.free(unsafe.Pointer(cver))
	}

	ptr := C.agentdb_tool_register(db.handle, cname, cdesc, cschema, cver)
	if ptr == nil {
		return "", lastError("agentdb_tool_register failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ToolList returns all registered tools as a JSON array.
func (db *DB) ToolList() (string, error) {
	ptr := C.agentdb_tool_list(db.handle)
	if ptr == nil {
		return "", lastError("agentdb_tool_list failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ToolLogCall logs a tool call invocation. Returns the tool call ID.
func (db *DB) ToolLogCall(sessionID, toolName string, argumentsJSON, resultJSON []byte, errMsg string, latencyMs int64) (string, error) {
	var csid *C.char
	if sessionID != "" {
		csid = C.CString(sessionID)
		defer C.free(unsafe.Pointer(csid))
	}

	ctn := C.CString(toolName)
	defer C.free(unsafe.Pointer(ctn))

	var cargs *C.char
	if argumentsJSON != nil {
		cargs = C.CString(string(argumentsJSON))
		defer C.free(unsafe.Pointer(cargs))
	}

	var cres *C.char
	if resultJSON != nil {
		cres = C.CString(string(resultJSON))
		defer C.free(unsafe.Pointer(cres))
	}

	var cerr *C.char
	if errMsg != "" {
		cerr = C.CString(errMsg)
		defer C.free(unsafe.Pointer(cerr))
	}

	ptr := C.agentdb_tool_log_call(db.handle, csid, ctn, cargs, cres, cerr, C.int64_t(latencyMs))
	if ptr == nil {
		return "", lastError("agentdb_tool_log_call failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ── Audit Log ──────────────────────────────────────────────────────────────

// AuditLog appends an entry to the immutable audit log. Returns the entry ID.
func (db *DB) AuditLog(actor, action, tableName, recordID string, oldValueJSON, newValueJSON []byte, reason string) (string, error) {
	var cactor *C.char
	if actor != "" {
		cactor = C.CString(actor)
		defer C.free(unsafe.Pointer(cactor))
	}

	caction := C.CString(action)
	defer C.free(unsafe.Pointer(caction))
	ctbl := C.CString(tableName)
	defer C.free(unsafe.Pointer(ctbl))
	crid := C.CString(recordID)
	defer C.free(unsafe.Pointer(crid))

	var cold *C.char
	if oldValueJSON != nil {
		cold = C.CString(string(oldValueJSON))
		defer C.free(unsafe.Pointer(cold))
	}

	var cnew *C.char
	if newValueJSON != nil {
		cnew = C.CString(string(newValueJSON))
		defer C.free(unsafe.Pointer(cnew))
	}

	var creason *C.char
	if reason != "" {
		creason = C.CString(reason)
		defer C.free(unsafe.Pointer(creason))
	}

	ptr := C.agentdb_audit_log(db.handle, cactor, caction, ctbl, crid, cold, cnew, creason)
	if ptr == nil {
		return "", lastError("agentdb_audit_log failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// AuditQueryRecent returns recent audit log entries as a JSON array.
func (db *DB) AuditQueryRecent(limit int) (string, error) {
	ptr := C.agentdb_audit_query_recent(db.handle, C.ulong(limit))
	if ptr == nil {
		return "", lastError("agentdb_audit_query_recent failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ── Context Window ─────────────────────────────────────────────────────────

// ContextAdd adds an entry to the context window. Returns the entry ID.
func (db *DB) ContextAdd(sessionID, sourceType, sourceID, contentPreview string, tokenCount int64, relevanceScore float64, priority int64) (string, error) {
	csid := C.CString(sessionID)
	defer C.free(unsafe.Pointer(csid))
	cst := C.CString(sourceType)
	defer C.free(unsafe.Pointer(cst))
	csi := C.CString(sourceID)
	defer C.free(unsafe.Pointer(csi))

	var cprev *C.char
	if contentPreview != "" {
		cprev = C.CString(contentPreview)
		defer C.free(unsafe.Pointer(cprev))
	}

	ptr := C.agentdb_context_add(db.handle, csid, cst, csi, cprev, C.int64_t(tokenCount), C.double(relevanceScore), C.int64_t(priority))
	if ptr == nil {
		return "", lastError("agentdb_context_add failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ContextBuildWindow builds a token-budgeted context window as a JSON array.
func (db *DB) ContextBuildWindow(sessionID string, maxTokens int64) (string, error) {
	csid := C.CString(sessionID)
	defer C.free(unsafe.Pointer(csid))

	ptr := C.agentdb_context_build_window(db.handle, csid, C.int64_t(maxTokens))
	if ptr == nil {
		return "", lastError("agentdb_context_build_window failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// ContextClear removes all context entries for a session.
func (db *DB) ContextClear(sessionID string) error {
	csid := C.CString(sessionID)
	defer C.free(unsafe.Pointer(csid))

	rc := C.agentdb_context_clear(db.handle, csid)
	if rc != 0 {
		return lastError("agentdb_context_clear failed")
	}
	return nil
}

// ── Prompt Templates ───────────────────────────────────────────────────────

// PromptCreate creates a new version of a prompt template. Returns the template ID.
func (db *DB) PromptCreate(name, template, modelHint string, maxTokens int64, metadataJSON []byte) (string, error) {
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	ctmpl := C.CString(template)
	defer C.free(unsafe.Pointer(ctmpl))

	var cmodel *C.char
	if modelHint != "" {
		cmodel = C.CString(modelHint)
		defer C.free(unsafe.Pointer(cmodel))
	}

	var cmeta *C.char
	if metadataJSON != nil {
		cmeta = C.CString(string(metadataJSON))
		defer C.free(unsafe.Pointer(cmeta))
	}

	ptr := C.agentdb_prompt_create(db.handle, cname, ctmpl, cmodel, C.int64_t(maxTokens), cmeta)
	if ptr == nil {
		return "", lastError("agentdb_prompt_create failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// PromptRender renders a prompt template with variable substitution.
// varsJSON should be a JSON object of key-value string pairs.
func (db *DB) PromptRender(name string, varsJSON []byte) (string, error) {
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))

	var cvars *C.char
	if varsJSON != nil {
		cvars = C.CString(string(varsJSON))
		defer C.free(unsafe.Pointer(cvars))
	}

	ptr := C.agentdb_prompt_render(db.handle, cname, cvars)
	if ptr == nil {
		return "", lastError("agentdb_prompt_render failed")
	}
	result := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return result, nil
}

// ── Data Labels (Privacy) ──────────────────────────────────────────────────

// LabelTag tags a record with a privacy/classification label.
func (db *DB) LabelTag(tableName, recordID, label, taggedBy string) error {
	ctbl := C.CString(tableName)
	defer C.free(unsafe.Pointer(ctbl))
	crid := C.CString(recordID)
	defer C.free(unsafe.Pointer(crid))
	clbl := C.CString(label)
	defer C.free(unsafe.Pointer(clbl))

	var cby *C.char
	if taggedBy != "" {
		cby = C.CString(taggedBy)
		defer C.free(unsafe.Pointer(cby))
	}

	rc := C.agentdb_label_tag(db.handle, ctbl, crid, clbl, cby)
	if rc != 0 {
		return lastError("agentdb_label_tag failed")
	}
	return nil
}

// LabelUntag removes a specific label from a record.
func (db *DB) LabelUntag(tableName, recordID, label string) error {
	ctbl := C.CString(tableName)
	defer C.free(unsafe.Pointer(ctbl))
	crid := C.CString(recordID)
	defer C.free(unsafe.Pointer(crid))
	clbl := C.CString(label)
	defer C.free(unsafe.Pointer(clbl))

	rc := C.agentdb_label_untag(db.handle, ctbl, crid, clbl)
	if rc != 0 {
		return lastError("agentdb_label_untag failed")
	}
	return nil
}

// LabelGet returns all labels for a record as a JSON array.
func (db *DB) LabelGet(tableName, recordID string) (string, error) {
	ctbl := C.CString(tableName)
	defer C.free(unsafe.Pointer(ctbl))
	crid := C.CString(recordID)
	defer C.free(unsafe.Pointer(crid))

	ptr := C.agentdb_label_get(db.handle, ctbl, crid)
	if ptr == nil {
		return "", lastError("agentdb_label_get failed")
	}
	raw := C.GoString(ptr)
	C.agentdb_free_string(ptr)
	return raw, nil
}

// LabelHas checks if a record has a specific label.
func (db *DB) LabelHas(tableName, recordID, label string) (bool, error) {
	ctbl := C.CString(tableName)
	defer C.free(unsafe.Pointer(ctbl))
	crid := C.CString(recordID)
	defer C.free(unsafe.Pointer(crid))
	clbl := C.CString(label)
	defer C.free(unsafe.Pointer(clbl))

	rc := C.agentdb_label_has(db.handle, ctbl, crid, clbl)
	if rc == -1 {
		return false, lastError("agentdb_label_has failed")
	}
	return rc == 1, nil
}
