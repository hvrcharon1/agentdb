/// Demonstrates the v0.4.0 AI-native layers: Conversations, Workflows, and
/// Reasoning Traces — all in a single AgentDB file.
use agentdb::AgentDB;
use serde_json::json;

fn main() -> agentdb::Result<()> {
    let db = AgentDB::open(":memory:")?;

    println!("=== AgentDB — AI Agent Loop Demo ===\n");

    // ── 1. Conversations ──────────────────────────────────────────────────────
    println!("1. Conversation threading...");

    let convs = db.conversations();
    convs.create_conversation(
        "conv-001",
        Some("User onboarding"),
        Some(json!({"agent": "onboarding-bot", "version": "1.0"})),
    )?;

    convs.add_message(
        "conv-001",
        "system",
        "You are a helpful onboarding assistant.",
        None,
    )?;
    convs.add_message(
        "conv-001",
        "user",
        "How do I get started with AgentDB?",
        None,
    )?;
    convs.add_message(
        "conv-001",
        "assistant",
        "Install with `pip install datacules-agentdb` or `cargo add datacules-agentdb`.",
        Some(json!({"tokens": 18, "model": "gpt-4o"})),
    )?;
    convs.add_message(
        "conv-001",
        "user",
        "What storage layers does it have?",
        None,
    )?;
    convs.add_message(
        "conv-001",
        "assistant",
        "Eight layers: SQL, Vector Search, Memory Graph, FTS, Hybrid Queries, Conversations, Workflows, and Reasoning Traces.",
        None,
    )?;

    let messages = convs.get_messages("conv-001", None)?;
    println!(
        "   {} messages stored in conversation conv-001",
        messages.len()
    );
    for msg in &messages {
        println!(
            "   [{}] {}",
            msg.role,
            &msg.content[..msg.content.len().min(60)]
        );
    }

    let all_convs = convs.list_conversations()?;
    println!("   Total conversations: {}", all_convs.len());

    // ── 2. Workflows ──────────────────────────────────────────────────────────
    println!("\n2. Workflow persistence...");

    let wf = db.workflows();
    wf.create_workflow(
        "wf-rag-001",
        "RAG Pipeline",
        Some(json!({"query": "What is AgentDB?", "top_k": 5})),
    )?;

    let step1 = wf.add_step("wf-rag-001", "Embed query", None)?;
    let step2 = wf.add_step("wf-rag-001", "Vector search", None)?;
    let step3 = wf.add_step("wf-rag-001", "Generate answer", None)?;

    // Simulate step execution
    wf.update_step(&step1, "running", None, None)?;
    wf.update_step(
        &step1,
        "completed",
        Some(json!({"embedding_dim": 1536, "model": "text-embedding-3-small"})),
        None,
    )?;

    wf.update_step(&step2, "running", None, None)?;
    wf.update_step(
        &step2,
        "completed",
        Some(json!({"results": 5, "top_score": 0.94})),
        None,
    )?;

    wf.update_step(&step3, "running", None, None)?;
    wf.update_step(
        &step3,
        "completed",
        Some(json!({"answer": "AgentDB is a single-file embedded database for AI agents."})),
        None,
    )?;

    wf.complete_workflow(
        "wf-rag-001",
        Some(json!({"answer": "AgentDB is a single-file embedded database for AI agents."})),
    )?;

    let workflow = wf.get_workflow("wf-rag-001")?;
    println!(
        "   Workflow '{}' status: {}",
        workflow.name, workflow.status
    );
    for step in &workflow.steps {
        println!(
            "   Step {}: {} → {}",
            step.step_index, step.name, step.status
        );
    }

    let active = wf.list_workflows(Some("completed"))?;
    println!("   Completed workflows: {}", active.len());

    // ── 3. Reasoning Traces ───────────────────────────────────────────────────
    println!("\n3. Reasoning traces...");

    let traces = db.traces();

    let root = traces.add_trace(
        Some("session-abc"),
        None,
        "thought",
        "The user is asking about installation. I should check their platform first.",
        None,
    )?;

    let tool_call = traces.add_trace(
        Some("session-abc"),
        Some(&root),
        "tool_call",
        "detect_platform()",
        Some(json!({"tool": "detect_platform"})),
    )?;

    traces.add_trace(
        Some("session-abc"),
        Some(&tool_call),
        "observation",
        "Platform: macOS arm64",
        Some(json!({"platform": "darwin", "arch": "arm64"})),
    )?;

    traces.add_trace(
        Some("session-abc"),
        Some(&root),
        "thought",
        "macOS arm64 — recommend Homebrew or cargo install.",
        None,
    )?;

    let session_traces = traces.get_traces("session-abc")?;
    println!(
        "   {} traces recorded for session-abc",
        session_traces.len()
    );

    let tree = traces.get_trace_tree(&root)?;
    println!("   Trace tree from root ({} nodes):", tree.len());
    for t in &tree {
        let indent = "  ".repeat(if t.parent_id.is_some() { 2 } else { 1 });
        println!(
            "   {}[{}] {}",
            indent,
            t.trace_type,
            &t.content[..t.content.len().min(55)]
        );
    }

    // ── 4. Summary ────────────────────────────────────────────────────────────
    println!("\n4. Database stats:");
    let stats = db.stats()?;
    println!("   Collections: {}", stats.collections);
    println!("   Vectors:     {}", stats.vectors);
    println!("   Nodes:       {}", stats.nodes);
    println!("   Edges:       {}", stats.edges);

    println!("\n✓ All v0.4.0 AI-native layers demonstrated in one file.");
    Ok(())
}
