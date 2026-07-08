//! # Sync Module
//!
//! Hybrid Logical Clock (HLC) based sync engine for multi-node AgentDB replication.
//!
//! The sync engine tracks mutations to any table via an operation log, packs
//! timestamps as `(physical_ms << 16) | logical` so they sort correctly across
//! nodes, and applies remote ops with a pluggable conflict strategy.

use crate::db::AgentDB;
use crate::error::{AgentDbError, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ── Hybrid Logical Clock ──────────────────────────────────────────────────────

/// Hybrid Logical Clock that combines physical milliseconds with a monotonic
/// logical counter.
///
/// Timestamps are packed as a single `i64`:
/// `(physical_ms << 16) | (logical & 0xFFFF)`
pub struct HybridClock {
    physical: i64,
    logical: u32,
    #[allow(dead_code)]
    node_id: String,
}

impl HybridClock {
    /// Create a new HLC seeded from the current wall-clock time.
    pub fn new(node_id: &str) -> Self {
        Self {
            physical: now_ms(),
            logical: 0,
            node_id: node_id.to_string(),
        }
    }

    /// Advance the clock and return a packed HLC timestamp.
    ///
    /// Guarantees that successive calls always produce strictly increasing values.
    pub fn now(&mut self) -> i64 {
        let wall = now_ms();
        if wall > self.physical {
            self.physical = wall;
            self.logical = 0;
        } else {
            // Wall time hasn't moved — bump the logical counter.
            self.logical += 1;
        }
        pack(self.physical, self.logical)
    }

    /// Merge the clock with a remote timestamp, advancing past it if necessary.
    pub fn update(&mut self, remote_ts: i64) {
        let (r_phys, r_log) = unpack(remote_ts);
        let wall = now_ms();
        let new_phys = wall.max(self.physical).max(r_phys);
        if new_phys == self.physical && new_phys == r_phys {
            self.logical = self.logical.max(r_log) + 1;
        } else if new_phys == self.physical {
            self.logical += 1;
        } else if new_phys == r_phys {
            self.logical = r_log + 1;
        } else {
            self.logical = 0;
        }
        self.physical = new_phys;
    }
}

#[inline]
fn pack(physical_ms: i64, logical: u32) -> i64 {
    (physical_ms << 16) | (logical as i64 & 0xFFFF)
}

#[inline]
fn unpack(ts: i64) -> (i64, u32) {
    let phys = ts >> 16;
    let log = (ts & 0xFFFF) as u32;
    (phys, log)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Core types ────────────────────────────────────────────────────────────────

/// The kind of mutation being recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpType {
    Insert,
    Update,
    Delete,
}

impl OpType {
    fn as_str(&self) -> &'static str {
        match self {
            OpType::Insert => "insert",
            OpType::Update => "update",
            OpType::Delete => "delete",
        }
    }

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "insert" => Ok(OpType::Insert),
            "update" => Ok(OpType::Update),
            "delete" => Ok(OpType::Delete),
            other => Err(AgentDbError::InvalidArgument(format!(
                "unknown op_type: {other}"
            ))),
        }
    }
}

/// A single recorded mutation (insert, update, or delete) on a row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOp {
    /// Globally unique operation identifier (UUID v4).
    pub op_id: String,
    /// Packed HLC timestamp (`(physical_ms << 16) | logical`).
    pub hlc_ts: i64,
    /// ID of the node that generated this operation.
    pub node_id: String,
    /// Name of the table that was mutated.
    pub table_name: String,
    /// Primary key of the affected row.
    pub record_id: String,
    /// Kind of mutation.
    pub op_type: OpType,
    /// The new row data (JSON); absent for deletes.
    pub payload: Option<serde_json::Value>,
}

/// Snapshot of a known peer's sync state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatus {
    /// Unique identifier of the peer node.
    pub peer_id: String,
    /// The highest HLC timestamp that was last successfully synced to this peer.
    pub last_synced_hlc: i64,
    /// Optional network endpoint (URL, socket address, etc.).
    pub endpoint: Option<String>,
    /// Number of local ops whose `hlc_ts` is greater than `last_synced_hlc`.
    pub ops_pending: usize,
}

