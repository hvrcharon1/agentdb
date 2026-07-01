using System;
using System.Runtime.InteropServices;

namespace Datacules.AgentDB
{
    /// <summary>
    /// AgentDB — .NET P/Invoke wrapper.
    ///
    /// <para>AgentDB is an embedded database engine that combines SQL (SQLite),
    /// a vector store, a memory graph, and full-text search in a single
    /// shared library.</para>
    ///
    /// <para><b>Loading the native library</b><br/>
    /// The P/Invoke declarations reference the library by its base name
    /// <c>"agentdb"</c>.  The .NET runtime resolves this to
    /// <c>libagentdb.so</c> on Linux, <c>libagentdb.dylib</c> on macOS, and
    /// <c>agentdb.dll</c> on Windows.  Place the library on your
    /// <c>LD_LIBRARY_PATH</c> / <c>DYLD_LIBRARY_PATH</c> / <c>PATH</c> or
    /// in the same directory as the application binary.</para>
    ///
    /// <para><b>Thread safety</b><br/>
    /// Each <see cref="AgentDB"/> instance must be used from one thread at a
    /// time.  The error-state (<c>agentdb_last_error</c>) is thread-local in
    /// the native library, which matches .NET's P/Invoke threading model.</para>
    ///
    /// <para><b>Disposal</b><br/>
    /// Always dispose via <c>using</c> or call <see cref="Dispose"/> explicitly.
    /// A finalizer provides a safety net but must not be relied upon in
    /// production.</para>
    /// </summary>
    public sealed class AgentDB : IDisposable
    {
        // ── P/Invoke declarations ─────────────────────────────────────────

        private const string LibName = "agentdb";

