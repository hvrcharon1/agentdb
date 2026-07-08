# frozen_string_literal: true

require "spec_helper"

# NOTE: These specs require the compiled shared library to actually run.
# Build it first:
#   cargo build --release --features ffi --lib
# Then run:
#   bundle exec rspec

RSpec.describe AgentDB do
  it "exports VERSION" do
    expect(AgentDB::VERSION).to match(/\A\d+\.\d+\.\d+\z/)
  end
end

RSpec.describe AgentDB::Database do
  # ── Lifecycle ────────────────────────────────────────────────────────────

  describe ".new" do
    it "opens an in-memory database without raising" do
      db = AgentDB::Database.new(":memory:")
      expect(db).to be_a(AgentDB::Database)
      db.close
    end

    it "raises DatabaseError when the path is invalid" do
      expect {
        AgentDB::Database.new("/nonexistent/path/that/cannot/be/created.db")
      }.to raise_error(AgentDB::DatabaseError)
    end
  end

  describe ".open (block form)" do
    it "closes the database after the block" do
      db_ref = nil
      AgentDB::Database.open(":memory:") do |db|
        db_ref = db
        expect(db.closed?).to be false
      end
      expect(db_ref.closed?).to be true
    end

    it "closes the database even when the block raises" do
      db_ref = nil
      expect {
        AgentDB::Database.open(":memory:") do |db|
          db_ref = db
          raise "intentional"
        end
      }.to raise_error("intentional")
      expect(db_ref.closed?).to be true
    end
  end

  describe "#close" do
    it "is idempotent" do
      db = AgentDB::Database.new(":memory:")
      db.close
      expect { db.close }.not_to raise_error
    end
  end

  # ── SQL ──────────────────────────────────────────────────────────────────

  describe "#execute / #query" do
    it "creates a table and inserts rows" do
      with_db do |db|
        db.execute("CREATE TABLE notes (id TEXT PRIMARY KEY, body TEXT NOT NULL)")
        rc = db.execute("INSERT INTO notes VALUES ('n1', 'hello world')")
        expect(rc).to eq(1)

        rows = db.query("SELECT * FROM notes")
        expect(rows.length).to eq(1)
        expect(rows.first["id"]).to eq("n1")
        expect(rows.first["body"]).to eq("hello world")
      end
    end

    it "returns an empty array for zero rows" do
      with_db do |db|
        db.execute("CREATE TABLE empty (x TEXT)")
        expect(db.query("SELECT * FROM empty")).to eq([])
      end
    end

    it "raises FFIError on bad SQL" do
      with_db do |db|
        expect {
          db.execute("THIS IS NOT VALID SQL !!!")
        }.to raise_error(AgentDB::FFIError)
      end
    end
  end

  describe "#query_params" do
    it "binds positional parameters" do
      with_db do |db|
        db.execute("CREATE TABLE items (name TEXT, score INTEGER)")
        db.execute("INSERT INTO items VALUES ('alpha', 10)")
        db.execute("INSERT INTO items VALUES ('beta',  20)")

        rows = db.query_params("SELECT * FROM items WHERE score > ?", [15])
        expect(rows.length).to eq(1)
        expect(rows.first["name"]).to eq("beta")
      end
    end
  end

  # ── Stats ─────────────────────────────────────────────────────────────────

  describe "#stats" do
    it "returns a Hash with expected keys" do
      with_db do |db|
        s = db.stats
        %w[collections vectors nodes edges conversations messages
           workflows workflow_steps traces tools tool_calls
           audit_entries prompt_templates].each do |key|
          expect(s).to have_key(key)
        end
      end
    end
  end

  # ── Vector collections ───────────────────────────────────────────────────

  describe "#collection" do
    it "returns a Collection object" do
      with_db do |db|
        col = db.collection("test", 4)
        expect(col).to be_a(AgentDB::Collection)
        expect(col.name).to eq("test")
        expect(col.dim).to eq(4)
      end
    end
  end

  # ── Memory graph ──────────────────────────────────────────────────────────

  describe "graph operations" do
    it "adds nodes and edges and traverses neighbours" do
      with_db do |db|
        db.graph_add_node("A", "concept", { label: "Alpha" })
        db.graph_add_node("B", "concept", { label: "Beta" })
        db.graph_add_edge("A", "B", "relates_to", 0.9)

        neighbours = db.graph_neighbors("A", max_depth: 1)
        ids = neighbours.map { |n| n["id"] }
        expect(ids).to include("B")
      end
    end

    it "fetches a single node" do
      with_db do |db|
        db.graph_add_node("X", "session")
        node = db.graph_get_node("X")
        expect(node["id"]).to eq("X")
        expect(node["kind"]).to eq("session")
      end
    end

    it "deletes a node" do
      with_db do |db|
        db.graph_add_node("Y", "temp")
        expect { db.graph_delete_node("Y") }.not_to raise_error
      end
    end

    it "deletes an edge" do
      with_db do |db|
        db.graph_add_node("P", "concept")
        db.graph_add_node("Q", "concept")
        db.graph_add_edge("P", "Q", "linked", 1.0)
        expect { db.graph_delete_edge("P", "Q", "linked") }.not_to raise_error
      end
    end
  end

  # ── Conversations ─────────────────────────────────────────────────────────

  describe "conversation operations" do
    it "creates a conversation and adds messages" do
      with_db do |db|
        db.conversation_create("conv-1", title: "Test chat")
        msg_id = db.conversation_add_message("conv-1", "user", "Hello!")
        expect(msg_id).to be_a(String)
        expect(msg_id).not_to be_empty

        messages = db.conversation_messages("conv-1")
        expect(messages.length).to eq(1)
        expect(messages.first["role"]).to eq("user")
        expect(messages.first["content"]).to eq("Hello!")
      end
    end

    it "lists and deletes conversations" do
      with_db do |db|
        db.conversation_create("conv-a")
        db.conversation_create("conv-b")

        list = db.conversation_list
        ids = list.map { |c| c["id"] }
        expect(ids).to include("conv-a", "conv-b")

        db.conversation_delete("conv-a")
        list_after = db.conversation_list.map { |c| c["id"] }
        expect(list_after).not_to include("conv-a")
        expect(list_after).to include("conv-b")
      end
    end
  end

  # ── Workflows ─────────────────────────────────────────────────────────────

  describe "workflow operations" do
    it "creates a workflow, adds and updates steps, then completes it" do
      with_db do |db|
        db.workflow_create("wf-1", "Test Workflow", input: { task: "demo" })
        step_id = db.workflow_add_step("wf-1", "step-one", input: { x: 1 })
        expect(step_id).to be_a(String)

        db.workflow_update_step(step_id, "running")
        db.workflow_update_step(step_id, "completed", output: { result: 42 })
        db.workflow_complete("wf-1", output: { summary: "done" })

        wf = db.workflow_get("wf-1")
        expect(wf["status"]).to eq("completed")
        expect(wf["steps"].length).to eq(1)
      end
    end

    it "marks a workflow as failed" do
      with_db do |db|
        db.workflow_create("wf-fail", "Failing Workflow")
        db.workflow_fail("wf-fail", error: "Something went wrong")

        wf = db.workflow_get("wf-fail")
        expect(wf["status"]).to eq("failed")
      end
    end

    it "lists workflows with status filter" do
      with_db do |db|
        db.workflow_create("wf-a", "A")
        db.workflow_create("wf-b", "B")
        db.workflow_complete("wf-b")

        pending_list = db.workflow_list(status: "pending")
        expect(pending_list.map { |w| w["id"] }).to include("wf-a")

        completed_list = db.workflow_list(status: "completed")
        expect(completed_list.map { |w| w["id"] }).to include("wf-b")
      end
    end
  end

  # ── Traces ────────────────────────────────────────────────────────────────

  describe "trace operations" do
    it "records traces and retrieves them by session" do
      with_db do |db|
        trace_id = db.trace_add("thought", "I should search for X",
                                session_id: "sess-1")
        expect(trace_id).to be_a(String)

        traces = db.trace_get_by_session("sess-1")
        expect(traces.length).to eq(1)
        expect(traces.first["content"]).to eq("I should search for X")
      end
    end

    it "retrieves a trace subtree" do
      with_db do |db|
        root_id = db.trace_add("thought", "root thought", session_id: "sess-2")
        db.trace_add("action", "child action",
                     session_id: "sess-2", parent_id: root_id)

        tree = db.trace_get_tree(root_id)
        expect(tree).to be_an(Array)
      end
    end
  end

  # ── Closed database guard ─────────────────────────────────────────────────

  describe "closed database guard" do
    it "raises DatabaseError when calling methods on a closed db" do
      db = AgentDB::Database.new(":memory:")
      db.close
      expect { db.execute("SELECT 1") }.to raise_error(AgentDB::DatabaseError, /closed/)
      expect { db.query("SELECT 1") }.to raise_error(AgentDB::DatabaseError, /closed/)
      expect { db.stats }.to raise_error(AgentDB::DatabaseError, /closed/)
      expect { db.collection("x", 4) }.to raise_error(AgentDB::DatabaseError, /closed/)
    end
  end
