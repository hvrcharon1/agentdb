use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A workflow and its current state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Workflow {
    /// Unique identifier for this workflow.
    pub id: String,
    /// Human-readable name for the workflow.
    pub name: String,
    /// Current status (`"pending"`, `"running"`, `"completed"`, `"failed"`).
    pub status: String,
    /// JSON input payload passed to the workflow.
    pub input: Option<Value>,
    /// JSON output payload produced by the workflow.
    pub output: Option<Value>,
    /// Error message if the workflow failed.
    pub error: Option<String>,
    /// Arbitrary JSON metadata.
    pub metadata: Option<Value>,
    /// Unix-millisecond timestamp when the workflow was created.
    pub created_at: i64,
    /// Unix-millisecond timestamp of the most recent status change.
    pub updated_at: i64,
    /// Number of steps in this workflow (populated by `list_workflows`; full
    /// step objects are only fetched by `get_workflow`).
    pub step_count: i64,
    /// Steps belonging to this workflow, ordered by `step_index`.
    pub steps: Vec<WorkflowStep>,
}

/// A single step within a workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowStep {
    /// Unique identifier for this step.
    pub id: String,
    /// ID of the parent workflow.
    pub workflow_id: String,
    /// Zero-based position of the step within the workflow.
    pub step_index: i64,
    /// Human-readable name for the step.
    pub name: String,
    /// Current status (`"pending"`, `"running"`, `"completed"`, `"failed"`).
    pub status: String,
    /// JSON input payload for this step.
    pub input: Option<Value>,
    /// JSON output payload produced by this step.
    pub output: Option<Value>,
    /// Error message if the step failed.
    pub error: Option<String>,
    /// Unix-millisecond timestamp when execution of this step began.
    pub started_at: Option<i64>,
    /// Unix-millisecond timestamp when execution of this step finished.
    pub completed_at: Option<i64>,
}

/// Manages workflow lifecycle and step tracking.
pub struct WorkflowStore {
    conn: Arc<Mutex<Connection>>,
}

