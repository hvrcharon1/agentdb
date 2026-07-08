# frozen_string_literal: true

require "json"
require_relative "ffi_bindings"
require_relative "errors"

module AgentDB
  # A vector collection inside an AgentDB database.
  #
  # Obtain an instance via AgentDB::Database#collection — do not construct
  # directly.
  #
  # Example:
  #   col = db.collection("memories", 1536)
  #   col.upsert("mem1", [0.1, 0.2, ...], { topic: "ruby" })
  #   results = col.search([0.1, 0.2, ...], top_k: 5)
  class Collection
    # @return [String] collection name
    attr_reader :name

    # @return [Integer] vector dimensionality
    attr_reader :dim

    # @param handle [FFI::Pointer] opaque AgentDbHandle pointer
    # @param name   [String]       collection name
    # @param dim    [Integer]      vector dimensionality
    def initialize(handle, name, dim)
      @handle = handle
      @name   = name.to_s
      @dim    = dim.to_i
    end

    # Insert or update a vector entry.
    #
    # @param id       [String]       unique identifier for this vector
    # @param vector   [Array<Float>] embedding values (length must equal dim)
    # @param metadata [Hash, nil]    arbitrary JSON-serialisable metadata
    # @raise [AgentDB::Error] on failure
    def upsert(id, vector, metadata = nil)
      raise ArgumentError, "vector length #{vector.size} != dim #{@dim}" if vector.size != @dim

      buf, size = FFIBindings.pack_floats(vector)
      meta_json = metadata ? metadata.to_json : nil

      rc = FFIBindings.agentdb_vector_upsert(
        @handle, @name, id.to_s, buf, size, meta_json
      )
      check_rc!(rc, "agentdb_vector_upsert")
      self
    end

    # Search the collection by approximate nearest-neighbour.
    #
    # @param query  [Array<Float>]  query embedding (length must equal dim)
    # @param top_k  [Integer]       number of results to return (default: 10)
    # @param filter [Hash, nil]     MongoDB-style metadata filter, e.g.
    #                               { "topic" => { "$eq" => "ruby" } }
    # @return [Array<Hash>] array of { "id", "score", "metadata" } hashes
    # @raise [AgentDB::Error] on failure
    def search(query, top_k: 10, filter: nil)
      raise ArgumentError, "query length #{query.size} != dim #{@dim}" if query.size != @dim

      buf, size = FFIBindings.pack_floats(query)
      filter_json = filter ? filter.to_json : nil

      ptr = FFIBindings.agentdb_vector_search(
        @handle, @name, buf, size, top_k.to_i, filter_json
      )
      json_string = read_json_ptr!(ptr, "agentdb_vector_search")
      JSON.parse(json_string)
    end

    # Delete a single vector by ID.
    #
    # @param id [String] vector identifier to remove
    # @raise [AgentDB::Error] on failure
    def delete(id)
      rc = FFIBindings.agentdb_vector_delete(@handle, @name, id.to_s, @dim)
      check_rc!(rc, "agentdb_vector_delete")
      self
    end

    # Drop the entire collection and all its vectors.
    #
    # @raise [AgentDB::Error] on failure
    def drop
      rc = FFIBindings.agentdb_drop_collection(@handle, @name)
      check_rc!(rc, "agentdb_drop_collection")
      nil
    end

    # Rebuild the HNSW index for this collection.
    #
    # Useful after bulk inserts to improve query performance.
    # @raise [AgentDB::Error] on failure
    def reindex
      rc = FFIBindings.agentdb_reindex(@handle, @name, @dim)
      check_rc!(rc, "agentdb_reindex")
      self
    end

    # Index a text document in the full-text search engine, linking it to
    # a vector entry so hybrid queries can combine both signals.
    #
    # @param vec_id        [String] corresponding vector entry ID
    # @param collection_id [String] collection-scoped document ID
    # @param text          [String] document body to index
    # @raise [AgentDB::Error] on failure
    def fts_index(vec_id, collection_id, text)
      rc = FFIBindings.agentdb_fts_index(
        @handle, @name, vec_id.to_s, collection_id.to_s, text.to_s
      )
      check_rc!(rc, "agentdb_fts_index")
      self
    end

    # Full-text search over the collection.
    #
    # @param query [String]  search terms
    # @param top_k [Integer] max results (default: 10)
    # @return [Array<Hash>]  array of { "id", "snippet", "rank" } hashes
    # @raise [AgentDB::Error] on failure
    def fts_search(query, top_k: 10)
      ptr = FFIBindings.agentdb_fts_search(@handle, @name, query.to_s, top_k.to_i)
      json_string = read_json_ptr!(ptr, "agentdb_fts_search")
      JSON.parse(json_string)
    end

    # Delete a document from the FTS index.
    #
    # @param vec_id [String] vector entry ID whose text should be removed
    # @raise [AgentDB::Error] on failure
    def fts_delete(vec_id)
      rc = FFIBindings.agentdb_fts_delete(@handle, @name, vec_id.to_s)
      check_rc!(rc, "agentdb_fts_delete")
      self
    end

    # Optimize (merge) FTS index segments for faster queries.
    #
    # @raise [AgentDB::Error] on failure
    def fts_optimize
      rc = FFIBindings.agentdb_fts_optimize(@handle, @name)
      check_rc!(rc, "agentdb_fts_optimize")
      self
    end

    private

    def check_rc!(rc, fn_name)
      return if rc >= 0

      msg = FFIBindings.last_error || "unknown error"
      raise AgentDB::FFIError, "#{fn_name} failed: #{msg}"
    end

    def read_json_ptr!(ptr, fn_name)
      if ptr.nil? || ptr.null?
        msg = FFIBindings.last_error || "unknown error"
        raise AgentDB::FFIError, "#{fn_name} failed: #{msg}"
      end
      FFIBindings.read_and_free(ptr)
    end
  end
end
