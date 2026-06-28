/*
 * JNI glue layer for AgentDB Java bindings.
 *
 * This file maps Java native method names (JNI-mangled symbols) to the
 * flat C FFI functions exported by libagentdb. Compile as a shared library
 * and link against libagentdb.
 *
 * Build (Linux):
 *   gcc -shared -fPIC -o libagentdb_jni.so agentdb_jni.c \
 *       -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/linux" \
 *       -L/path/to/libagentdb -lagentdb
 *
 * Build (macOS):
 *   gcc -shared -fPIC -o libagentdb_jni.dylib agentdb_jni.c \
 *       -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/darwin" \
 *       -L/path/to/libagentdb -lagentdb
 *
 * Build (Windows):
 *   cl /LD agentdb_jni.c /I"%JAVA_HOME%\include" /I"%JAVA_HOME%\include\win32" \
 *      /link agentdb.lib /out:agentdb.dll
 */

#include <jni.h>
#include <stdlib.h>
#include <string.h>

/* Forward declarations of the AgentDB C FFI. */
typedef struct AgentDbHandle AgentDbHandle;

extern AgentDbHandle* agentdb_open(const char* path);
extern void           agentdb_close(AgentDbHandle* handle);
extern long long      agentdb_execute(AgentDbHandle* handle, const char* sql);
extern char*          agentdb_query_json(AgentDbHandle* handle, const char* sql);
extern int            agentdb_vector_upsert(AgentDbHandle* handle, const char* collection,
                          const char* id, const float* vector, size_t dim, const char* metadata);
extern char*          agentdb_vector_search(AgentDbHandle* handle, const char* collection,
                          const float* query, size_t dim, size_t top_k, const char* filter_json);
extern int            agentdb_graph_add_node(AgentDbHandle* handle, const char* id,
                          const char* kind, const char* data_json);
extern int            agentdb_graph_add_edge(AgentDbHandle* handle, const char* src,
                          const char* dst, const char* relation, double weight);
extern char*          agentdb_graph_neighbors(AgentDbHandle* handle, const char* node_id,
                          size_t max_depth, double min_weight);
extern int            agentdb_fts_index(AgentDbHandle* handle, const char* collection,
                          const char* vec_id, const char* collection_id, const char* text);
extern char*          agentdb_fts_search(AgentDbHandle* handle, const char* collection,
                          const char* query, size_t top_k);
extern char*          agentdb_hybrid_query(AgentDbHandle* handle, const char* anchor_node,
                          const float* embedding, size_t dim, const char* collection,
                          size_t graph_depth, size_t top_k, double alpha);
extern char*          agentdb_stats(AgentDbHandle* handle);
extern char*          agentdb_last_error(void);
extern void           agentdb_free_string(char* ptr);

/* Conversation FFI */
extern int   agentdb_conversation_create(AgentDbHandle*, const char*, const char*, const char*);
extern char* agentdb_conversation_add_message(AgentDbHandle*, const char*, const char*, const char*, const char*);
extern char* agentdb_conversation_get_messages(AgentDbHandle*, const char*, size_t);
extern char* agentdb_conversation_list(AgentDbHandle*);
extern int   agentdb_conversation_delete(AgentDbHandle*, const char*);

/* Workflow FFI */
extern int   agentdb_workflow_create(AgentDbHandle*, const char*, const char*, const char*);
extern char* agentdb_workflow_add_step(AgentDbHandle*, const char*, const char*, const char*);
extern int   agentdb_workflow_update_step(AgentDbHandle*, const char*, const char*, const char*, const char*);
extern int   agentdb_workflow_complete(AgentDbHandle*, const char*, const char*);
extern char* agentdb_workflow_get(AgentDbHandle*, const char*);
extern char* agentdb_workflow_list(AgentDbHandle*, const char*);

/* Trace FFI */
extern char* agentdb_trace_add(AgentDbHandle*, const char*, const char*, const char*, const char*, const char*);
extern char* agentdb_trace_get_by_session(AgentDbHandle*, const char*);
extern char* agentdb_trace_get_tree(AgentDbHandle*, const char*);

