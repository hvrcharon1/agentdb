use rusqlite::params;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub data: Option<Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub src: String,
    pub dst: String,
    pub relation: String,
    pub weight: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TraversalOptions {
    pub relation: Option<String>,
    pub max_depth: usize,
    pub min_weight: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TraversalResult {
    pub node: Node,
    pub depth: usize,
    pub weight: f64,
}

pub struct MemoryGraph {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryGraph {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Add or update a node
    pub fn add_node(&self, id: &str, kind: &str, data: Option<Value>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let data_str = data.as_ref().map(|d| d.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_nodes (id, kind, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind,
               data = excluded.data,
               updated_at = excluded.updated_at",
            params![id, kind, data_str, now, now],
        )?;
        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Result<Node> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, kind, data, created_at, updated_at FROM _adb_nodes WHERE id = ?1",
            params![id],
            |row| {
                let data_str: Option<String> = row.get(2)?;
                Ok(Node {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    data: data_str.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| AgentDbError::NodeNotFound(id.to_string()))
    }

    /// Delete a node and all its edges
    pub fn delete_node(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM _adb_nodes WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Add or update an edge between nodes
    pub fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> Result<()> {
        // Ensure both nodes exist
        self.get_node(src)?;
        self.get_node(dst)?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_edges (src, dst, relation, weight, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(src, dst, relation) DO UPDATE SET
               weight = excluded.weight,
               created_at = excluded.created_at",
            params![src, dst, relation, weight, now_ms()],
        )?;
        Ok(())
    }

    /// Delete an edge
    pub fn delete_edge(&self, src: &str, dst: &str, relation: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "DELETE FROM _adb_edges WHERE src = ?1 AND dst = ?2 AND relation = ?3",
            params![src, dst, relation],
        )?;
        if changed == 0 {
            return Err(AgentDbError::EdgeNotFound {
                src: src.to_string(),
                dst: dst.to_string(),
            });
        }
        Ok(())
    }

    /// Traverse the graph from a starting node using recursive CTEs
    pub fn neighbors(&self, node_id: &str, opts: TraversalOptions) -> Result<Vec<TraversalResult>> {
        let conn = self.conn.lock().unwrap();
        let max_depth = opts.max_depth.max(1) as i64;
        let min_weight = opts.min_weight.unwrap_or(0.0);

        let results = if let Some(ref relation) = opts.relation {
            let sql = "
                WITH RECURSIVE traverse(node_id, depth, weight) AS (
                    SELECT dst, 1, weight
                    FROM _adb_edges
                    WHERE src = ?1 AND relation = ?2 AND weight >= ?4
                    UNION ALL
                    SELECT e.dst, t.depth + 1, e.weight
                    FROM _adb_edges e
                    JOIN traverse t ON e.src = t.node_id
                    WHERE t.depth < ?3
                      AND e.relation = ?2
                      AND e.weight >= ?4
                )
                SELECT DISTINCT n.id, n.kind, n.data, n.created_at, n.updated_at,
                       t.depth, t.weight
                FROM traverse t
                JOIN _adb_nodes n ON n.id = t.node_id
                ORDER BY t.depth ASC, t.weight DESC
            ";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(
                params![node_id, relation, max_depth, min_weight],
                parse_traversal_row,
            )?;
            rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect::<Result<Vec<_>>>()?
        } else {
            let sql = "
                WITH RECURSIVE traverse(node_id, depth, weight) AS (
                    SELECT dst, 1, weight
                    FROM _adb_edges
                    WHERE src = ?1 AND weight >= ?3
                    UNION ALL
                    SELECT e.dst, t.depth + 1, e.weight
                    FROM _adb_edges e
                    JOIN traverse t ON e.src = t.node_id
                    WHERE t.depth < ?2
                      AND e.weight >= ?3
                )
                SELECT DISTINCT n.id, n.kind, n.data, n.created_at, n.updated_at,
                       t.depth, t.weight
                FROM traverse t
                JOIN _adb_nodes n ON n.id = t.node_id
                ORDER BY t.depth ASC, t.weight DESC
            ";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(
                params![node_id, max_depth, min_weight],
                parse_traversal_row,
            )?;
            rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect::<Result<Vec<_>>>()?
        };

        Ok(results)
    }

    /// List all nodes of a given kind
    pub fn nodes_by_kind(&self, kind: &str) -> Result<Vec<Node>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, data, created_at, updated_at FROM _adb_nodes WHERE kind = ?1"
        )?;
        let rows = stmt.query_map(params![kind], |row| {
            let data_str: Option<String> = row.get(2)?;
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                data: data_str.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Count all nodes and edges
    pub fn stats(&self) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let nodes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM _adb_nodes", [], |r| r.get(0)
        )?;
        let edges: i64 = conn.query_row(
            "SELECT COUNT(*) FROM _adb_edges", [], |r| r.get(0)
        )?;
        Ok((nodes, edges))
    }
}

fn parse_traversal_row(row: &rusqlite::Row) -> rusqlite::Result<TraversalResult> {
    let data_str: Option<String> = row.get(2)?;
    Ok(TraversalResult {
        node: Node {
            id: row.get(0)?,
            kind: row.get(1)?,
            data: data_str.as_deref().and_then(|s| serde_json::from_str(s).ok()),
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        },
        depth: row.get::<_, i64>(5)? as usize,
        weight: row.get(6)?,
    })
}
