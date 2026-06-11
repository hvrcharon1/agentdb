use thiserror::Error;

/// All errors that AgentDB operations can produce.
///
/// Every public API function returns [`Result<T>`] which aliases
/// `std::result::Result<T, AgentDbError>`.
#[derive(Debug, Error)]
pub enum AgentDbError {
    /// Underlying SQLite operation failed.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// JSON (de)serialization failed; the message describes what went wrong.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The named vector collection does not exist in the database.
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),

    /// The caller supplied a vector whose length differs from the collection's
    /// declared dimensionality.
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    /// A memory graph node with the given ID does not exist.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// A directed edge between the given nodes does not exist.
    #[error("Edge not found: {src} -> {dst}")]
    EdgeNotFound { src: String, dst: String },

    /// The on-disk schema version is newer or older than this library expects.
    /// Run `agentdb migrate` to bring the database up to date.
    #[error("Schema version mismatch: run agentdb migrate")]
    SchemaMigration,

    /// Low-level database corruption was detected (checksum failure,
    /// unexpected NULL in a required column, etc.).
    #[error("Database corrupted: {0}")]
    Corruption(String),

    /// A caller-supplied argument was out of range or otherwise invalid.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

/// Convenience alias — all AgentDB public APIs return this type.
pub type Result<T> = std::result::Result<T, AgentDbError>;