/* ── Helpers ─────────────────────────────────────────────────────────── */

static const char* jstr_to_c(JNIEnv* env, jstring s) {
    if (s == NULL) return NULL;
    return (*env)->GetStringUTFChars(env, s, NULL);
}

static void release_jstr(JNIEnv* env, jstring s, const char* c) {
    if (s != NULL && c != NULL) {
        (*env)->ReleaseStringUTFChars(env, s, c);
    }
}

static jstring c_to_jstr_free(JNIEnv* env, char* c) {
    if (c == NULL) return NULL;
    jstring result = (*env)->NewStringUTF(env, c);
    agentdb_free_string(c);
    return result;
}

/* ── JNI implementations ─────────────────────────────────────────────── */

JNIEXPORT jlong JNICALL
Java_com_datacules_agentdb_AgentDB_nativeOpen(JNIEnv* env, jclass cls, jstring path) {
    const char* p = jstr_to_c(env, path);
    AgentDbHandle* h = agentdb_open(p);
    release_jstr(env, path, p);
    return (jlong)(intptr_t)h;
}

JNIEXPORT void JNICALL
Java_com_datacules_agentdb_AgentDB_nativeClose(JNIEnv* env, jclass cls, jlong handle) {
    if (handle != 0) {
        agentdb_close((AgentDbHandle*)(intptr_t)handle);
    }
}

