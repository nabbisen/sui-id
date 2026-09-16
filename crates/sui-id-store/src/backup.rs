//! Encrypted backup and restore for sui-id (RFC 040, refactored RFC 075).
//!
//! Lives in `sui-id-store` since 2026-09-16 (RFC 094 handoff
//! `backup-into-store.md`): it opens the database file directly, and raw
//! database access belongs to this crate alone. The functions take the three
//! values they need (database path, key-file path, issuer) explicitly; the
//! `sui-id` binary keeps a thin adapter that reads them from its `Config`.

mod ops;
mod tar;
#[cfg(test)]
mod tests;
mod types;

pub use ops::{run_backup, run_restore, run_verify};
pub use types::{BackupError, BackupOptions, Manifest, RestoreOptions, VerifyReport};

/// Archive format version. Incremented on incompatible format changes.
pub const FORMAT_VERSION: u32 = 1;
