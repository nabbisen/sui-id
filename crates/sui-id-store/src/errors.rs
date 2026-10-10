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

    /// RFC 112 D6: the database's stored schema version is **newer** than this
    /// build's `migrations::MAX_SCHEMA_VERSION`. Nothing was written. `found` is
    /// `i64` so a stored value beyond `i32` is reported as what it is: too new.
    #[error(
        "the database schema is newer than this build understands \
         (found version {found}; this build supports up to {supported})"
    )]
    SchemaTooNew { found: i64, supported: i32 },

    /// RFC 112 D2, D6: the stored schema version cannot be trusted: unreadable
    /// (not a canonical non-negative integer, empty, a BLOB, negative) or
    /// **absent from a database that has application tables** (a foreign SQLite
    /// file, or a sui-id database whose row was lost). `tables` is how many
    /// application tables the database has. Nothing was written, and no
    /// migration ran: re-running them from 0001 against a populated database is
    /// the defect this refuses.
    #[error("the database schema version is unreadable: {detail} ({tables} application table(s))")]
    SchemaVersionInvalid { tables: usize, detail: String },

    /// RFC 112 D6: a migration failed to apply. It used to surface as
    /// `Db`, whose Display is "database I/O error", which sends an operator to
    /// look at their disk.
    #[error("migration {version} failed to apply")]
    MigrationFailed {
        version: i32,
        #[source]
        source: rusqlite::Error,
    },

    /// RFC 096-B1 stage 0 / RFC 094's 2026-08-12 amendment: `ReadConn`
    /// refused to prepare a statement because `sqlite3_stmt_readonly`
    /// did not call it read-only. Nothing was read or written through
    /// it — see `read_conn.rs`'s module doc.
    #[error("ReadConn refused a non-read-only statement: {sql}")]
    NotReadOnly { sql: String },

    /// RFC 096-B1 stage 0: `ReadConn` refuses every `PRAGMA`
    /// unconditionally, because `sqlite3_stmt_readonly` is documented as
    /// unreliable for `PRAGMA` specifically — see `read_conn.rs`'s module
    /// doc for why this is a zero-cost refusal today.
    #[error("ReadConn refuses PRAGMA unconditionally: {sql}")]
    PragmaRefused { sql: String },

    /// RFC 096-B1 stage 0 (fix): `ReadConn` refuses every transaction-control
    /// or connection-config statement unconditionally, by leading keyword,
    /// because `sqlite3_stmt_readonly` reports all of them read-only despite
    /// their effect on connection, transaction, or filesystem state (SQLite's
    /// own documentation for `sqlite3_stmt_readonly` names this class
    /// explicitly). `keyword` is the matched keyword, upper-cased, so an
    /// operator reading the error knows which rule fired and why.
    #[error(
        "ReadConn refuses {keyword} unconditionally: it changes connection, \
         transaction, or filesystem state even though sqlite3_stmt_readonly \
         reports it read-only ({sql})"
    )]
    NonContentEffectRefused { keyword: String, sql: String },

    /// RFC 096-B1 stage 0 (fix): `ReadConn` refuses every `EXPLAIN` /
    /// `EXPLAIN QUERY PLAN` statement unconditionally. Not independently
    /// exploitable — SQLite never executes the explained statement's own
    /// side effects, only lists its bytecode (checked directly: an
    /// `EXPLAIN ATTACH ...` does not create the attached file) — but an
    /// `EXPLAIN`-prefixed statement reports the same `sqlite3_stmt_readonly`
    /// value as its unprefixed form and defeats a first-keyword check
    /// looking for the unprefixed form, so it is refused rather than relied
    /// on to stay harmless by an undocumented-as-a-guarantee accident of how
    /// `EXPLAIN` happens to behave today.
    #[error("ReadConn refuses EXPLAIN unconditionally: {sql}")]
    ExplainRefused { sql: String },

    /// RFC 096-B1 stage 3: a `federation_login_attempt` claim was refused
    /// because `expires_at <= now_at_claim` (RFC 096 `:587-588`). Distinct
    /// from [`Self::ClockRegression`], which is the *other* time-based
    /// refusal and must not be confused with this one — a backward clock
    /// makes a row look less elapsed, not more, so it cannot be detected by
    /// this check and needs its own.
    #[error("the attempt expired before it was claimed")]
    AttemptExpired,

    /// RFC 096-B1 stage 3: a `federation_login_attempt` claim was refused
    /// because the wall clock read at claim time is earlier than the row's
    /// own `created_at` — RFC 096 `:589`'s "clock regression fails closed."
    /// `expires_at <= now_at_claim` alone is the wrong test under a backward
    /// clock (it makes the window look less elapsed, not more), so this is
    /// checked independently rather than folded into
    /// [`Self::AttemptExpired`].
    #[error("the wall clock read at claim time precedes the attempt's created_at")]
    ClockRegression,
}

/// Why U37 refused to issue a recovery link (RFC 103 D5, D6, D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryRefusal {
    /// The recorded reason is empty (D6).
    ReasonRequired,
    /// The reason is longer than `commands::RECOVERY_REASON_MAX_CHARS`
    /// characters after trimming.
    ReasonTooLong,
    /// The reason contains an ASCII control character.
    ReasonHasControlCharacters,
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
            Self::ReasonTooLong => {
                return write!(
                    f,
                    "the reason is too long (at most {} characters, and at most {} bytes)",
                    crate::commands::RECOVERY_REASON_MAX_CHARS,
                    crate::registry::MAX_ATTRIBUTE_VALUE_BYTES
                );
            }
            Self::ReasonHasControlCharacters => {
                "the reason must not contain control characters such as a newline or tab"
            }
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
