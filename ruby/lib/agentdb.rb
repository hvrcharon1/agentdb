# frozen_string_literal: true

# AgentDB — Ruby SDK
#
# A Ruby wrapper around the AgentDB C shared library via the +ffi+ gem.
# AgentDB is a single-file embedded database for AI agents providing:
#   * SQLite-backed relational storage (SQL API)
#   * HNSW vector search (embeddings)
#   * Full-text search (BM25/FTS5)
#   * Hybrid graph + vector queries
#   * Memory graph (nodes + weighted edges)
#   * Conversation history
#   * Workflow state machine
#   * Reasoning traces
#
# Quick start:
#   require 'agentdb'
#
#   AgentDB::Database.open("agent.agentdb") do |db|
#     col = db.collection("memories", 1536)
#     col.upsert("mem1", embedding, { topic: "ruby" })
#     results = col.search(query_embedding, top_k: 5)
#   end

require_relative "agentdb/version"
require_relative "agentdb/errors"
require_relative "agentdb/ffi_bindings"
require_relative "agentdb/collection"
require_relative "agentdb/database"
