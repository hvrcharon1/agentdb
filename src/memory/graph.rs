use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A typed node in the memory graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Node {
    /// Unique identifier for this node.
    pub id: String,
    /// Semantic type label (e.g. `"session"`, `"thought"`, `"tool"`).
    pub kind: String,
    /// Arbitrary JSON payload attached to the node.
    pub data: Option<Value>,
    /// Unix-millisecond timestamp when the node was first created.
    pub created_at: i64,
    /// Unix-millisecond timestamp of the most recent update.
    pub updated_at: i64,
}

/// A directed, weighted edge between two nodes in the memory graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    /// ID of the source node.
    pub src: String,
    /// ID of the destination node.
    pub dst: String,
    /// Semantic relationship label (e.g. `"recalled"`, `"leads_to"`).
    pub relation: String,
    /// Importance or strength of the relationship, conventionally in `[0.0, 1.0]`.
    pub weight: f64,
    /// Unix-millisecond timestamp when the edge was created or last updated.
    pub created_at: i64,
}

/// Options controlling how the memory graph is traversed.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TraversalOptions {
    /// If set, only follow edges whose relation label matches this string.
    pub relation: Option<String>,
    /// Maximum number of hops from the anchor node.
    pub max_depth: usize,
    /// Discard edges whose weight is below this threshold.
    pub min_weight: Option<f64>,
}

/// A single node returned by a graph traversal, with path metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TraversalResult {
    /// The reached node.
    pub node: Node,
    /// Number of hops from the anchor node.
    pub depth: usize,
    /// Weight of the edge that connected this node to its parent in the traversal.
    pub weight: f64,
}

/// In-process memory graph backed by a SQLite WAL database.
pub struct MemoryGraph {
    conn: Arc<Mutex<Connection>>,
}

