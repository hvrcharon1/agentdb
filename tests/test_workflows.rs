#[cfg(test)]
mod tests {
    use agentdb::AgentDB;
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    // ── create_workflow ───────────────────────────────────────────────────────

    #[test]
    fn test_create_workflow_pending_status() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "My Pipeline", None, None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.id, "wf-1");
        assert_eq!(workflow.name, "My Pipeline");
        assert_eq!(workflow.status, "pending");
        assert!(workflow.output.is_none());
    }

    #[test]
    fn test_create_workflow_with_input() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Ingest", Some(json!({ "file": "data.csv" })), None)
            .unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.input.as_ref().unwrap()["file"], "data.csv");
    }

    // ── list_workflows ────────────────────────────────────────────────────────

    #[test]
    fn test_list_workflows_empty() {
        let db = open();
        let list = db.workflows().list_workflows(None).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_workflows_all() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Alpha", None, None).unwrap();
        wf.create_workflow("wf-2", "Beta", None, None).unwrap();
        let list = wf.list_workflows(None).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_list_workflows_status_filter() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Alpha", None, None).unwrap();
        wf.create_workflow("wf-2", "Beta", None, None).unwrap();
        // Complete one workflow so we have a mix of statuses.
        wf.complete_workflow("wf-1", Some(json!({ "result": "ok" })))
            .unwrap();
        let pending = wf.list_workflows(Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "wf-2");
        let completed = wf.list_workflows(Some("completed")).unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "wf-1");
    }

    // ── add_step ──────────────────────────────────────────────────────────────

    #[test]
    fn test_add_step_returns_id() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        let step_id = wf.add_step("wf-1", "Fetch", None).unwrap();
        assert!(!step_id.is_empty());
    }

    #[test]
    fn test_add_multiple_steps_indexed_sequentially() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.add_step("wf-1", "Step A", None).unwrap();
        wf.add_step("wf-1", "Step B", None).unwrap();
        wf.add_step("wf-1", "Step C", None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.steps.len(), 3);
        assert_eq!(workflow.steps[0].step_index, 0);
        assert_eq!(workflow.steps[1].step_index, 1);
        assert_eq!(workflow.steps[2].step_index, 2);
        assert_eq!(workflow.steps[0].name, "Step A");
        assert_eq!(workflow.steps[2].name, "Step C");
    }

    #[test]
    fn test_add_step_initial_status_is_pending() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.add_step("wf-1", "Fetch", None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.steps[0].status, "pending");
        assert!(workflow.steps[0].started_at.is_none());
        assert!(workflow.steps[0].completed_at.is_none());
    }

    #[test]
    fn test_add_step_with_input() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.add_step("wf-1", "Transform", Some(json!({ "format": "json" })))
            .unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.steps[0].input.as_ref().unwrap()["format"], "json");
    }

    // ── update_step ───────────────────────────────────────────────────────────

    #[test]
    fn test_update_step_to_running_sets_started_at() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        let step_id = wf.add_step("wf-1", "Run", None).unwrap();
        wf.update_step(&step_id, "running", None, None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        let step = &workflow.steps[0];
        assert_eq!(step.status, "running");
        assert!(step.started_at.is_some());
        assert!(step.completed_at.is_none());
    }

    #[test]
    fn test_update_step_to_completed_sets_completed_at() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        let step_id = wf.add_step("wf-1", "Run", None).unwrap();
        wf.update_step(&step_id, "running", None, None).unwrap();
        wf.update_step(&step_id, "completed", Some(json!({ "rows": 42 })), None)
            .unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        let step = &workflow.steps[0];
        assert_eq!(step.status, "completed");
        assert!(step.completed_at.is_some());
        assert_eq!(step.output.as_ref().unwrap()["rows"], 42);
    }

    #[test]
    fn test_update_step_to_failed_records_error() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        let step_id = wf.add_step("wf-1", "Run", None).unwrap();
        wf.update_step(&step_id, "failed", None, Some("timeout"))
            .unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        let step = &workflow.steps[0];
        assert_eq!(step.status, "failed");
        assert_eq!(step.error.as_deref(), Some("timeout"));
        assert!(step.completed_at.is_some());
    }

    #[test]
    fn test_update_step_nonexistent_errors() {
        let db = open();
        let result = db
            .workflows()
            .update_step("no-such-step", "running", None, None);
        assert!(result.is_err());
    }

    // ── complete_workflow ─────────────────────────────────────────────────────

    #[test]
    fn test_complete_workflow_sets_status_and_output() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.complete_workflow("wf-1", Some(json!({ "summary": "done" })))
            .unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.status, "completed");
        assert_eq!(workflow.output.as_ref().unwrap()["summary"], "done");
    }

    #[test]
    fn test_complete_workflow_without_output() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.complete_workflow("wf-1", None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.status, "completed");
        assert!(workflow.output.is_none());
    }

    #[test]
    fn test_complete_nonexistent_workflow_errors() {
        let db = open();
        let result = db.workflows().complete_workflow("ghost", None);
        assert!(result.is_err());
    }

    // ── get_workflow ──────────────────────────────────────────────────────────

    #[test]
    fn test_get_workflow_includes_steps() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.add_step("wf-1", "Alpha", None).unwrap();
        wf.add_step("wf-1", "Beta", None).unwrap();
        let workflow = wf.get_workflow("wf-1").unwrap();
        assert_eq!(workflow.steps.len(), 2);
    }

    #[test]
    fn test_get_nonexistent_workflow_errors() {
        let db = open();
        let result = db.workflows().get_workflow("ghost");
        assert!(result.is_err());
    }

    // ── list_workflows skips steps ────────────────────────────────────────────

    #[test]
    fn test_list_workflows_steps_not_populated() {
        let db = open();
        let wf = db.workflows();
        wf.create_workflow("wf-1", "Pipeline", None, None).unwrap();
        wf.add_step("wf-1", "Step", None).unwrap();
        // list_workflows is documented to NOT populate steps for performance.
        let list = wf.list_workflows(None).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].steps.is_empty());
    }

    // ── full lifecycle ────────────────────────────────────────────────────────

    #[test]
    fn test_full_workflow_lifecycle() {
        let db = open();
        let wf = db.workflows();

        // Create
        wf.create_workflow("wf-run", "E2E Test", Some(json!({ "input": 1 })), None)
            .unwrap();

        // Add steps
        let s1 = wf.add_step("wf-run", "Fetch", None).unwrap();
        let s2 = wf.add_step("wf-run", "Process", None).unwrap();

        // Run step 1
        wf.update_step(&s1, "running", None, None).unwrap();
        wf.update_step(&s1, "completed", Some(json!({ "rows": 10 })), None)
            .unwrap();

        // Run step 2
        wf.update_step(&s2, "running", None, None).unwrap();
        wf.update_step(&s2, "completed", Some(json!({ "rows": 10 })), None)
            .unwrap();

        // Complete the workflow
        wf.complete_workflow("wf-run", Some(json!({ "total": 10 })))
            .unwrap();

        // Verify final state
        let workflow = wf.get_workflow("wf-run").unwrap();
        assert_eq!(workflow.status, "completed");
        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.steps[0].status, "completed");
        assert_eq!(workflow.steps[1].status, "completed");
        assert!(workflow.steps[0].started_at.is_some());
        assert!(workflow.steps[0].completed_at.is_some());
    }
}
