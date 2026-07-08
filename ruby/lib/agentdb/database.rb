# frozen_string_literal: true

require "json"
require_relative "ffi_bindings"
require_relative "collection"
require_relative "errors"

module AgentDB
  # High-level Ruby interface to an AgentDB database.
  #
  # Example:
  #   db = AgentDB::Database.new("agent.agentdb")
  #
  #   # SQL
  #   db.execute("CREATE TABLE IF NOT EXISTS notes (id TEXT, body TEXT)")
  #   db.execute("INSERT INTO notes VALUES ('n1', 'hello world')")
  #   rows = db.query("SELECT * FROM notes")  # => [{"id"=>"n1","body"=>"hello world"}]
  #
  #   # Vectors
  #   col = db.collection("memories", 1536)
  #   col.upsert("mem1", embedding, { topic: "ruby" })
  #   results = col.search(query_embedding, top_k: 5)
  #
  #   db.close
  #
  # Use a block form to ensure the handle is always closed:
  #   AgentDB::Database.open("agent.agentdb") do |db|
  #     db.execute("SELECT 1")
  #   end
  class Database
    # Open (or create) an AgentDB database at +path+.
    #
    # Pass +":memory:"+ for a transient in-process database.
    #
    # @param path [String] file system path or ":memory:"
    # @raise [AgentDB::DatabaseError] if the library reports an error
    def initialize(path)
      @handle = FFIBindings.agentdb_open(path.to_s)
      if @handle.null?
        msg = FFIBindings.last_error || "agentdb_open returned NULL"
        raise AgentDB::DatabaseError, "Failed to open '#{path}': #{msg}"
      end
      @closed = false
    end

    # Open a database, yield to the block, and close it even if the block
    # raises.
    #
    # @param path [String] file path or ":memory:"
    # @yieldparam db [AgentDB::Database]
    # @return the block's return value
    def self.open(path, &block)
      db = new(path)
      return db unless block_given?

      begin
        block.call(db)
      ensure
        db.close
      end
    end

    # Close the database and release the native handle.
    #
    # Calling close more than once is safe.
    def close
      return if @closed

      FFIBindings.agentdb_close(@handle)
      @closed = true
    end

    # @return [Boolean] true if the database handle has been closed
    def closed?
      @closed
    end

    # Execute a raw SQL statement.
    #
    # @param sql [String] any DDL or DML statement
    # @return [Integer] number of rows affected
    # @raise [AgentDB::FFIError] on SQL error
    def execute(sql)
      ensure_open!
      rc = FFIBindings.agentdb_execute(@handle, sql.to_s)
      if rc == -1
        msg = FFIBindings.last_error || "unknown error"
        raise AgentDB::FFIError, "execute failed: #{msg}"
      end
      rc
    end

    # Query rows and return them as an array of hashes.
    #
    # @param sql [String] SELECT statement
    # @return [Array<Hash>] rows as Ruby hashes with string keys
    # @raise [AgentDB::FFIError] on SQL error
    def query(sql)
      ensure_open!
      ptr = FFIBindings.agentdb_query_json(@handle, sql.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_query_json")
      JSON.parse(json_string)
    end

    # Query rows with positional parameters.
    #
    # Parameters are supplied as a Ruby array and serialised to a JSON array
    # before being passed to the C layer, which maps them to SQL +?+ placeholders.
    #
    # @param sql    [String]  parameterised SELECT, e.g. "SELECT * FROM t WHERE x = ?"
    # @param params [Array]   parameter values (strings, numbers, booleans, nil)
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on SQL error
    def query_params(sql, params = [])
      ensure_open!
      params_json = params.to_json
      ptr = FFIBindings.agentdb_query_json_params(@handle, sql.to_s, params_json)
      json_string = read_json_ptr!(ptr, "agentdb_query_json_params")
      JSON.parse(json_string)
    end

    # Return a Collection object for vector operations.
    #
    # The collection is created inside the database on first upsert if it does
    # not already exist.
    #
    # @param name [String]  collection name
    # @param dim  [Integer] vector dimensionality
    # @return [AgentDB::Collection]
    def collection(name, dim)
      ensure_open!
      Collection.new(@handle, name, dim)
    end

    # Return database statistics as a Hash.
    #
    # Keys: collections, vectors, nodes, edges, conversations, messages,
    #       workflows, workflow_steps, traces, tools, tool_calls,
    #       audit_entries, prompt_templates
    #
    # @return [Hash]
    # @raise [AgentDB::FFIError] on error
    def stats
      ensure_open!
      ptr = FFIBindings.agentdb_stats(@handle)
      json_string = read_json_ptr!(ptr, "agentdb_stats")
      JSON.parse(json_string)
    end

    # ── Memory graph ──────────────────────────────────────────────────────

    # Add or update a node in the memory graph.
    #
    # @param id   [String]     unique node identifier
    # @param kind [String]     node type label (e.g. "session", "concept")
    # @param data [Hash, nil]  arbitrary JSON-serialisable metadata
    # @raise [AgentDB::FFIError] on error
    def graph_add_node(id, kind, data = nil)
      ensure_open!
      data_json = data ? data.to_json : nil
      rc = FFIBindings.agentdb_graph_add_node(@handle, id.to_s, kind.to_s, data_json)
      check_rc!(rc, "agentdb_graph_add_node")
    end

    # Add or update a directed weighted edge between two nodes.
    #
    # @param src      [String] source node ID
    # @param dst      [String] destination node ID
    # @param relation [String] edge type / label
    # @param weight   [Float]  edge weight (0.0–1.0 typical)
    # @raise [AgentDB::FFIError] on error
    def graph_add_edge(src, dst, relation, weight = 1.0)
      ensure_open!
      rc = FFIBindings.agentdb_graph_add_edge(
        @handle, src.to_s, dst.to_s, relation.to_s, weight.to_f
      )
      check_rc!(rc, "agentdb_graph_add_edge")
    end

    # Traverse from a node and return neighbouring nodes.
    #
    # @param node_id    [String]       starting node ID
    # @param max_depth  [Integer]      maximum hops (default: 2)
    # @param min_weight [Float]        minimum edge weight to follow (default: 0.0)
    # @param relation   [String, nil]  filter to a specific edge relation
    # @return [Array<Hash>]  nodes with id, kind, depth, weight, data keys
    # @raise [AgentDB::FFIError] on error
    def graph_neighbors(node_id, max_depth: 2, min_weight: 0.0, relation: nil)
      ensure_open!
      ptr = FFIBindings.agentdb_graph_neighbors(
        @handle, node_id.to_s, max_depth.to_i, min_weight.to_f, relation&.to_s
      )
      json_string = read_json_ptr!(ptr, "agentdb_graph_neighbors")
      JSON.parse(json_string)
    end

    # Fetch a single graph node by ID.
    #
    # @param id [String] node identifier
    # @return [Hash]  node with id, kind, data, created_at, updated_at
    # @raise [AgentDB::FFIError] on error
    def graph_get_node(id)
      ensure_open!
      ptr = FFIBindings.agentdb_graph_get_node(@handle, id.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_graph_get_node")
      JSON.parse(json_string)
    end

    # Delete a graph node and all its connected edges (CASCADE).
    #
    # @param id [String] node identifier
    # @raise [AgentDB::FFIError] on error
    def graph_delete_node(id)
      ensure_open!
      rc = FFIBindings.agentdb_graph_delete_node(@handle, id.to_s)
      check_rc!(rc, "agentdb_graph_delete_node")
    end

    # Delete a specific directed edge.
    #
    # @param src      [String] source node ID
    # @param dst      [String] destination node ID
    # @param relation [String] edge relation / label
    # @raise [AgentDB::FFIError] on error
    def graph_delete_edge(src, dst, relation)
      ensure_open!
      rc = FFIBindings.agentdb_graph_delete_edge(
        @handle, src.to_s, dst.to_s, relation.to_s
      )
      check_rc!(rc, "agentdb_graph_delete_edge")
    end

    # ── Hybrid queries ────────────────────────────────────────────────────

    # Run a hybrid graph-traversal + vector similarity query.
    #
    # @param anchor_node  [String]       graph traversal start node ID
    # @param embedding    [Array<Float>] query vector
    # @param dim          [Integer]      embedding dimensions
    # @param collection   [String]       vector collection name
    # @param graph_depth  [Integer]      max traversal hops (default: 2)
    # @param top_k        [Integer]      results to return (default: 10)
    # @param alpha        [Float]        blending factor 0.0=graph, 1.0=vector
    # @param filter       [Hash, nil]    optional metadata filter
    # @return [Array<Hash>]  id, rank_score, vector_score, graph_weight per result
    # @raise [AgentDB::FFIError] on error
    def hybrid_query(anchor_node, embedding, dim,
                     collection:,
                     graph_depth: 2,
                     top_k: 10,
                     alpha: 0.5,
                     filter: nil)
      ensure_open!
      buf, size = FFIBindings.pack_floats(embedding)
      filter_json = filter ? filter.to_json : nil

      ptr = FFIBindings.agentdb_hybrid_query(
        @handle,
        anchor_node.to_s,
        buf, size,
        collection.to_s,
        graph_depth.to_i,
        top_k.to_i,
        alpha.to_f,
        filter_json
      )
      json_string = read_json_ptr!(ptr, "agentdb_hybrid_query")
      JSON.parse(json_string)
    end

    # ── Conversations ─────────────────────────────────────────────────────

    # Create a new conversation record.
    #
    # @param id       [String]     unique conversation identifier
    # @param title    [String, nil] optional display title
    # @param metadata [Hash, nil]  optional JSON metadata
    # @raise [AgentDB::FFIError] on error
    def conversation_create(id, title: nil, metadata: nil)
      ensure_open!
      meta_json = metadata ? metadata.to_json : nil
      rc = FFIBindings.agentdb_conversation_create(
        @handle, id.to_s, title&.to_s, meta_json
      )
      check_rc!(rc, "agentdb_conversation_create")
    end

    # Append a message to an existing conversation.
    #
    # @param conversation_id [String]     target conversation
    # @param role            [String]     "user", "assistant", "system", etc.
    # @param content         [String]     message body
    # @param metadata        [Hash, nil]  optional JSON metadata
    # @return [String] the new message ID
    # @raise [AgentDB::FFIError] on error
    def conversation_add_message(conversation_id, role, content, metadata: nil)
      ensure_open!
      meta_json = metadata ? metadata.to_json : nil
      ptr = FFIBindings.agentdb_conversation_add_message(
        @handle, conversation_id.to_s, role.to_s, content.to_s, meta_json
      )
      read_string_ptr!(ptr, "agentdb_conversation_add_message")
    end

    # Retrieve messages for a conversation.
    #
    # @param conversation_id [String]       target conversation
    # @param limit           [Integer, nil] max messages (nil = all)
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on error
    def conversation_messages(conversation_id, limit: nil)
      ensure_open!
      ptr = FFIBindings.agentdb_conversation_get_messages(
        @handle, conversation_id.to_s, (limit || 0).to_i
      )
      json_string = read_json_ptr!(ptr, "agentdb_conversation_get_messages")
      JSON.parse(json_string)
    end

    # List all conversations.
    #
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on error
    def conversation_list
      ensure_open!
      ptr = FFIBindings.agentdb_conversation_list(@handle)
      json_string = read_json_ptr!(ptr, "agentdb_conversation_list")
      JSON.parse(json_string)
    end

    # Delete a conversation and all its messages.
    #
    # @param id [String] conversation identifier
    # @raise [AgentDB::FFIError] on error
    def conversation_delete(id)
      ensure_open!
      rc = FFIBindings.agentdb_conversation_delete(@handle, id.to_s)
      check_rc!(rc, "agentdb_conversation_delete")
    end

    # ── Workflows ─────────────────────────────────────────────────────────

    # Create a new workflow.
    #
    # @param id       [String]     unique workflow identifier
    # @param name     [String]     human-readable name
    # @param input    [Hash, nil]  optional JSON input
    # @param metadata [Hash, nil]  optional JSON metadata
    # @raise [AgentDB::FFIError] on error
    def workflow_create(id, name, input: nil, metadata: nil)
      ensure_open!
      rc = FFIBindings.agentdb_workflow_create(
        @handle,
        id.to_s, name.to_s,
        input ? input.to_json : nil,
        metadata ? metadata.to_json : nil
      )
      check_rc!(rc, "agentdb_workflow_create")
    end

    # Add a step to an existing workflow.
    #
    # @param workflow_id [String]    workflow identifier
    # @param name        [String]    step name
    # @param input       [Hash, nil] optional JSON input
    # @return [String] new step ID
    # @raise [AgentDB::FFIError] on error
    def workflow_add_step(workflow_id, name, input: nil)
      ensure_open!
      ptr = FFIBindings.agentdb_workflow_add_step(
        @handle, workflow_id.to_s, name.to_s, input ? input.to_json : nil
      )
      read_string_ptr!(ptr, "agentdb_workflow_add_step")
    end

    # Update a workflow step.
    #
    # @param step_id [String]     step identifier
    # @param status  [String]     "running", "completed", or "failed"
    # @param output  [Hash, nil]  optional JSON output
    # @param error   [String, nil] optional error message
    # @raise [AgentDB::FFIError] on error
    def workflow_update_step(step_id, status, output: nil, error: nil)
      ensure_open!
      rc = FFIBindings.agentdb_workflow_update_step(
        @handle,
        step_id.to_s, status.to_s,
        output ? output.to_json : nil,
        error&.to_s
      )
      check_rc!(rc, "agentdb_workflow_update_step")
    end

    # Mark a workflow as completed.
    #
    # @param id     [String]    workflow identifier
    # @param output [Hash, nil] optional JSON result
    # @raise [AgentDB::FFIError] on error
    def workflow_complete(id, output: nil)
      ensure_open!
      rc = FFIBindings.agentdb_workflow_complete(
        @handle, id.to_s, output ? output.to_json : nil
      )
      check_rc!(rc, "agentdb_workflow_complete")
    end

    # Mark a workflow as failed.
    #
    # @param id    [String]     workflow identifier
    # @param error [String, nil] optional error description
    # @raise [AgentDB::FFIError] on error
    def workflow_fail(id, error: nil)
      ensure_open!
      rc = FFIBindings.agentdb_workflow_fail(@handle, id.to_s, error&.to_s)
      check_rc!(rc, "agentdb_workflow_fail")
    end

    # Retrieve a workflow with all its steps.
    #
    # @param id [String] workflow identifier
    # @return [Hash]
    # @raise [AgentDB::FFIError] on error
    def workflow_get(id)
      ensure_open!
      ptr = FFIBindings.agentdb_workflow_get(@handle, id.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_workflow_get")
      JSON.parse(json_string)
    end

    # List workflows, optionally filtered by status.
    #
    # @param status [String, nil] "pending", "running", "completed", "failed", or nil for all
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on error
    def workflow_list(status: nil)
      ensure_open!
      ptr = FFIBindings.agentdb_workflow_list(@handle, status&.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_workflow_list")
      JSON.parse(json_string)
    end

    # ── Traces ────────────────────────────────────────────────────────────

    # Record a reasoning trace entry.
    #
    # @param trace_type  [String]      type label (e.g. "thought", "action")
    # @param content     [String]      trace body
    # @param session_id  [String, nil] optional session context
    # @param parent_id   [String, nil] optional parent trace ID for nesting
    # @param metadata    [Hash, nil]   optional JSON metadata
    # @return [String] new trace ID
    # @raise [AgentDB::FFIError] on error
    def trace_add(trace_type, content, session_id: nil, parent_id: nil, metadata: nil)
      ensure_open!
      ptr = FFIBindings.agentdb_trace_add(
        @handle,
        session_id&.to_s, parent_id&.to_s,
        trace_type.to_s, content.to_s,
        metadata ? metadata.to_json : nil
      )
      read_string_ptr!(ptr, "agentdb_trace_add")
    end

    # Retrieve all traces for a session.
    #
    # @param session_id [String] session identifier
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on error
    def trace_get_by_session(session_id)
      ensure_open!
      ptr = FFIBindings.agentdb_trace_get_by_session(@handle, session_id.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_trace_get_by_session")
      JSON.parse(json_string)
    end

    # Retrieve a trace subtree rooted at +root_id+.
    #
    # @param root_id [String] root trace ID
    # @return [Array<Hash>]
    # @raise [AgentDB::FFIError] on error
    def trace_get_tree(root_id)
      ensure_open!
      ptr = FFIBindings.agentdb_trace_get_tree(@handle, root_id.to_s)
      json_string = read_json_ptr!(ptr, "agentdb_trace_get_tree")
      JSON.parse(json_string)
    end

    private

    def ensure_open!
      raise AgentDB::DatabaseError, "Database is closed" if @closed
    end

    def check_rc!(rc, fn_name)
      return if rc >= 0

      msg = FFIBindings.last_error || "unknown error"
      raise AgentDB::FFIError, "#{fn_name} failed: #{msg}"
    end

    # Read a heap-allocated JSON string, free it, and raise if NULL.
    def read_json_ptr!(ptr, fn_name)
      if ptr.nil? || ptr.null?
        msg = FFIBindings.last_error || "unknown error"
        raise AgentDB::FFIError, "#{fn_name} failed: #{msg}"
      end
      FFIBindings.read_and_free(ptr)
    end

    # Read a heap-allocated plain string (not necessarily JSON), free it,
    # and raise if NULL.
    def read_string_ptr!(ptr, fn_name)
      if ptr.nil? || ptr.null?
        msg = FFIBindings.last_error || "unknown error"
        raise AgentDB::FFIError, "#{fn_name} failed: #{msg}"
      end
      FFIBindings.read_and_free(ptr)
    end
  end
end