impl MemoryGraph {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Add or update a node. If a node with this `id` already exists it is overwritten
    /// in place (kind, data, and updated_at are refreshed; created_at is preserved).
    pub fn add_node(&self, id: &str, kind: &str, data: Option<Value>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let data_str = data.as_ref().map(|d| d.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_nodes (id, kind, data, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               kind       = excluded.kind,
               data       = excluded.data,
               updated_at = excluded.updated_at",
            params![id, kind, data_str, now, now],
        )?;
        Ok(())
    }

    /// Retrieve a node by its ID.
    ///
    /// Returns [`AgentDbError::NodeNotFound`] if no node with that ID exists.
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
                    data: data_str
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok()),
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .map_err(|_| AgentDbError::NodeNotFound(id.to_string()))
    }

    /// Remove a node by its ID. Associated edges are also deleted via ON DELETE CASCADE.
    pub fn delete_node(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM _adb_nodes WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Add or update a directed edge `src → dst` with the given `relation` label and `weight`.
    ///
    /// Both `src` and `dst` must already exist; returns [`AgentDbError::NodeNotFound`] otherwise.
    /// If an edge with the same `(src, dst, relation)` triple already exists its weight is updated.
    pub fn add_edge(&self, src: &str, dst: &str, relation: &str, weight: f64) -> Result<()> {
        self.get_node(src)?;
        self.get_node(dst)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_edges (src, dst, relation, weight, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(src, dst, relation) DO UPDATE SET
               weight     = excluded.weight,
               created_at = excluded.created_at",
            params![src, dst, relation, weight, now_ms()],
        )?;
        Ok(())
    }

    /// Remove the edge `src → dst` with the given `relation`.
    ///
    /// Returns [`AgentDbError::EdgeNotFound`] if no such edge exists.
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

    /// Traverse the graph from `node_id` and return all reachable nodes within the
    /// constraints defined by `opts`.
    ///
    /// Uses a recursive Common Table Expression (CTE) for efficient SQLite-side
    /// traversal. Results are ordered by ascending depth, then descending edge weight.
    pub fn neighbors(&self, node_id: &str, opts: TraversalOptions) -> Result<Vec<TraversalResult>> {
        let conn = self.conn.lock().unwrap();
        let max_depth = opts.max_depth.max(1) as i64;
        let min_weight = opts.min_weight.unwrap_or(0.0);

        let results = if let Some(ref relation) = opts.relation {
            let sql = "
                WITH RECURSIVE traverse(node_id, depth, weight, visited) AS (
                    SELECT dst, 1, weight, ',' || ?1 || ',' || dst || ','
                    FROM _adb_edges
                    WHERE src = ?1 AND relation = ?2 AND weight >= ?4
                    UNION ALL
                    SELECT e.dst, t.depth + 1, e.weight,
                           t.visited || e.dst || ','
                    FROM _adb_edges e
                    JOIN traverse t ON e.src = t.node_id
                    WHERE t.depth < ?3
                      AND e.relation = ?2
                      AND e.weight >= ?4
                      AND INSTR(t.visited, ',' || e.dst || ',') = 0
                )
                SELECT n.id, n.kind, n.data, n.created_at, n.updated_at,
                       MIN(t.depth) AS depth, MAX(t.weight) AS weight
                FROM traverse t
                JOIN _adb_nodes n ON n.id = t.node_id
                GROUP BY n.id
                ORDER BY depth ASC, weight DESC
            ";
            let mut stmt = conn.prepare(sql)?;
            let rows =
                stmt.query_map(params![node_id, relation, max_depth, min_weight], parse_row)?;
            rows.map(|r| r.map_err(AgentDbError::Sqlite))
                .collect::<Result<Vec<_>>>()?
        } else {
            let sql = "
                WITH RECURSIVE traverse(node_id, depth, weight, visited) AS (
                    SELECT dst, 1, weight, ',' || ?1 || ',' || dst || ','
                    FROM _adb_edges
                    WHERE src = ?1 AND weight >= ?3
                    UNION ALL
                    SELECT e.dst, t.depth + 1, e.weight,
                           t.visited || e.dst || ','
                    FROM _adb_edges e
                    JOIN traverse t ON e.src = t.node_id
                    WHERE t.depth < ?2
                      AND e.weight >= ?3
                      AND INSTR(t.visited, ',' || e.dst || ',') = 0
                )
                SELECT n.id, n.kind, n.data, n.created_at, n.updated_at,
                       MIN(t.depth) AS depth, MAX(t.weight) AS weight
                FROM traverse t
                JOIN _adb_nodes n ON n.id = t.node_id
                GROUP BY n.id
                ORDER BY depth ASC, weight DESC
            ";
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map(params![node_id, max_depth, min_weight], parse_row)?;
            rows.map(|r| r.map_err(AgentDbError::Sqlite))
                .collect::<Result<Vec<_>>>()?
        };

        Ok(results)
    }

    /// Return all nodes whose `kind` field matches the given string.
    pub fn nodes_by_kind(&self, kind: &str) -> Result<Vec<Node>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, data, created_at, updated_at FROM _adb_nodes WHERE kind = ?1",
        )?;
        let rows = stmt.query_map(params![kind], |row| {
            let data_str: Option<String> = row.get(2)?;
            Ok(Node {
                id: row.get(0)?,
                kind: row.get(1)?,
                data: data_str
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Return the total node and edge counts as `(nodes, edges)`.
    pub fn stats(&self) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let nodes: i64 = conn.query_row("SELECT COUNT(*) FROM _adb_nodes", [], |r| r.get(0))?;
        let edges: i64 = conn.query_row("SELECT COUNT(*) FROM _adb_edges", [], |r| r.get(0))?;
        Ok((nodes, edges))
    }
}

fn parse_row(row: &rusqlite::Row) -> rusqlite::Result<TraversalResult> {
    let data_str: Option<String> = row.get(2)?;
    Ok(TraversalResult {
        node: Node {
            id: row.get(0)?,
            kind: row.get(1)?,
            data: data_str
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        },
        depth: row.get::<_, i64>(5)? as usize,
        weight: row.get(6)?,
    })
}
