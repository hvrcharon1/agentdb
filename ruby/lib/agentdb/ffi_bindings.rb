# frozen_string_literal: true

require "ffi"
require_relative "errors"

module AgentDB
  # Low-level FFI bindings that map directly to the agentdb C shared library.
  #
  # Consumers should use the higher-level AgentDB::Database and
  # AgentDB::Collection classes rather than calling these functions directly.
  #
  # Memory contract:
  #   - All char* values returned by agentdb_* functions are heap-allocated.
  #     They MUST be freed with agentdb_free_string() — never Ruby's GC alone.
  #   - Input const char* parameters are borrowed for the duration of the call.
  #   - AgentDbHandle* is opaque; always close with agentdb_close().
  module FFIBindings
    extend FFI::Library

    # ── Library loading ────────────────────────────────────────────────────

    # Build the candidate list in priority order so we find the library on
    # every supported platform without hard-coding an absolute path.
    def self.lib_candidates
      base = "agentdb"
      search_dirs = [
        # Sibling cargo output (when running from the repo)
        File.expand_path("../../../target/release", __dir__),
        File.expand_path("../../../target/debug", __dir__),
        # Standard system locations
        "/usr/local/lib",
        "/usr/lib",
        # macOS Homebrew prefix
        "/opt/homebrew/lib",
      ]

      candidates = ["libagentdb", "agentdb"]

      # Add directory-qualified paths for each extension
      exts = case RbConfig::CONFIG["host_os"]
             when /darwin/ then ["dylib", "so"]
             when /mswin|mingw|cygwin/ then ["dll"]
             else ["so"]
             end

      paths = []
      search_dirs.each do |dir|
        candidates.each do |name|
          exts.each do |ext|
            paths << File.join(dir, "#{name}.#{ext}")
          end
        end
      end

      # Also try bare names so FFI can apply its own search path logic
      candidates.each do |name|
        exts.each { |ext| paths << "#{name}.#{ext}" }
      end
      paths << base  # last-resort: let FFI search LD_LIBRARY_PATH / PATH
      paths
    end

    begin
      ffi_lib lib_candidates
    rescue LoadError => e
      raise AgentDB::LibraryNotFoundError,
            "Could not load the agentdb shared library. " \
            "Build it first: `cargo build --release --features ffi --lib` " \
            "then ensure the resulting .so/.dylib/.dll is on your library path. " \
            "(#{e.message})"
    end

    # ── Type aliases ────────────────────────────────────────────────────────
    #
    # :size_t  maps to C uintptr_t / size_t  (used for dim, top_k, etc.)
    # :int64   maps to C int64_t              (used for execute row count)
    # :int32   maps to C int32_t              (used for boolean-style return codes)
    # :double  maps to C double
    # :float   maps to C float (only appears as pointer-to-float array)

    # ── Lifecycle ──────────────────────────────────────────────────────────

    # char *agentdb_last_error(void);
    attach_function :agentdb_last_error, [], :pointer

    # void agentdb_free_string(char *ptr);
    attach_function :agentdb_free_string, [:pointer], :void

    # AgentDbHandle *agentdb_open(const char *path);
    attach_function :agentdb_open, [:string], :pointer

    # void agentdb_close(AgentDbHandle *handle);
    attach_function :agentdb_close, [:pointer], :void

    # ── SQL ────────────────────────────────────────────────────────────────

    # int64_t agentdb_execute(AgentDbHandle *handle, const char *sql);
    attach_function :agentdb_execute, [:pointer, :string], :int64

    # char *agentdb_query_json(AgentDbHandle *handle, const char *sql);
    attach_function :agentdb_query_json, [:pointer, :string], :pointer

    # char *agentdb_query_json_params(AgentDbHandle *handle, const char *sql,
    #                                  const char *params_json);
    attach_function :agentdb_query_json_params, [:pointer, :string, :string], :pointer

    # ── Vector store ───────────────────────────────────────────────────────

    # int32_t agentdb_vector_upsert(AgentDbHandle *handle,
    #                               const char *collection, const char *id,
    #                               const float *vector, uintptr_t dim,
    #                               const char *metadata);
    attach_function :agentdb_vector_upsert,
                    [:pointer, :string, :string, :pointer, :size_t, :string],
                    :int32

    # char *agentdb_vector_search(AgentDbHandle *handle,
    #                             const char *collection,
    #                             const float *query, uintptr_t dim,
    #                             uintptr_t top_k,
    #                             const char *filter_json);
    attach_function :agentdb_vector_search,
                    [:pointer, :string, :pointer, :size_t, :size_t, :string],
                    :pointer

    # int32_t agentdb_vector_delete(AgentDbHandle *handle,
    #                               const char *collection, const char *id,
    #                               uintptr_t dim);
    attach_function :agentdb_vector_delete, [:pointer, :string, :string, :size_t], :int32

    # int32_t agentdb_drop_collection(AgentDbHandle *handle,
    #                                 const char *collection);
    attach_function :agentdb_drop_collection, [:pointer, :string], :int32

    # int32_t agentdb_reindex(AgentDbHandle *handle,
    #                         const char *collection, uintptr_t dim);
    attach_function :agentdb_reindex, [:pointer, :string, :size_t], :int32

    # ── Memory graph ───────────────────────────────────────────────────────

    # int32_t agentdb_graph_add_node(AgentDbHandle *handle,
    #                                const char *id, const char *kind,
    #                                const char *data_json);
    attach_function :agentdb_graph_add_node, [:pointer, :string, :string, :string], :int32

    # int32_t agentdb_graph_add_edge(AgentDbHandle *handle,
    #                                const char *src, const char *dst,
    #                                const char *relation, double weight);
    attach_function :agentdb_graph_add_edge, [:pointer, :string, :string, :string, :double], :int32

    # char *agentdb_graph_neighbors(AgentDbHandle *handle,
    #                               const char *node_id, uintptr_t max_depth,
    #                               double min_weight, const char *relation);
    attach_function :agentdb_graph_neighbors,
                    [:pointer, :string, :size_t, :double, :string],
                    :pointer

    # char *agentdb_graph_get_node(AgentDbHandle *handle, const char *id);
    attach_function :agentdb_graph_get_node, [:pointer, :string], :pointer

    # int32_t agentdb_graph_delete_node(AgentDbHandle *handle, const char *id);
    attach_function :agentdb_graph_delete_node, [:pointer, :string], :int32

    # int32_t agentdb_graph_delete_edge(AgentDbHandle *handle,
    #                                   const char *src, const char *dst,
    #                                   const char *relation);
    attach_function :agentdb_graph_delete_edge, [:pointer, :string, :string, :string], :int32

    # ── Full-text search ───────────────────────────────────────────────────

    # int32_t agentdb_fts_index(AgentDbHandle *handle,
    #                           const char *collection, const char *vec_id,
    #                           const char *collection_id, const char *text);
    attach_function :agentdb_fts_index, [:pointer, :string, :string, :string, :string], :int32

    # char *agentdb_fts_search(AgentDbHandle *handle,
    #                          const char *collection, const char *query,
    #                          uintptr_t top_k);
    attach_function :agentdb_fts_search, [:pointer, :string, :string, :size_t], :pointer

    # int32_t agentdb_fts_delete(AgentDbHandle *handle,
    #                            const char *collection, const char *vec_id);
    attach_function :agentdb_fts_delete, [:pointer, :string, :string], :int32

    # int32_t agentdb_fts_optimize(AgentDbHandle *handle,
    #                              const char *collection);
    attach_function :agentdb_fts_optimize, [:pointer, :string], :int32

    # ── Hybrid query ───────────────────────────────────────────────────────

    # char *agentdb_hybrid_query(AgentDbHandle *handle,
    #                            const char *anchor_node,
    #                            const float *embedding, uintptr_t dim,
    #                            const char *collection,
    #                            uintptr_t graph_depth, uintptr_t top_k,
    #                            double alpha, const char *filter_json);
    attach_function :agentdb_hybrid_query,
                    [:pointer, :string, :pointer, :size_t, :string,
                     :size_t, :size_t, :double, :string],
                    :pointer

    # ── Stats ──────────────────────────────────────────────────────────────

    # char *agentdb_stats(AgentDbHandle *handle);
    attach_function :agentdb_stats, [:pointer], :pointer

    # ── Conversations ──────────────────────────────────────────────────────

    # int32_t agentdb_conversation_create(AgentDbHandle *handle,
    #                                     const char *id, const char *title,
    #                                     const char *metadata);
    attach_function :agentdb_conversation_create, [:pointer, :string, :string, :string], :int32

    # char *agentdb_conversation_add_message(AgentDbHandle *handle,
    #                                        const char *conversation_id,
    #                                        const char *role, const char *content,
    #                                        const char *metadata);
    attach_function :agentdb_conversation_add_message,
                    [:pointer, :string, :string, :string, :string],
                    :pointer

    # char *agentdb_conversation_get_messages(AgentDbHandle *handle,
    #                                         const char *conversation_id,
    #                                         uintptr_t limit);
    attach_function :agentdb_conversation_get_messages, [:pointer, :string, :size_t], :pointer

    # char *agentdb_conversation_list(AgentDbHandle *handle);
    attach_function :agentdb_conversation_list, [:pointer], :pointer

    # int32_t agentdb_conversation_delete(AgentDbHandle *handle, const char *id);
    attach_function :agentdb_conversation_delete, [:pointer, :string], :int32

    # ── Workflows ──────────────────────────────────────────────────────────

    # int32_t agentdb_workflow_create(AgentDbHandle *handle,
    #                                 const char *id, const char *name,
    #                                 const char *input, const char *metadata);
    attach_function :agentdb_workflow_create,
                    [:pointer, :string, :string, :string, :string],
                    :int32

    # char *agentdb_workflow_add_step(AgentDbHandle *handle,
    #                                 const char *workflow_id, const char *name,
    #                                 const char *input);
    attach_function :agentdb_workflow_add_step, [:pointer, :string, :string, :string], :pointer

    # int32_t agentdb_workflow_update_step(AgentDbHandle *handle,
    #                                      const char *step_id, const char *status,
    #                                      const char *output, const char *error);
    attach_function :agentdb_workflow_update_step,
                    [:pointer, :string, :string, :string, :string],
                    :int32

    # int32_t agentdb_workflow_complete(AgentDbHandle *handle,
    #                                   const char *id, const char *output);
    attach_function :agentdb_workflow_complete, [:pointer, :string, :string], :int32

    # int32_t agentdb_workflow_fail(AgentDbHandle *handle,
    #                               const char *id, const char *error);
    attach_function :agentdb_workflow_fail, [:pointer, :string, :string], :int32

    # char *agentdb_workflow_get(AgentDbHandle *handle, const char *id);
    attach_function :agentdb_workflow_get, [:pointer, :string], :pointer

    # char *agentdb_workflow_list(AgentDbHandle *handle,
    #                             const char *status_filter);
    attach_function :agentdb_workflow_list, [:pointer, :string], :pointer

    # ── Traces ─────────────────────────────────────────────────────────────

    # char *agentdb_trace_add(AgentDbHandle *handle,
    #                         const char *session_id, const char *parent_id,
    #                         const char *trace_type, const char *content,
    #                         const char *metadata);
    attach_function :agentdb_trace_add,
                    [:pointer, :string, :string, :string, :string, :string],
                    :pointer

    # char *agentdb_trace_get_by_session(AgentDbHandle *handle,
    #                                    const char *session_id);
    attach_function :agentdb_trace_get_by_session, [:pointer, :string], :pointer

    # char *agentdb_trace_get_tree(AgentDbHandle *handle, const char *root_id);
    attach_function :agentdb_trace_get_tree, [:pointer, :string], :pointer

    # ── Helpers ────────────────────────────────────────────────────────────

    # Read a heap-allocated C string returned by the library, then immediately
    # free it.  Returns the Ruby String or nil if ptr is NULL.
    def self.read_and_free(ptr)
      return nil if ptr.null?

      begin
        ptr.read_string.dup.force_encoding(Encoding::UTF_8)
      ensure
        agentdb_free_string(ptr)
      end
    end

    # Read the current last-error string and clear it by reading.
    # Returns nil if there is no pending error.
    def self.last_error
      ptr = agentdb_last_error
      read_and_free(ptr)
    end

    # Pack a Ruby Array of Floats into a native float32 buffer suitable for
    # passing to vector functions.  Returns [FFI::MemoryPointer, size].
    def self.pack_floats(array)
      floats = array.map(&:to_f)
      buf = FFI::MemoryPointer.new(:float, floats.size)
      buf.put_array_of_float(0, floats)
      [buf, floats.size]
    end
  end
end
