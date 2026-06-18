package com.datacules.agentdb;

import java.io.Closeable;

/**
 * AgentDB — Java JNI wrapper.
 *
 * <p>AgentDB is an embedded database engine that combines SQL (SQLite),
 * a vector store, a memory graph, and full-text search in a single
 * shared library.
 *
 * <h2>Loading the native library</h2>
 * <p>By default {@code AgentDB.open()} calls
 * {@code System.loadLibrary("agentdb")}, which searches
 * {@code java.library.path}. You can load the library yourself before
 * calling {@code open} by calling {@link #loadLibrary(String)} with an
 * absolute path, or by setting the JVM flag:
 * <pre>  -Djava.library.path=/path/to/libagentdb</pre>
 *
 * <h2>Thread safety</h2>
 * <p>Each {@code AgentDB} instance must be used from a single thread at a
 * time. Concurrent access requires external synchronisation.
 *
 * <h2>Memory contract</h2>
 * <p>Strings returned by native methods are freed inside the JNI glue layer;
 * Java callers never need to call {@code agentdb_free_string} directly.
 */
public class AgentDB implements Closeable {

    // ── Native library loading ────────────────────────────────────────────

    private static volatile boolean libraryLoaded = false;

    /**
     * Load the native {@code agentdb} shared library from the default
     * {@code java.library.path}.  Called automatically by {@link #open}.
     * Safe to call multiple times.
     */
    public static synchronized void loadLibrary() {
        if (!libraryLoaded) {
            System.loadLibrary("agentdb");
            libraryLoaded = true;
        }
    }

    /**
     * Load the native library from an explicit absolute path.
     * Call this before {@link #open} if the library is not on
     * {@code java.library.path}.
     *
     * @param absolutePath absolute filesystem path to the shared library
     *                     (e.g. {@code /opt/agentdb/libagentdb.so})
     */
    public static synchronized void loadLibrary(String absolutePath) {
        if (!libraryLoaded) {
            System.load(absolutePath);
            libraryLoaded = true;
        }
    }

    // ── JNI declarations ─────────────────────────────────────────────────

    /** @return opaque native handle, or 0 on failure */
    private static native long nativeOpen(String path);

    /** Closes the native handle. Null-safe. */
    private static native void nativeClose(long handle);

    /**
     * Execute a raw SQL statement.
     *
     * @return rows affected, or -1 on error
     */
    private static native long nativeExecute(long handle, String sql);

    /**
     * Query and return rows as a JSON array string.
     *
     * @return JSON string, or {@code null} on error
     */
    private static native String nativeQueryJson(long handle, String sql);

    /**
     * Upsert a vector.
     *
     * @return 0 on success, -1 on error
     */
    private static native int nativeVectorUpsert(long handle, String collection,
            String id, float[] vector, String metadataJson);

    /**
     * Search for nearest vectors.
     *
     * @return JSON array string, or {@code null} on error
     */
    private static native String nativeVectorSearch(long handle, String collection,
            float[] query, int topK, String filterJson);

    /**
     * Add or update a memory-graph node.
     *
     * @return 0 on success, -1 on error
     */
    private static native int nativeGraphAddNode(long handle, String id,
            String kind, String dataJson);

    /**
     * Add or update a directed edge.
     *
     * @return 0 on success, -1 on error
     */
    private static native int nativeGraphAddEdge(long handle, String src,
            String dst, String relation, double weight);

    /**
     * Traverse the memory graph.
     *
     * @return JSON array string, or {@code null} on error
     */
    private static native String nativeGraphNeighbors(long handle, String nodeId,
            int maxDepth, double minWeight);

    /**
     * Index a text document for full-text search.
     *
     * @return 0 on success, -1 on error
     */
    private static native int nativeFtsIndex(long handle, String collection,
            String vecId, String collectionId, String text);

    /**
     * Full-text search.
     *
     * @return JSON array string, or {@code null} on error
     */
    private static native String nativeFtsSearch(long handle, String collection,
            String query, int topK);

