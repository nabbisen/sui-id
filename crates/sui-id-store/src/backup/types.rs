//! Backup public types.

use serde::{Deserialize, Serialize};

// The encrypted-envelope magic bytes and the Argon2id key-derivation
// parameters these paragraphs used to document now live with their actual
// declarations in `ops.rs` (`ENCRYPTED_MAGIC` and the module-level format
// doc); nothing here declared them, so this text was dangling.

/// Provenance metadata written into every backup. `restore` consults
/// `format_version` and `schema_version` before doing anything
/// destructive; everything else is for the operator to read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub sui_id_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub hostname: String,
    pub issuer: String,
}

#[derive(Debug, Default, Clone)]
pub struct BackupOptions {
    /// When `Some`, the backup is encrypted under a key derived from
    /// the passphrase. When `None`, a plain tarball is produced.
    pub passphrase: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct RestoreOptions {
    pub force: bool,
    /// Required when the backup file is encrypted. Optional otherwise
    /// (a plain tarball is accepted with `passphrase = None`).
    pub passphrase: Option<String>,
}

/// Result of `verify-backup` — purely informational.
#[derive(Debug, Clone)]
pub struct VerifyReport {
    pub manifest: Manifest,
    pub encrypted: bool,
    /// Total bytes of the tar (post-decrypt if encrypted).
    pub tar_bytes: usize,
    /// Bytes of the inner SQLite snapshot.
    pub db_bytes: usize,
    /// Whether the master key entry is present.
    pub key_present: bool,
}

/// Every way `run_backup`, `run_restore` and `run_verify` can refuse or fail.
///
/// A dedicated type rather than new [`crate::StoreError`] variants: `StoreError`
/// is mapped onto HTTP and API error codes by `sui-id-core`, and archive-format
/// refusals that no request path can produce do not belong in that mapping.
/// One variant per refusal; each `Display` is the message the CLI printed
/// before the move into this crate, and underlying causes are kept as
/// `#[source]` so an `anyhow` report (`{:#}`) prints the same chain.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    // ---- backup ----
    #[error("refusing to overwrite existing file {}", .0.display())]
    DestinationExists(std::path::PathBuf),
    #[error("configured database does not exist at {}", .0.display())]
    DatabaseMissing(std::path::PathBuf),
    #[error("configured key file does not exist at {}", .0.display())]
    KeyFileMissing(std::path::PathBuf),
    #[error("creating temp dir for snapshot")]
    CreateTempDir(#[source] std::io::Error),
    #[error("opening source database for snapshot")]
    OpenSourceDatabase(#[source] rusqlite::Error),
    #[error("snapshot path must be valid UTF-8")]
    SnapshotPathNotUtf8,
    #[error("VACUUM INTO failed")]
    VacuumInto(#[source] rusqlite::Error),
    #[error("reading database snapshot")]
    ReadSnapshot(#[source] std::io::Error),
    #[error("reading master key file")]
    ReadKeyFile(#[source] std::io::Error),
    #[error("reopening snapshot to read schema_version")]
    ReopenSnapshot(#[source] rusqlite::Error),
    #[error("serialising MANIFEST")]
    SerializeManifest(#[source] serde_json::Error),
    #[error("creating backup file {}", .path.display())]
    CreateBackupFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    // ---- restore / verify: reading the archive ----
    #[error("backup file {} does not exist", .0.display())]
    SourceMissing(std::path::PathBuf),
    #[error("reading {}", .path.display())]
    ReadSource {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("this backup is encrypted; supply --decrypt and provide the passphrase")]
    PassphraseRequired,
    #[error("backup file is not encrypted, but a passphrase was provided")]
    PassphraseForPlainBackup,
    #[error("parsing MANIFEST.json")]
    ParseManifest(#[source] serde_json::Error),
    #[error("backup is missing sui-id.sqlite entry")]
    MissingDatabaseEntry,
    #[error("backup is missing sui-id.key entry")]
    MissingKeyEntry,

    // ---- restore: compatibility and destinations ----
    #[error(
        "backup format_version {found} is newer than this build supports ({supported}). \
         Restore on a newer sui-id or downgrade the backup."
    )]
    FormatVersionTooNew { found: u32, supported: u32 },
    #[error(
        "backup schema_version {found} is newer than this build supports (max {max}). \
         Use a newer sui-id binary to restore this backup."
    )]
    SchemaVersionTooNew { found: i64, max: i64 },
    /// RFC 112 D4: the database inside the snapshot (or being snapshotted) has
    /// a schema version that cannot be trusted: unreadable, or missing from a
    /// database that has tables. Never recorded as `0`, never restored.
    #[error("the database's schema version is unreadable: {0}")]
    SchemaVersionUnreadable(String),
    #[error(
        "refusing to overwrite existing database at {} (pass --force to override)",
        .0.display()
    )]
    DatabaseExists(std::path::PathBuf),
    #[error(
        "refusing to overwrite existing key file at {} (pass --force to override)",
        .0.display()
    )]
    KeyFileExists(std::path::PathBuf),
    #[error("creating temp file {}", .path.display())]
    CreateTempFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("renaming temp file into {}", .path.display())]
    RenameTempFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    // ---- verify ----
    #[error("staging snapshot for integrity check")]
    StageSnapshot(#[source] std::io::Error),
    #[error("opening snapshot for integrity check")]
    OpenStagedSnapshot(#[source] rusqlite::Error),
    #[error("running integrity_check")]
    RunIntegrityCheck(#[source] rusqlite::Error),
    #[error("SQLite integrity_check failed: {0}")]
    IntegrityCheckFailed(String),

    // ---- encrypted envelope ----
    #[error("encryption failed")]
    Encrypt,
    #[error("encrypted backup truncated (header missing)")]
    EnvelopeTruncated,
    #[error("encrypted backup magic mismatch")]
    EnvelopeMagicMismatch,
    #[error(
        "encrypted backup envelope version {found} is not supported (this build supports {supported})"
    )]
    EnvelopeVersionUnsupported { found: u32, supported: u32 },
    #[error("encrypted backup nonce has invalid length")]
    EnvelopeNonceLength,
    #[error("could not decrypt backup — wrong passphrase, or the file has been tampered with")]
    Decrypt,
    #[error("argon2 params: {0}")]
    Argon2Params(String),
    #[error("argon2 derive: {0}")]
    Argon2Derive(String),

    // ---- tar ----
    #[error("tar entry name too long: {0}")]
    TarNameTooLong(String),
    #[error("tar entry name is not UTF-8")]
    TarNameNotUtf8(#[source] std::str::Utf8Error),
    #[error("truncated tar entry for {0}")]
    TarTruncatedEntry(String),
    #[error("tar archive contains no entries")]
    TarEmpty,
    #[error("invalid octal digit in tar header")]
    TarInvalidOctal,

    /// A write to the output stream failed. Before the move this was a bare
    /// `?` on `std::io::Error` with no added context; `transparent` keeps it so.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