/// Summary of a completed `apply_remote_ops` call.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncResult {
    /// Number of ops that were written / won their conflict.
    pub applied: usize,
    /// Number of ops that collided with an existing op for the same row.
    pub conflicts: usize,
    /// Number of ops that lost their conflict and were not written.
    pub skipped: usize,
}

/// Conflict resolution strategy for `apply_remote_ops`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// The op with the higher HLC timestamp (later writer) wins.
    LastWriterWins,
    /// The op with the lower HLC timestamp (earlier writer) wins.
    FirstWriterWins,
}

// ── SyncEngine ────────────────────────────────────────────────────────────────

/// The primary entry-point for the sync layer.
///
/// Call [`SyncEngine::new`] once per database, then use
/// [`record_mutation`](SyncEngine::record_mutation) after every write and
/// [`apply_remote_ops`](SyncEngine::apply_remote_ops) when you receive a
/// batch from a peer.
pub struct SyncEngine {
    conn: Arc<Mutex<rusqlite::Connection>>,
    clock: HybridClock,
    node_id: String,
    strategy: ConflictStrategy,
}

impl SyncEngine {
    /// Open the sync layer on top of an existing [`AgentDB`] connection.
    ///
    /// Creates `_adb_sync_log` and `_adb_sync_peers` if they don't exist yet.
    pub fn new(db: &AgentDB, node_id: &str) -> Result<Self> {
        // Clone the Arc so we share the same underlying connection pool.
        let conn = db.conn_arc();
        {
            let c = conn.lock().unwrap();
            c.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS _adb_sync_log (
                    op_id       TEXT PRIMARY KEY,
                    hlc_ts      INTEGER NOT NULL,
                    node_id     TEXT NOT NULL,
                    table_name  TEXT NOT NULL,
                    record_id   TEXT NOT NULL,
                    op_type     TEXT NOT NULL CHECK(op_type IN ('insert','update','delete')),
                    payload     TEXT,
                    created_at  TEXT DEFAULT (datetime('now'))
                );
                CREATE INDEX IF NOT EXISTS idx_sync_log_hlc
                    ON _adb_sync_log(hlc_ts);
                CREATE INDEX IF NOT EXISTS idx_sync_log_row
                    ON _adb_sync_log(table_name, record_id);

                CREATE TABLE IF NOT EXISTS _adb_sync_peers (
                    peer_id         TEXT PRIMARY KEY,
                    last_synced_hlc INTEGER NOT NULL DEFAULT 0,
                    endpoint        TEXT,
                    updated_at      TEXT DEFAULT (datetime('now'))
                );
                ",
            )?;
        }
        Ok(Self {
            conn,
            clock: HybridClock::new(node_id),
            node_id: node_id.to_string(),
            strategy: ConflictStrategy::LastWriterWins,
        })
    }

    /// Record a mutation in the sync log and return its `op_id`.
    pub fn record_mutation(
        &mut self,
        table: &str,
        record_id: &str,
        op: OpType,
        payload: Option<serde_json::Value>,
    ) -> Result<String> {
        let op_id = Uuid::new_v4().to_string();
        let hlc_ts = self.clock.now();
        let payload_str = payload.as_ref().map(|v| v.to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_sync_log
                 (op_id, hlc_ts, node_id, table_name, record_id, op_type, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                op_id,
                hlc_ts,
                self.node_id,
                table,
                record_id,
                op.as_str(),
                payload_str,
            ],
        )?;
        Ok(op_id)
    }

    /// Return all ops with `hlc_ts > since_hlc`, ordered by timestamp.
    pub fn get_ops_since(&self, since_hlc: i64) -> Result<Vec<SyncOp>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT op_id, hlc_ts, node_id, table_name, record_id, op_type, payload
             FROM _adb_sync_log
             WHERE hlc_ts > ?1
             ORDER BY hlc_ts ASC",
        )?;
        let rows = stmt.query_map(params![since_hlc], parse_sync_op)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Apply a batch of ops received from a remote peer.
    ///
    /// For each op:
    /// - If no existing op exists for the same `(table_name, record_id)` →
    ///   insert it (counts as `applied`).
    /// - If a conflict exists → apply the configured [`ConflictStrategy`]:
    ///   the winning op is inserted/updated; `conflicts` is always incremented;
    ///   the losing op increments `skipped`.
    pub fn apply_remote_ops(&mut self, ops: Vec<SyncOp>) -> Result<SyncResult> {
        let mut result = SyncResult::default();

        for op in ops {
            // Advance our clock past every incoming timestamp.
            self.clock.update(op.hlc_ts);

            let existing = {
                let conn = self.conn.lock().unwrap();
                conn.query_row(
                    "SELECT op_id, hlc_ts FROM _adb_sync_log
                     WHERE table_name = ?1 AND record_id = ?2
                     LIMIT 1",
                    params![op.table_name, op.record_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?
            };

            match existing {
                None => {
                    // No conflict — just insert.
                    self.insert_op(&op)?;
                    result.applied += 1;
                }
                Some((existing_op_id, existing_hlc)) => {
                    result.conflicts += 1;
                    let incoming_wins = match self.strategy {
                        ConflictStrategy::LastWriterWins => op.hlc_ts > existing_hlc,
                        ConflictStrategy::FirstWriterWins => op.hlc_ts < existing_hlc,
                    };
                    if incoming_wins {
                        // Replace the existing op.
                        let conn = self.conn.lock().unwrap();
                        conn.execute(
                            "DELETE FROM _adb_sync_log WHERE op_id = ?1",
                            params![existing_op_id],
                        )?;
                        drop(conn);
                        self.insert_op(&op)?;
                        result.applied += 1;
                    } else {
                        result.skipped += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Register a peer node, optionally with a network endpoint.
    pub fn add_peer(&self, peer_id: &str, endpoint: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_sync_peers (peer_id, last_synced_hlc, endpoint)
             VALUES (?1, 0, ?2)
             ON CONFLICT(peer_id) DO UPDATE SET
                 endpoint   = excluded.endpoint,
                 updated_at = datetime('now')",
            params![peer_id, endpoint],
        )?;
        Ok(())
    }

    /// Return the sync status of every registered peer.
    pub fn sync_status(&self) -> Result<Vec<PeerStatus>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT peer_id, last_synced_hlc, endpoint
             FROM _adb_sync_peers
             ORDER BY peer_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;

        let mut statuses = Vec::new();
        for row in rows {
            let (peer_id, last_synced_hlc, endpoint) = row.map_err(AgentDbError::Sqlite)?;
            // Count how many local ops have not yet been synced to this peer.
            let ops_pending: i64 = conn.query_row(
                "SELECT COUNT(*) FROM _adb_sync_log WHERE hlc_ts > ?1",
                params![last_synced_hlc],
                |r| r.get(0),
            )?;
            statuses.push(PeerStatus {
                peer_id,
                last_synced_hlc,
                endpoint,
                ops_pending: ops_pending as usize,
            });
        }
        Ok(statuses)
    }

    /// Change the conflict resolution strategy (default: `LastWriterWins`).
    pub fn set_strategy(&mut self, strategy: ConflictStrategy) {
        self.strategy = strategy;
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn insert_op(&self, op: &SyncOp) -> Result<()> {
        let payload_str = op.payload.as_ref().map(|v| v.to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO _adb_sync_log
                 (op_id, hlc_ts, node_id, table_name, record_id, op_type, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                op.op_id,
                op.hlc_ts,
                op.node_id,
                op.table_name,
                op.record_id,
                op.op_type.as_str(),
                payload_str,
            ],
        )?;
        Ok(())
    }
}

// ── Row parser ────────────────────────────────────────────────────────────────

fn parse_sync_op(row: &rusqlite::Row) -> rusqlite::Result<SyncOp> {
    let op_type_str: String = row.get(5)?;
    let payload_str: Option<String> = row.get(6)?;
    let payload = payload_str.and_then(|s| serde_json::from_str(&s).ok());
    let op_type = OpType::from_str(&op_type_str).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(std::fmt::Error),
        )
    })?;
    Ok(SyncOp {
        op_id: row.get(0)?,
        hlc_ts: row.get(1)?,
        node_id: row.get(2)?,
        table_name: row.get(3)?,
        record_id: row.get(4)?,
        op_type,
        payload,
    })
}