impl WorkflowStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Create a new workflow in `pending` status.
    pub fn create_workflow(
        &self,
        id: &str,
        name: &str,
        input: Option<Value>,
        metadata: Option<Value>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let input_str = input.as_ref().map(|v| v.to_string());
        let meta_str = metadata.as_ref().map(|v| v.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_workflows
                 (id, name, status, input, metadata, created_at, updated_at)
             VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6)",
            params![id, name, input_str, meta_str, now, now],
        )?;
        Ok(())
    }

    /// Append a step to an existing workflow.
    ///
    /// The `step_index` is assigned automatically as one more than the current
    /// maximum index in the workflow (0-based). Returns the new step ID.
    pub fn add_step(&self, workflow_id: &str, name: &str, input: Option<Value>) -> Result<String> {
        let step_id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let input_str = input.as_ref().map(|v| v.to_string());
        let step_index: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(step_index) + 1, 0)
                 FROM _adb_workflow_steps
                 WHERE workflow_id = ?1",
                params![workflow_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO _adb_workflow_steps
                 (id, workflow_id, step_index, name, status, input)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
            params![step_id, workflow_id, step_index, name, input_str],
        )?;
        Ok(step_id)
    }

    /// Update the status, output, and/or error of a workflow step.
    ///
    /// When `status` is `"running"` the `started_at` timestamp is recorded.
    /// When `status` is `"completed"` or `"failed"` the `completed_at`
    /// timestamp is recorded.
    pub fn update_step(
        &self,
        step_id: &str,
        status: &str,
        output: Option<Value>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let output_str = output.as_ref().map(|v| v.to_string());
        let now = now_ms();
        let started_at: Option<i64> = if status == "running" { Some(now) } else { None };
        let completed_at: Option<i64> = if status == "completed" || status == "failed" {
            Some(now)
        } else {
            None
        };
        let changed = conn.execute(
            "UPDATE _adb_workflow_steps
             SET status       = ?2,
                 output       = COALESCE(?3, output),
                 error        = COALESCE(?4, error),
                 started_at   = COALESCE(?5, started_at),
                 completed_at = COALESCE(?6, completed_at)
             WHERE id = ?1",
            params![step_id, status, output_str, error, started_at, completed_at],
        )?;
        if changed == 0 {
            return Err(AgentDbError::InvalidArgument(format!(
                "step not found: {step_id}"
            )));
        }
        Ok(())
    }

    /// Mark a workflow as `completed` and record its output.
    pub fn complete_workflow(&self, id: &str, output: Option<Value>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let output_str = output.as_ref().map(|v| v.to_string());
        let now = now_ms();
        let changed = conn.execute(
            "UPDATE _adb_workflows
             SET status = 'completed', output = ?2, updated_at = ?3
             WHERE id = ?1",
            params![id, output_str, now],
        )?;
        if changed == 0 {
            return Err(AgentDbError::InvalidArgument(format!(
                "workflow not found: {id}"
            )));
        }
        Ok(())
    }

    /// Mark a workflow as `failed` and record an optional error message.
    pub fn fail_workflow(&self, id: &str, error: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = now_ms();
        let changed = conn.execute(
            "UPDATE _adb_workflows
             SET status = 'failed', error = COALESCE(?2, error), updated_at = ?3
             WHERE id = ?1",
            params![id, error, now],
        )?;
        if changed == 0 {
            return Err(AgentDbError::InvalidArgument(format!(
                "workflow not found: {id}"
            )));
        }
        Ok(())
    }

    /// Retrieve a workflow and all of its steps.
    pub fn get_workflow(&self, id: &str) -> Result<Workflow> {
        let workflow = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT id, name, status, input, output, error, metadata, created_at, updated_at
                 FROM _adb_workflows
                 WHERE id = ?1",
                params![id],
                parse_workflow_row,
            )
            .map_err(|_| AgentDbError::InvalidArgument(format!("workflow not found: {id}")))?
        };
        let steps = self.steps_for_workflow(id)?;
        let step_count = steps.len() as i64;
        Ok(Workflow { steps, step_count, ..workflow })
    }

    /// List workflows, optionally filtered by status.
    ///
    /// Pass `None` to return all workflows. Results are ordered by `created_at`
    /// descending (most recent first). The `step_count` field is populated.
    /// Full step objects are only fetched by [`get_workflow`].
    pub fn list_workflows(&self, status_filter: Option<&str>) -> Result<Vec<Workflow>> {
        let conn = self.conn.lock().unwrap();
        let workflows: Vec<Workflow> = match status_filter {
            Some(s) => {
                let mut stmt = conn.prepare(
                    "SELECT w.id, w.name, w.status, w.input, w.output, w.error,
                            w.metadata, w.created_at, w.updated_at,
                            COUNT(s.id) AS step_count
                     FROM _adb_workflows w
                     LEFT JOIN _adb_workflow_steps s ON s.workflow_id = w.id
                     WHERE w.status = ?1
                     GROUP BY w.id
                     ORDER BY w.created_at DESC",
                )?;
                let rows = stmt.query_map(params![s], parse_workflow_row_with_count)?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT w.id, w.name, w.status, w.input, w.output, w.error,
                            w.metadata, w.created_at, w.updated_at,
                            COUNT(s.id) AS step_count
                     FROM _adb_workflows w
                     LEFT JOIN _adb_workflow_steps s ON s.workflow_id = w.id
                     GROUP BY w.id
                     ORDER BY w.created_at DESC",
                )?;
                let rows = stmt.query_map([], parse_workflow_row_with_count)?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
        };
        Ok(workflows)
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn steps_for_workflow(&self, workflow_id: &str) -> Result<Vec<WorkflowStep>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, step_index, name, status,
                    input, output, error, started_at, completed_at
             FROM _adb_workflow_steps
             WHERE workflow_id = ?1
             ORDER BY step_index ASC",
        )?;
        let rows = stmt.query_map(params![workflow_id], parse_step_row)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }
}

// ── Row parsers ──────────────────────────────────────────────────────────────

// Used by get_workflow (no step_count column in query)
fn parse_workflow_row(row: &rusqlite::Row) -> rusqlite::Result<Workflow> {
    // Column order: id(0) name(1) status(2) input(3) output(4) error(5) metadata(6) created_at(7) updated_at(8)
    let input_str: Option<String> = row.get(3)?;
    let output_str: Option<String> = row.get(4)?;
    let meta_str: Option<String> = row.get(6)?;
    Ok(Workflow {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        input: input_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        output: output_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        error: row.get(5)?,
        metadata: meta_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        step_count: 0,
        steps: vec![],
    })
}

// Used by list_workflows (includes step_count at column 9)
fn parse_workflow_row_with_count(row: &rusqlite::Row) -> rusqlite::Result<Workflow> {
    let input_str: Option<String> = row.get(3)?;
    let output_str: Option<String> = row.get(4)?;
    let meta_str: Option<String> = row.get(6)?;
    Ok(Workflow {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        input: input_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        output: output_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        error: row.get(5)?,
        metadata: meta_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        step_count: row.get(9)?,
        steps: vec![],
    })
}

fn parse_step_row(row: &rusqlite::Row) -> rusqlite::Result<WorkflowStep> {
    let input_str: Option<String> = row.get(5)?;
    let output_str: Option<String> = row.get(6)?;
    Ok(WorkflowStep {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        step_index: row.get(2)?,
        name: row.get(3)?,
        status: row.get(4)?,
        input: input_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        output: output_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        error: row.get(7)?,
        started_at: row.get(8)?,
        completed_at: row.get(9)?,
    })
}
