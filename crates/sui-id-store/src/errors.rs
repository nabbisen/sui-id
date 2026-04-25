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
}

pub type StoreResult<T> = Result<T, StoreError>;