    /**
     * Hybrid graph + vector query.
     *
     * @return JSON array string, or {@code null} on error
     */
    private static native String nativeHybridQuery(long handle, String anchorNode,
            float[] embedding, String collection, int graphDepth, int topK, double alpha);

    /**
     * Retrieve database statistics.
     *
     * @return JSON object string, or {@code null} on error
     */
    private static native String nativeStats(long handle);

    /**
     * Retrieve the last native error message for the calling thread.
     *
     * @return error string, or {@code null} if none
     */
    private static native String nativeLastError();

    // ── Instance state ────────────────────────────────────────────────────

    private long handle; // 0 when closed

    private AgentDB(long handle) {
        this.handle = handle;
    }

    // ── Public API ────────────────────────────────────────────────────────

    /**
     * Open or create an AgentDB database at {@code path}.
     * Use {@code ":memory:"} for an ephemeral in-memory database.
     *
     * <p>Loads the native library the first time it is called.
     *
     * @param path filesystem path or {@code ":memory:"}
     * @return open database handle
     * @throws AgentDBException if the database cannot be opened
     */
    public static AgentDB open(String path) {
        loadLibrary();
        long h = nativeOpen(path);
        if (h == 0) {
            String err = nativeLastError();
            throw new AgentDBException(err != null ? err : "agentdb_open returned null handle");
        }
        return new AgentDB(h);
    }

    /**
     * Close the database and release all native resources.
     * Idempotent — safe to call more than once.
     */
    @Override
    public void close() {
        if (handle != 0) {
            nativeClose(handle);
            handle = 0;
        }
    }

    // ── SQL ───────────────────────────────────────────────────────────────

    /**
     * Execute a raw SQL statement (DDL or DML, no result rows).
     *
     * @param sql SQL statement
     * @return number of rows affected
     * @throws AgentDBException on SQL error
     */
    public long execute(String sql) {
        checkOpen();
        long n = nativeExecute(handle, sql);
        if (n == -1) {
            throw new AgentDBException(requireLastError("agentdb_execute failed"));
        }
        return n;
    }

