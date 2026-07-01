use crate::error::{AgentDbError, Result};
use crate::schema::now_ms;
use rusqlite::params;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A full-text search result for a message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageSearchResult {
    /// ID of the matching message.
    pub message_id: String,
    /// ID of the conversation the message belongs to.
    pub conversation_id: String,
    /// BM25-ranked snippet of the matched content.
    pub snippet: String,
    /// BM25 rank score (lower is better; negate for descending sort).
    pub rank: f64,
}

/// A conversation thread.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Conversation {
    /// Unique identifier for this conversation.
    pub id: String,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Arbitrary JSON payload attached to the conversation.
    pub metadata: Option<Value>,
    /// Unix-millisecond timestamp when the conversation was created.
    pub created_at: i64,
    /// Unix-millisecond timestamp of the most recent update.
    pub updated_at: i64,
}

/// A single message within a conversation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: String,
    /// ID of the parent conversation.
    pub conversation_id: String,
    /// Role of the sender (e.g. `"user"`, `"assistant"`, `"system"`).
    pub role: String,
    /// Text content of the message.
    pub content: String,
    /// Arbitrary JSON payload attached to the message.
    pub metadata: Option<Value>,
    /// Unix-millisecond timestamp when the message was created.
    pub created_at: i64,
}

/// Manages conversation threads and their messages.
pub struct ConversationStore {
    conn: Arc<Mutex<Connection>>,
}

impl ConversationStore {
    pub(crate) fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Create a new conversation. The `id` must be unique; use
    /// `uuid::Uuid::new_v4().to_string()` if you do not have a stable ID.
    pub fn create_conversation(
        &self,
        id: &str,
        title: Option<&str>,
        metadata: Option<Value>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let meta_str = metadata.as_ref().map(|m| m.to_string());
        let now = now_ms();
        conn.execute(
            "INSERT INTO _adb_conversations (id, title, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, meta_str, now, now],
        )?;
        Ok(())
    }

    /// Append a message to an existing conversation.
    ///
    /// The INSERT and the updated_at bump run inside a single lock acquisition
    /// so a crash between the two operations cannot leave a stale timestamp.
    ///
    /// Returns the newly generated message ID.
    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        metadata: Option<Value>,
    ) -> Result<String> {
        let msg_id = Uuid::new_v4().to_string();
        let meta_str = metadata.as_ref().map(|m| m.to_string());
        let now = now_ms();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO _adb_messages
                 (id, conversation_id, role, content, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![msg_id, conversation_id, role, content, meta_str, now],
        )?;
        conn.execute(
            "UPDATE _adb_conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, conversation_id],
        )?;
        conn.execute(
            "INSERT INTO _adb_messages_fts (message_id, conversation_id, content)
             VALUES (?1, ?2, ?3)",
            params![&msg_id, conversation_id, content],
        )?;
        Ok(msg_id)
    }

    /// Return messages for a conversation in chronological order.
    ///
    /// If `limit` is `Some(n)` only the most-recent `n` messages are returned
    /// (still in ascending chronological order). Pass `None` for all messages.
    pub fn get_messages(
        &self,
        conversation_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let rows: Vec<Message> = match limit {
            Some(n) => {
                // Fetch the last n rows via a sub-query so they come back in
                // ascending (chronological) order.
                let mut stmt = conn.prepare(
                    "SELECT id, conversation_id, role, content, metadata, created_at
                     FROM (
                         SELECT id, conversation_id, role, content, metadata, created_at
                         FROM _adb_messages
                         WHERE conversation_id = ?1
                         ORDER BY created_at DESC
                         LIMIT ?2
                     )
                     ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![conversation_id, n as i64], parse_message)?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, conversation_id, role, content, metadata, created_at
                     FROM _adb_messages
                     WHERE conversation_id = ?1
                     ORDER BY created_at ASC",
                )?;
                let rows = stmt.query_map(params![conversation_id], parse_message)?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }

    /// List all conversations ordered by most-recently updated first.
    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, metadata, created_at, updated_at
             FROM _adb_conversations
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], parse_conversation)?;
        rows.map(|r| r.map_err(AgentDbError::Sqlite)).collect()
    }

    /// Delete a conversation and all its messages (via ON DELETE CASCADE).
    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM _adb_messages_fts WHERE conversation_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM _adb_conversations WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Full-text search over all message content.
    ///
    /// Returns up to `top_k` results ranked by BM25 relevance.
    /// Optionally filter to a single conversation with `conversation_id`.
    pub fn search_messages(
        &self,
        query: &str,
        top_k: usize,
        conversation_id: Option<&str>,
    ) -> Result<Vec<MessageSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let rows = match conversation_id {
            Some(cid) => {
                let mut stmt = conn.prepare(
                    "SELECT message_id, conversation_id,
                            snippet(_adb_messages_fts, 2, '<b>', '</b>', '...', 10),
                            rank
                     FROM _adb_messages_fts
                     WHERE _adb_messages_fts MATCH ?1
                       AND conversation_id = ?2
                     ORDER BY rank
                     LIMIT ?3",
                )?;
                let rows = stmt.query_map(
                    params![query, cid, top_k as i64],
                    parse_message_search_result,
                )?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT message_id, conversation_id,
                            snippet(_adb_messages_fts, 2, '<b>', '</b>', '...', 10),
                            rank
                     FROM _adb_messages_fts
                     WHERE _adb_messages_fts MATCH ?1
                     ORDER BY rank
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(
                    params![query, top_k as i64],
                    parse_message_search_result,
                )?;
                rows.map(|r| r.map_err(AgentDbError::Sqlite))
                    .collect::<Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }
}

// ── Row parsers ──────────────────────────────────────────────────────────────

fn parse_message_search_result(row: &rusqlite::Row) -> rusqlite::Result<MessageSearchResult> {
    Ok(MessageSearchResult {
        message_id:      row.get(0)?,
        conversation_id: row.get(1)?,
        snippet:         row.get(2)?,
        rank:            row.get(3)?,
    })
}

fn parse_conversation(row: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    let meta_str: Option<String> = row.get(2)?;
    Ok(Conversation {
        id: row.get(0)?,
        title: row.get(1)?,
        metadata: meta_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn parse_message(row: &rusqlite::Row) -> rusqlite::Result<Message> {
    let meta_str: Option<String> = row.get(4)?;
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        metadata: meta_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.get(5)?,
    })
}
