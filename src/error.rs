use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentDbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Edge not found: {src} -> {dst}")]
    EdgeNotFound { src: String, dst: String },
    #[error("Schema version mismatch: run agentdb migrate")]
    SchemaMigration,
    #[error("Database corrupted: {0}")]
    Corruption(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, AgentDbError>;