end

RSpec.describe AgentDB::Collection do
  # ── Vector upsert / search ────────────────────────────────────────────────

  describe "#upsert and #search" do
    it "inserts a vector and retrieves it by similarity" do
      with_db do |db|
        col = db.collection("embeddings", 4)
        vec = [0.1, 0.2, 0.3, 0.4]
        col.upsert("v1", vec, { category: "test" })

        results = col.search(vec, top_k: 1)
        expect(results).to be_an(Array)
        expect(results.length).to eq(1)
        expect(results.first["id"]).to eq("v1")
        # Cosine similarity to itself should be ~1.0
        expect(results.first["score"]).to be_within(0.01).of(1.0)
      end
    end

    it "returns up to top_k results" do
      with_db do |db|
        col = db.collection("multi", 4)
        5.times { |i| col.upsert("v#{i}", random_embedding(4)) }

        results = col.search(random_embedding(4), top_k: 3)
        expect(results.length).to be <= 3
      end
    end

    it "raises ArgumentError when vector length != dim" do
      with_db do |db|
        col = db.collection("strict", 4)
        expect {
          col.upsert("bad", [1.0, 2.0])  # length 2, not 4
        }.to raise_error(ArgumentError, /dim/)
      end
    end
  end

  # ── Delete ────────────────────────────────────────────────────────────────

  describe "#delete" do
    it "removes a vector so it no longer appears in results" do
      with_db do |db|
        col = db.collection("del_test", 4)
        vec = [1.0, 0.0, 0.0, 0.0]
        col.upsert("target", vec)
        col.delete("target")

        results = col.search(vec, top_k: 10)
        ids = results.map { |r| r["id"] }
        expect(ids).not_to include("target")
      end
    end
  end

  # ── Drop ──────────────────────────────────────────────────────────────────

  describe "#drop" do
    it "drops the entire collection without raising" do
      with_db do |db|
        col = db.collection("droppable", 4)
        col.upsert("v1", [1.0, 0.0, 0.0, 0.0])
        expect { col.drop }.not_to raise_error
      end
    end
  end

  # ── Reindex ───────────────────────────────────────────────────────────────

  describe "#reindex" do
    it "rebuilds the index without raising" do
      with_db do |db|
        col = db.collection("reindexable", 4)
        3.times { |i| col.upsert("v#{i}", random_embedding(4)) }
        expect { col.reindex }.not_to raise_error
      end
    end
  end

  # ── FTS ───────────────────────────────────────────────────────────────────

  describe "full-text search" do
    it "indexes and searches text documents" do
      with_db do |db|
        col = db.collection("fts_col", 4)
        col.upsert("doc1", random_embedding(4))
        col.fts_index("doc1", "cid1", "The quick brown fox jumps over the lazy dog")

        results = col.fts_search("quick fox", top_k: 5)
        expect(results).to be_an(Array)
        expect(results.length).to be >= 1
        expect(results.first["id"]).to eq("doc1")
      end
    end

    it "deletes text from the index" do
      with_db do |db|
        col = db.collection("fts_del", 4)
        col.upsert("doc2", random_embedding(4))
        col.fts_index("doc2", "cid2", "hello world ruby")
        expect { col.fts_delete("doc2") }.not_to raise_error
      end
    end

    it "optimizes the FTS index" do
      with_db do |db|
        col = db.collection("fts_opt", 4)
        expect { col.fts_optimize }.not_to raise_error
      end
    end
  end
end