JNIEXPORT jlong JNICALL
Java_com_datacules_agentdb_AgentDB_nativeExecute(JNIEnv* env, jclass cls, jlong handle, jstring sql) {
    const char* s = jstr_to_c(env, sql);
    jlong result = (jlong)agentdb_execute((AgentDbHandle*)(intptr_t)handle, s);
    release_jstr(env, sql, s);
    return result;
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeQueryJson(JNIEnv* env, jclass cls, jlong handle, jstring sql) {
    const char* s = jstr_to_c(env, sql);
    char* result = agentdb_query_json((AgentDbHandle*)(intptr_t)handle, s);
    release_jstr(env, sql, s);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jint JNICALL
Java_com_datacules_agentdb_AgentDB_nativeVectorUpsert(JNIEnv* env, jclass cls,
        jlong handle, jstring collection, jstring id, jfloatArray vector, jstring metadataJson) {
    const char* col = jstr_to_c(env, collection);
    const char* vid = jstr_to_c(env, id);
    const char* meta = jstr_to_c(env, metadataJson);

    jsize dim = (*env)->GetArrayLength(env, vector);
    jfloat* vec = (*env)->GetFloatArrayElements(env, vector, NULL);

    jint rc = agentdb_vector_upsert((AgentDbHandle*)(intptr_t)handle, col, vid, vec, (size_t)dim, meta);

    (*env)->ReleaseFloatArrayElements(env, vector, vec, JNI_ABORT);
    release_jstr(env, collection, col);
    release_jstr(env, id, vid);
    release_jstr(env, metadataJson, meta);
    return rc;
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeVectorSearch(JNIEnv* env, jclass cls,
        jlong handle, jstring collection, jfloatArray query, jint topK, jstring filterJson) {
    const char* col = jstr_to_c(env, collection);
    const char* filt = jstr_to_c(env, filterJson);

    jsize dim = (*env)->GetArrayLength(env, query);
    jfloat* q = (*env)->GetFloatArrayElements(env, query, NULL);

    char* result = agentdb_vector_search((AgentDbHandle*)(intptr_t)handle,
        col, q, (size_t)dim, (size_t)topK, filt);

    (*env)->ReleaseFloatArrayElements(env, query, q, JNI_ABORT);
    release_jstr(env, collection, col);
    release_jstr(env, filterJson, filt);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jint JNICALL
Java_com_datacules_agentdb_AgentDB_nativeGraphAddNode(JNIEnv* env, jclass cls,
        jlong handle, jstring id, jstring kind, jstring dataJson) {
    const char* cid = jstr_to_c(env, id);
    const char* ckind = jstr_to_c(env, kind);
    const char* cdata = jstr_to_c(env, dataJson);
    jint rc = agentdb_graph_add_node((AgentDbHandle*)(intptr_t)handle, cid, ckind, cdata);
    release_jstr(env, id, cid);
    release_jstr(env, kind, ckind);
    release_jstr(env, dataJson, cdata);
    return rc;
}

JNIEXPORT jint JNICALL
Java_com_datacules_agentdb_AgentDB_nativeGraphAddEdge(JNIEnv* env, jclass cls,
        jlong handle, jstring src, jstring dst, jstring relation, jdouble weight) {
    const char* csrc = jstr_to_c(env, src);
    const char* cdst = jstr_to_c(env, dst);
    const char* crel = jstr_to_c(env, relation);
    jint rc = agentdb_graph_add_edge((AgentDbHandle*)(intptr_t)handle, csrc, cdst, crel, weight);
    release_jstr(env, src, csrc);
    release_jstr(env, dst, cdst);
    release_jstr(env, relation, crel);
    return rc;
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeGraphNeighbors(JNIEnv* env, jclass cls,
        jlong handle, jstring nodeId, jint maxDepth, jdouble minWeight) {
    const char* nid = jstr_to_c(env, nodeId);
    char* result = agentdb_graph_neighbors((AgentDbHandle*)(intptr_t)handle,
        nid, (size_t)maxDepth, minWeight);
    release_jstr(env, nodeId, nid);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jint JNICALL
Java_com_datacules_agentdb_AgentDB_nativeFtsIndex(JNIEnv* env, jclass cls,
        jlong handle, jstring collection, jstring vecId, jstring collectionId, jstring text) {
    const char* col = jstr_to_c(env, collection);
    const char* vid = jstr_to_c(env, vecId);
    const char* cid = jstr_to_c(env, collectionId);
    const char* txt = jstr_to_c(env, text);
    jint rc = agentdb_fts_index((AgentDbHandle*)(intptr_t)handle, col, vid, cid, txt);
    release_jstr(env, collection, col);
    release_jstr(env, vecId, vid);
    release_jstr(env, collectionId, cid);
    release_jstr(env, text, txt);
    return rc;
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeFtsSearch(JNIEnv* env, jclass cls,
        jlong handle, jstring collection, jstring query, jint topK) {
    const char* col = jstr_to_c(env, collection);
    const char* q = jstr_to_c(env, query);
    char* result = agentdb_fts_search((AgentDbHandle*)(intptr_t)handle, col, q, (size_t)topK);
    release_jstr(env, collection, col);
    release_jstr(env, query, q);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeHybridQuery(JNIEnv* env, jclass cls,
        jlong handle, jstring anchorNode, jfloatArray embedding,
        jstring collection, jint graphDepth, jint topK, jdouble alpha) {
    const char* anchor = jstr_to_c(env, anchorNode);
    const char* col = jstr_to_c(env, collection);

    jsize dim = (*env)->GetArrayLength(env, embedding);
    jfloat* emb = (*env)->GetFloatArrayElements(env, embedding, NULL);

    char* result = agentdb_hybrid_query((AgentDbHandle*)(intptr_t)handle,
        anchor, emb, (size_t)dim, col, (size_t)graphDepth, (size_t)topK, alpha);

    (*env)->ReleaseFloatArrayElements(env, embedding, emb, JNI_ABORT);
    release_jstr(env, anchorNode, anchor);
    release_jstr(env, collection, col);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeStats(JNIEnv* env, jclass cls, jlong handle) {
    char* result = agentdb_stats((AgentDbHandle*)(intptr_t)handle);
    return c_to_jstr_free(env, result);
}

JNIEXPORT jstring JNICALL
Java_com_datacules_agentdb_AgentDB_nativeLastError(JNIEnv* env, jclass cls) {
    char* err = agentdb_last_error();
    return c_to_jstr_free(env, err);
}