        [DllImport(LibName, EntryPoint = "agentdb_open",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeOpen(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path);

        [DllImport(LibName, EntryPoint = "agentdb_close",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern void NativeClose(IntPtr handle);

        [DllImport(LibName, EntryPoint = "agentdb_execute",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern long NativeExecute(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sql);

        [DllImport(LibName, EntryPoint = "agentdb_query_json",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeQueryJson(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sql);

        [DllImport(LibName, EntryPoint = "agentdb_vector_upsert",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeVectorUpsert(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
            float[] vector,
            UIntPtr dim,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? metadataJson);

        [DllImport(LibName, EntryPoint = "agentdb_vector_search",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeVectorSearch(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            float[] query,
            UIntPtr dim,
            UIntPtr topK,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? filterJson);

        [DllImport(LibName, EntryPoint = "agentdb_graph_add_node",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGraphAddNode(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string kind,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? dataJson);

        [DllImport(LibName, EntryPoint = "agentdb_graph_add_edge",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGraphAddEdge(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string src,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string dst,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string relation,
            double weight);

        [DllImport(LibName, EntryPoint = "agentdb_graph_neighbors",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeGraphNeighbors(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string nodeId,
            UIntPtr maxDepth,
            double minWeight,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? relation);

        [DllImport(LibName, EntryPoint = "agentdb_fts_index",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeFtsIndex(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string vecId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collectionId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string text);

        [DllImport(LibName, EntryPoint = "agentdb_fts_search",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeFtsSearch(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string query,
            UIntPtr topK);

        [DllImport(LibName, EntryPoint = "agentdb_hybrid_query",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeHybridQuery(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string anchorNode,
            float[] embedding,
            UIntPtr dim,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            UIntPtr graphDepth,
            UIntPtr topK,
            double alpha,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? filterJson);

        [DllImport(LibName, EntryPoint = "agentdb_stats",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeStats(IntPtr handle);

        [DllImport(LibName, EntryPoint = "agentdb_last_error",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeLastError();

        [DllImport(LibName, EntryPoint = "agentdb_vector_delete",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeVectorDelete(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

        [DllImport(LibName, EntryPoint = "agentdb_drop_collection",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeDropCollection(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection);

        [DllImport(LibName, EntryPoint = "agentdb_reindex",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeReindex(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection);

        [DllImport(LibName, EntryPoint = "agentdb_graph_get_node",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeGraphGetNode(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

        [DllImport(LibName, EntryPoint = "agentdb_graph_delete_node",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGraphDeleteNode(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id);

        [DllImport(LibName, EntryPoint = "agentdb_graph_delete_edge",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeGraphDeleteEdge(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string src,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string dst,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string relation);

        [DllImport(LibName, EntryPoint = "agentdb_fts_delete",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeFtsDelete(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string vecId);

        [DllImport(LibName, EntryPoint = "agentdb_fts_optimize",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeFtsOptimize(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string collection);

        [DllImport(LibName, EntryPoint = "agentdb_workflow_fail",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeWorkflowFail(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string id,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? error);

        // ── Tool Registry ─────────────────────────────────────────────────

        [DllImport(LibName, EntryPoint = "agentdb_tool_register",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeToolRegister(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? description,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? parametersSchema,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? version);

        [DllImport(LibName, EntryPoint = "agentdb_tool_list",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeToolList(IntPtr handle);

        [DllImport(LibName, EntryPoint = "agentdb_tool_log_call",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeToolLogCall(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? sessionId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string toolName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? arguments,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? result,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? error,
            long latencyMs);

        // ── Audit Log ────────────────────────────────────────────────────

        [DllImport(LibName, EntryPoint = "agentdb_audit_log",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeAuditLog(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? actor,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string action,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string tableName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string recordId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? oldValue,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? newValue,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? reason);

        [DllImport(LibName, EntryPoint = "agentdb_audit_query_recent",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeAuditQueryRecent(IntPtr handle, UIntPtr limit);

        // ── Context Window ───────────────────────────────────────────────

        [DllImport(LibName, EntryPoint = "agentdb_context_add",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeContextAdd(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sourceType,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sourceId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? contentPreview,
            long tokenCount,
            double relevanceScore,
            long priority);

        [DllImport(LibName, EntryPoint = "agentdb_context_build_window",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeContextBuildWindow(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId,
            long maxTokens);

        [DllImport(LibName, EntryPoint = "agentdb_context_clear",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeContextClear(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string sessionId);

        // ── Prompt Templates ─────────────────────────────────────────────

        [DllImport(LibName, EntryPoint = "agentdb_prompt_create",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativePromptCreate(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string template,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? modelHint,
            long maxTokens,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? metadata);

        [DllImport(LibName, EntryPoint = "agentdb_prompt_render",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativePromptRender(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string varsJson);

        // ── Data Labels (Privacy) ────────────────────────────────────────

        [DllImport(LibName, EntryPoint = "agentdb_label_tag",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeLabelTag(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string tableName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string recordId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string label,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? taggedBy);

        [DllImport(LibName, EntryPoint = "agentdb_label_untag",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeLabelUntag(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string tableName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string recordId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string label);

        [DllImport(LibName, EntryPoint = "agentdb_label_get",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr NativeLabelGet(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string tableName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string recordId);

        [DllImport(LibName, EntryPoint = "agentdb_label_has",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern int NativeLabelHas(
            IntPtr handle,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string tableName,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string recordId,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string label);

        [DllImport(LibName, EntryPoint = "agentdb_free_string",
            CallingConvention = CallingConvention.Cdecl)]
        private static extern void NativeFreeString(IntPtr ptr);

        // ── Instance state ────────────────────────────────────────────────

        private IntPtr _handle;
        private bool _disposed;

        private AgentDB(IntPtr handle)
        {
            _handle = handle;
        }

        // ── Public API ────────────────────────────────────────────────────

        /// <summary>
        /// Open or create an AgentDB database at <paramref name="path"/>.
        /// Use <c>":memory:"</c> for an ephemeral in-memory database.
        /// </summary>
        /// <param name="path">File system path or <c>":memory:"</c>.</param>
        /// <returns>An open <see cref="AgentDB"/> instance.</returns>
        /// <exception cref="AgentDBException">Thrown if the database cannot be opened.</exception>
        public static AgentDB Open(string path)
        {
            IntPtr h = NativeOpen(path);
            if (h == IntPtr.Zero)
                throw new AgentDBException(GetLastError("agentdb_open returned null handle"));
            return new AgentDB(h);
        }

        /// <summary>
        /// Release all native resources. Safe to call multiple times.
        /// </summary>
        public void Dispose()
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero)
                {
                    NativeClose(_handle);
                    _handle = IntPtr.Zero;
                }
                _disposed = true;
                GC.SuppressFinalize(this);
            }
        }

        /// <summary>Finalizer — safety net only. Prefer <c>using</c> blocks.</summary>
        ~AgentDB() => Dispose();

        // ── SQL ───────────────────────────────────────────────────────────

        /// <summary>
        /// Execute a raw SQL statement (DDL or DML). Returns the number of
        /// rows affected.
        /// </summary>
        /// <param name="sql">SQL statement to execute.</param>
        /// <returns>Number of rows affected.</returns>
        /// <exception cref="AgentDBException">Thrown on SQL error.</exception>
        public long Execute(string sql)
        {
            CheckOpen();
            long n = NativeExecute(_handle, sql);
            if (n == -1)
                throw new AgentDBException(GetLastError("agentdb_execute failed"));
            return n;
        }

        /// <summary>
        /// Execute a SELECT statement and return all rows as a JSON array.
        /// Each element is a JSON object whose keys are the column names.
        /// </summary>
        /// <param name="sql">SELECT statement.</param>
        /// <returns>JSON array string, e.g. <c>[{"id":"1","name":"Alice"}]</c>.</returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string QueryJson(string sql)
        {
            CheckOpen();
            IntPtr ptr = NativeQueryJson(_handle, sql);
            return ConsumeStringPtr(ptr, "agentdb_query_json failed");
        }

        // ── Vector store ──────────────────────────────────────────────────

        /// <summary>
        /// Upsert a vector into <paramref name="collection"/>.
        /// The collection is created automatically if it does not exist.
        /// </summary>
        /// <param name="collection">Collection name.</param>
        /// <param name="id">Unique document identifier.</param>
        /// <param name="vector">Embedding values.</param>
        /// <param name="metadataJson">Optional JSON metadata object, or <c>null</c>.</param>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void VectorUpsert(string collection, string id, float[] vector,
            string? metadataJson = null)
        {
            CheckOpen();
            if (vector is null || vector.Length == 0)
                throw new ArgumentException("vector must not be empty", nameof(vector));

            int rc = NativeVectorUpsert(_handle, collection, id, vector,
                (UIntPtr)vector.Length, metadataJson);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_vector_upsert failed"));
        }

        /// <summary>
        /// Find the nearest vectors to <paramref name="query"/> in
        /// <paramref name="collection"/>.
        /// </summary>
        /// <param name="collection">Collection name.</param>
        /// <param name="query">Query embedding.</param>
        /// <param name="topK">Maximum results to return.</param>
        /// <param name="filterJson">
        ///   Optional MongoDB-style metadata filter JSON, or <c>null</c>.
        /// </param>
        /// <returns>JSON array of <c>{"id", "score", "metadata"}</c> objects.</returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string VectorSearch(string collection, float[] query, int topK,
            string? filterJson = null)
        {
            CheckOpen();
            if (query is null || query.Length == 0)
                throw new ArgumentException("query must not be empty", nameof(query));

            IntPtr ptr = NativeVectorSearch(_handle, collection, query,
                (UIntPtr)query.Length, (UIntPtr)topK, filterJson);
            return ConsumeStringPtr(ptr, "agentdb_vector_search failed");
        }

        // ── Memory graph ──────────────────────────────────────────────────

        /// <summary>Add or update a node in the memory graph.</summary>
        /// <param name="id">Unique node identifier.</param>
        /// <param name="kind">Node type label (e.g. <c>"session"</c>, <c>"concept"</c>).</param>
        /// <param name="dataJson">Optional JSON metadata, or <c>null</c>.</param>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void GraphAddNode(string id, string kind, string? dataJson = null)
        {
            CheckOpen();
            int rc = NativeGraphAddNode(_handle, id, kind, dataJson);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_graph_add_node failed"));
        }

        /// <summary>Add or update a directed, weighted edge in the memory graph.</summary>
        /// <param name="src">Source node id.</param>
        /// <param name="dst">Destination node id.</param>
        /// <param name="relation">Relation label.</param>
        /// <param name="weight">Edge weight (0.0–1.0 recommended).</param>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void GraphAddEdge(string src, string dst, string relation, double weight)
        {
            CheckOpen();
            int rc = NativeGraphAddEdge(_handle, src, dst, relation, weight);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_graph_add_edge failed"));
        }

        /// <summary>
        /// Traverse the memory graph outward from <paramref name="nodeId"/>.
        /// </summary>
        /// <param name="nodeId">Anchor node identifier.</param>
        /// <param name="maxDepth">Maximum hops to traverse.</param>
        /// <param name="minWeight">Minimum edge weight to follow (0.0 = follow all).</param>
        /// <returns>JSON array of <c>{"id", "kind", "depth", "weight", "data"}</c> objects.</returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string GraphNeighbors(string nodeId, int maxDepth, double minWeight = 0.0, string? relation = null)
        {
            CheckOpen();
            IntPtr ptr = NativeGraphNeighbors(_handle, nodeId, (UIntPtr)maxDepth, minWeight, relation);
            return ConsumeStringPtr(ptr, "agentdb_graph_neighbors failed");
        }

        // ── Full-text search ──────────────────────────────────────────────

        /// <summary>Index a text document for full-text search.</summary>
        /// <param name="collection">FTS collection name.</param>
        /// <param name="vecId">Correlation key back to a vector entry.</param>
        /// <param name="collectionId">Correlation key for the vector collection.</param>
        /// <param name="text">Document text to index.</param>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void FtsIndex(string collection, string vecId, string collectionId, string text)
        {
            CheckOpen();
            int rc = NativeFtsIndex(_handle, collection, vecId, collectionId, text);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_fts_index failed"));
        }

        /// <summary>Run a full-text search query.</summary>
        /// <param name="collection">Collection to search.</param>
        /// <param name="query">Search query string.</param>
        /// <param name="topK">Maximum results.</param>
        /// <returns>JSON array of <c>{"id", "snippet", "rank"}</c> objects.</returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string FtsSearch(string collection, string query, int topK)
        {
            CheckOpen();
            IntPtr ptr = NativeFtsSearch(_handle, collection, query, (UIntPtr)topK);
            return ConsumeStringPtr(ptr, "agentdb_fts_search failed");
        }

        // ── Hybrid query ──────────────────────────────────────────────────

        /// <summary>
        /// Run a hybrid graph-traversal + vector-similarity query.
        /// </summary>
        /// <param name="anchorNode">Starting node id for graph traversal.</param>
        /// <param name="embedding">Query embedding.</param>
        /// <param name="collection">Vector collection name.</param>
        /// <param name="graphDepth">Maximum graph traversal depth.</param>
        /// <param name="topK">Results to return.</param>
        /// <param name="alpha">
        ///   Blend factor: 0.0 = pure graph ranking, 1.0 = pure vector ranking.
        /// </param>
        /// <param name="filterJson">
        ///   Optional MongoDB-style metadata filter JSON, or <c>null</c>.
        /// </param>
        /// <returns>
        ///   JSON array of <c>{"id", "rank_score", "vector_score", "graph_weight"}</c> objects.
        /// </returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string HybridQuery(string anchorNode, float[] embedding, string collection,
            int graphDepth, int topK, double alpha, string? filterJson = null)
        {
            CheckOpen();
            if (embedding is null || embedding.Length == 0)
                throw new ArgumentException("embedding must not be empty", nameof(embedding));

            IntPtr ptr = NativeHybridQuery(_handle, anchorNode, embedding,
                (UIntPtr)embedding.Length, collection,
                (UIntPtr)graphDepth, (UIntPtr)topK, alpha, filterJson);
            return ConsumeStringPtr(ptr, "agentdb_hybrid_query failed");
        }

        // ── Additional vector operations ──────────────────────────────────────

        /// <summary>Delete a vector by id from collection.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void VectorDelete(string collection, string id)
        {
            CheckOpen();
            int rc = NativeVectorDelete(_handle, collection, id);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_vector_delete failed"));
        }

        /// <summary>Drop a vector collection and all its data permanently.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void DropCollection(string collection)
        {
            CheckOpen();
            int rc = NativeDropCollection(_handle, collection);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_drop_collection failed"));
        }

        /// <summary>Force a full HNSW index rebuild for collection.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void Reindex(string collection)
        {
            CheckOpen();
            int rc = NativeReindex(_handle, collection);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_reindex failed"));
        }

        // ── Additional graph operations ───────────────────────────────────────

        /// <summary>Get a single memory graph node by id as a JSON object.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string GraphGetNode(string id)
        {
            CheckOpen();
            IntPtr ptr = NativeGraphGetNode(_handle, id);
            return ConsumeStringPtr(ptr, "agentdb_graph_get_node failed");
        }

        /// <summary>Delete a node (and its incident edges) from the memory graph.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void GraphDeleteNode(string id)
        {
            CheckOpen();
            int rc = NativeGraphDeleteNode(_handle, id);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_graph_delete_node failed"));
        }

        /// <summary>Delete a directed edge from the memory graph.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void GraphDeleteEdge(string src, string dst, string relation)
        {
            CheckOpen();
            int rc = NativeGraphDeleteEdge(_handle, src, dst, relation);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_graph_delete_edge failed"));
        }

        // ── Additional FTS operations ─────────────────────────────────────────

        /// <summary>Delete a document from the FTS index.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void FtsDelete(string collection, string vecId)
        {
            CheckOpen();
            int rc = NativeFtsDelete(_handle, collection, vecId);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_fts_delete failed"));
        }

        /// <summary>Merge FTS index segments for better query performance.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void FtsOptimize(string collection)
        {
            CheckOpen();
            int rc = NativeFtsOptimize(_handle, collection);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_fts_optimize failed"));
        }

        // ── Additional workflow operations ────────────────────────────────────

        /// <summary>Mark a workflow as failed with an optional error message.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void WorkflowFail(string id, string? error = null)
        {
            CheckOpen();
            int rc = NativeWorkflowFail(_handle, id, error);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_workflow_fail failed"));
        }

        // ── Tool Registry ────────────────────────────────────────────────────

        /// <summary>Register or update a tool definition. Returns JSON with tool ID.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string ToolRegister(string name, string? description = null,
            string? parametersSchema = null, string? version = null)
        {
            CheckOpen();
            IntPtr ptr = NativeToolRegister(_handle, name, description, parametersSchema, version);
            return ConsumeStringPtr(ptr, "agentdb_tool_register failed");
        }

        /// <summary>List all registered tools as a JSON array.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string ToolList()
        {
            CheckOpen();
            IntPtr ptr = NativeToolList(_handle);
            return ConsumeStringPtr(ptr, "agentdb_tool_list failed");
        }

        /// <summary>Log a tool call invocation. Returns JSON with tool call ID.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string ToolLogCall(string toolName, string? sessionId = null,
            string? arguments = null, string? result = null,
            string? error = null, long latencyMs = 0)
        {
            CheckOpen();
            IntPtr ptr = NativeToolLogCall(_handle, sessionId, toolName, arguments,
                result, error, latencyMs);
            return ConsumeStringPtr(ptr, "agentdb_tool_log_call failed");
        }

        // ── Audit Log ────────────────────────────────────────────────────────

        /// <summary>Append an entry to the immutable audit log. Returns JSON with entry ID.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string AuditLog(string action, string tableName, string recordId,
            string? actor = null, string? oldValue = null,
            string? newValue = null, string? reason = null)
        {
            CheckOpen();
            IntPtr ptr = NativeAuditLog(_handle, actor, action, tableName, recordId,
                oldValue, newValue, reason);
            return ConsumeStringPtr(ptr, "agentdb_audit_log failed");
        }

        /// <summary>Query recent audit log entries as a JSON array.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string AuditQueryRecent(int limit = 100)
        {
            CheckOpen();
            IntPtr ptr = NativeAuditQueryRecent(_handle, (UIntPtr)limit);
            return ConsumeStringPtr(ptr, "agentdb_audit_query_recent failed");
        }

        // ── Context Window ───────────────────────────────────────────────────

        /// <summary>Add an entry to the context window. Returns JSON with entry ID.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string ContextAdd(string sessionId, string sourceType, string sourceId,
            string? contentPreview, long tokenCount, double relevanceScore, long priority)
        {
            CheckOpen();
            IntPtr ptr = NativeContextAdd(_handle, sessionId, sourceType, sourceId,
                contentPreview, tokenCount, relevanceScore, priority);
            return ConsumeStringPtr(ptr, "agentdb_context_add failed");
        }

        /// <summary>Build a token-budgeted context window. Returns JSON array of entries.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string ContextBuildWindow(string sessionId, long maxTokens)
        {
            CheckOpen();
            IntPtr ptr = NativeContextBuildWindow(_handle, sessionId, maxTokens);
            return ConsumeStringPtr(ptr, "agentdb_context_build_window failed");
        }

        /// <summary>Clear all context entries for a session.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void ContextClear(string sessionId)
        {
            CheckOpen();
            int rc = NativeContextClear(_handle, sessionId);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_context_clear failed"));
        }

        // ── Prompt Templates ─────────────────────────────────────────────────

        /// <summary>Create a new version of a prompt template. Returns JSON with template ID.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string PromptCreate(string name, string template,
            string? modelHint = null, long maxTokens = 0, string? metadata = null)
        {
            CheckOpen();
            IntPtr ptr = NativePromptCreate(_handle, name, template, modelHint, maxTokens, metadata);
            return ConsumeStringPtr(ptr, "agentdb_prompt_create failed");
        }

        /// <summary>Render a prompt template with placeholder substitution.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string PromptRender(string name, string varsJson)
        {
            CheckOpen();
            IntPtr ptr = NativePromptRender(_handle, name, varsJson);
            return ConsumeStringPtr(ptr, "agentdb_prompt_render failed");
        }

        // ── Data Labels (Privacy) ────────────────────────────────────────────

        /// <summary>Tag a record with a privacy/classification label.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void LabelTag(string tableName, string recordId, string label,
            string? taggedBy = null)
        {
            CheckOpen();
            int rc = NativeLabelTag(_handle, tableName, recordId, label, taggedBy);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_label_tag failed"));
        }

        /// <summary>Remove a specific label from a record.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public void LabelUntag(string tableName, string recordId, string label)
        {
            CheckOpen();
            int rc = NativeLabelUntag(_handle, tableName, recordId, label);
            if (rc != 0)
                throw new AgentDBException(GetLastError("agentdb_label_untag failed"));
        }

        /// <summary>Get all labels for a record as a JSON array.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string LabelGet(string tableName, string recordId)
        {
            CheckOpen();
            IntPtr ptr = NativeLabelGet(_handle, tableName, recordId);
            return ConsumeStringPtr(ptr, "agentdb_label_get failed");
        }

        /// <summary>Check if a record has a specific label.</summary>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public bool LabelHas(string tableName, string recordId, string label)
        {
            CheckOpen();
            int rc = NativeLabelHas(_handle, tableName, recordId, label);
            if (rc == -1)
                throw new AgentDBException(GetLastError("agentdb_label_has failed"));
            return rc == 1;
        }

        // ── Stats ─────────────────────────────────────────────────────────

        /// <summary>
        /// Return a JSON object with database statistics:
        /// <c>collections</c>, <c>vectors</c>, <c>nodes</c>, <c>edges</c>.
        /// </summary>
        /// <returns>JSON object string.</returns>
        /// <exception cref="AgentDBException">Thrown on error.</exception>
        public string Stats()
        {
            CheckOpen();
            IntPtr ptr = NativeStats(_handle);
            return ConsumeStringPtr(ptr, "agentdb_stats failed");
        }

        // ── Internal helpers ──────────────────────────────────────────────

        private void CheckOpen()
        {
            if (_disposed || _handle == IntPtr.Zero)
                throw new ObjectDisposedException(nameof(AgentDB));
        }

        /// <summary>
        /// Convert a heap-allocated C string returned by AgentDB to a managed
        /// string, then free the native memory.
        /// </summary>
        private string ConsumeStringPtr(IntPtr ptr, string fallback)
        {
            if (ptr == IntPtr.Zero)
                throw new AgentDBException(GetLastError(fallback));
            try
            {
                return Marshal.PtrToStringUTF8(ptr)
                    ?? throw new AgentDBException(fallback);
            }
            finally
            {
                NativeFreeString(ptr);
            }
        }

        /// <summary>
        /// Retrieve and clear the thread-local last-error from the native
        /// library. Returns <paramref name="fallback"/> if none is set.
        /// </summary>
        private static string GetLastError(string fallback)
        {
            IntPtr ptr = NativeLastError();
            if (ptr == IntPtr.Zero) return fallback;
            try
            {
                return Marshal.PtrToStringUTF8(ptr) ?? fallback;
            }
            finally
            {
                NativeFreeString(ptr);
            }
        }
    }
}
