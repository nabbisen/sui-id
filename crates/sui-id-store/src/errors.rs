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

    /// RFC 102 B4: a step-up-gated command found, inside its transaction,
    /// that the acting session is no longer fresh (or no longer live) while
    /// the user has a second factor. Nothing was written; the caller sends
    /// the user to step up again.
    #[error("a fresh step-up is required")]
    StepUpRequired,

    /// RFC 103: U37 (issue a recovery link) refused, before writing
    /// anything. The variant says why, so the operation's caller can show
    /// an explicit message (D5, D8).
    #[error("recovery link refused: {0}")]
    RecoveryRefused(RecoveryRefusal),
}

/// Why U37 refused to issue a recovery link (RFC 103 D5, D6, D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryRefusal {
    /// The recorded reason is empty (D6).
    ReasonRequired,
    /// No user has that id.
    TargetUnknown,
    /// The target is the issuing administrator (web only): self-service
    /// password change and forgot-password exist for that.
    TargetIsSelf,
    /// The target is an administrator (web only): administrators recover
    /// only through the CLI, so a stolen admin session cannot capture
    /// another administrator.
    TargetIsAdmin,
    /// The target's identity is not local (RFC 103 T10).
    TargetNonLocal,
    /// The target is disabled.
    TargetDisabled,
    /// The target is deleted.
    TargetDeleted,
    /// The issuer, or the CLI, has already issued the hour's limit (D8).
    Throttled,
}

impl std::fmt::Display for RecoveryRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ReasonRequired => "a reason is required",
            Self::TargetUnknown => "no such user",
            Self::TargetIsSelf => "the target is the issuing administrator",
            Self::TargetIsAdmin => "the target is an administrator",
            Self::TargetNonLocal => "the target is not a local account",
            Self::TargetDisabled => "the target is disabled",
            Self::TargetDeleted => "the target is deleted",
            Self::Throttled => "the hourly limit of recovery links has been reached",
        })
    }
}

pub type StoreResult<T> = Result<T, StoreError>;