    /**
     * Execute a SELECT statement and return all rows as a JSON array.
     * Each element is a JSON object whose keys match the column names.
     *
     * @param sql SELECT statement
     * @return JSON array string (e.g. {@code [{"id":"1","name":"Alice"}]})
     * @throws AgentDBException on error
     */
    public String queryJson(String sql) {
        checkOpen();
        String result = nativeQueryJson(handle, sql);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_query_json failed"));
        }
        return result;
    }

    // ── Vector store ──────────────────────────────────────────────────────

    /**
     * Upsert a vector into {@code collection}. The collection is created
     * automatically if it does not exist.
     *
     * @param collection  collection name
     * @param id          unique document identifier
     * @param vector      embedding values
     * @param metadataJson optional JSON metadata object, or {@code null}
     * @throws AgentDBException on error
     */
    public void vectorUpsert(String collection, String id, float[] vector,
            String metadataJson) {
        checkOpen();
        int rc = nativeVectorUpsert(handle, collection, id, vector, metadataJson);
        if (rc != 0) {
            throw new AgentDBException(requireLastError("agentdb_vector_upsert failed"));
        }
    }

    /**
     * Find the nearest vectors to {@code query} in {@code collection}.
     *
     * @param collection collection name
     * @param query      query embedding
     * @param topK       maximum results to return
     * @param filterJson optional MongoDB-style metadata filter JSON, or {@code null}
     * @return JSON array of {@code {id, score, metadata}} objects
     * @throws AgentDBException on error
     */
    public String vectorSearch(String collection, float[] query, int topK,
            String filterJson) {
        checkOpen();
        String result = nativeVectorSearch(handle, collection, query, topK, filterJson);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_vector_search failed"));
        }
        return result;
    }

    // ── Memory graph ──────────────────────────────────────────────────────

    /**
     * Add or update a node in the memory graph.
     *
     * @param id       unique node identifier
     * @param kind     node type label (e.g. {@code "session"}, {@code "concept"})
     * @param dataJson optional JSON metadata, or {@code null}
     * @throws AgentDBException on error
     */
    public void graphAddNode(String id, String kind, String dataJson) {
        checkOpen();
        int rc = nativeGraphAddNode(handle, id, kind, dataJson);
        if (rc != 0) {
            throw new AgentDBException(requireLastError("agentdb_graph_add_node failed"));
        }
    }

    /**
     * Add or update a directed, weighted edge in the memory graph.
     *
     * @param src      source node id
     * @param dst      destination node id
     * @param relation relation label
     * @param weight   edge weight (0.0–1.0 recommended)
     * @throws AgentDBException on error
     */
    public void graphAddEdge(String src, String dst, String relation, double weight) {
        checkOpen();
        int rc = nativeGraphAddEdge(handle, src, dst, relation, weight);
        if (rc != 0) {
            throw new AgentDBException(requireLastError("agentdb_graph_add_edge failed"));
        }
    }

    /**
     * Traverse the memory graph outward from {@code nodeId}.
     *
     * @param nodeId    anchor node identifier
     * @param maxDepth  maximum hops to traverse
     * @param minWeight minimum edge weight to follow (0.0 = follow all)
     * @return JSON array of {@code {id, kind, depth, weight, data}} objects
     * @throws AgentDBException on error
     */
    public String graphNeighbors(String nodeId, int maxDepth, double minWeight) {
        checkOpen();
        String result = nativeGraphNeighbors(handle, nodeId, maxDepth, minWeight);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_graph_neighbors failed"));
        }
        return result;
    }

    // ── Full-text search ──────────────────────────────────────────────────

    /**
     * Index a text document for full-text search.
     *
     * @param collection   FTS collection name
     * @param vecId        correlation key back to a vector entry
     * @param collectionId correlation key for the vector collection
     * @param text         document text to index
     * @throws AgentDBException on error
     */
    public void ftsIndex(String collection, String vecId, String collectionId,
            String text) {
        checkOpen();
        int rc = nativeFtsIndex(handle, collection, vecId, collectionId, text);
        if (rc != 0) {
            throw new AgentDBException(requireLastError("agentdb_fts_index failed"));
        }
    }

    /**
     * Run a full-text search query.
     *
     * @param collection collection to search
     * @param query      search query string
     * @param topK       maximum results
     * @return JSON array of {@code {id, snippet, rank}} objects
     * @throws AgentDBException on error
     */
    public String ftsSearch(String collection, String query, int topK) {
        checkOpen();
        String result = nativeFtsSearch(handle, collection, query, topK);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_fts_search failed"));
        }
        return result;
    }

    // ── Hybrid query ──────────────────────────────────────────────────────

    /**
     * Run a hybrid graph-traversal + vector-similarity query.
     *
     * @param anchorNode  starting node id for graph traversal
     * @param embedding   query embedding
     * @param collection  vector collection name
     * @param graphDepth  maximum graph traversal depth
     * @param topK        results to return
     * @param alpha       blend factor: 0.0 = pure graph, 1.0 = pure vector
     * @return JSON array of {@code {id, rank_score, vector_score, graph_weight}} objects
     * @throws AgentDBException on error
     */
    public String hybridQuery(String anchorNode, float[] embedding, String collection,
            int graphDepth, int topK, double alpha) {
        checkOpen();
        String result = nativeHybridQuery(handle, anchorNode, embedding,
                collection, graphDepth, topK, alpha);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_hybrid_query failed"));
        }
        return result;
    }

    // ── Stats ─────────────────────────────────────────────────────────────

    /**
     * Return a JSON object with database statistics:
     * {@code collections}, {@code vectors}, {@code nodes}, {@code edges}.
     *
     * @return JSON object string
     * @throws AgentDBException on error
     */
    public String stats() {
        checkOpen();
        String result = nativeStats(handle);
        if (result == null) {
            throw new AgentDBException(requireLastError("agentdb_stats failed"));
        }
        return result;
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    private void checkOpen() {
        if (handle == 0) {
            throw new IllegalStateException("AgentDB has been closed");
        }
    }

    private static String requireLastError(String fallback) {
        String err = nativeLastError();
        return err != null && !err.isEmpty() ? err : fallback;
    }
}
