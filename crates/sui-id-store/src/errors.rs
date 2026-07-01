//! Store-specific error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database I/O error")]
    Db(#[from] rusqlite::Error),

    #[error("encryption / decryption failure")]
    Crypto,

    #[error("invalid master key length: expected 32 bytes, got {0}")]
    InvalidMasterKeyLength(usize),

    #[error("requested resource was not found")]
    NotFound,

    #[error("requested operation conflicts with current state")]
    Conflict,

    #[error("data integrity violation: {0}")]
    Integrity(String),

    #[error("serialization error")]
    Serde(#[from] serde_json::Error),

    /// A JSON-TEXT column value failed to deserialize. Indicates either
    /// corruption from an out-of-band write or a bug in a previous write
    /// path. Surfaced as a typed error so callers can decide whether to
    /// skip the row, reject the request, or page an operator.
    #[error("corrupt JSON in column '{context}': {source}")]
    CorruptJson {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },

    /// A `tokio::task::spawn_blocking` task panicked or was cancelled.
    /// This is a programming error (the closure panicked) or a runtime
    /// shutdown condition; treated as an internal error by callers.
    #[error("blocking DB task failed: {0}")]
    JoinError(String),

    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type StoreResult<T> = Result<T, StoreError>;
