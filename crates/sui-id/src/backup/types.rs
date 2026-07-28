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
