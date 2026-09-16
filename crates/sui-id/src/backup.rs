//! Backup and restore — the CLI-facing adapter.
//!
//! The implementation lives in [`sui_id_store::backup`], where raw database
//! access belongs (RFC 094 handoff `backup-into-store.md`). This module keeps
//! the signatures the CLI and the e2e tests have always used: it reads the
//! database path, key-file path and issuer from [`Config`] and converts the
//! store's typed [`BackupError`] into `anyhow`, which prints the same message
//! chain as before the move.

use crate::config::Config;
use anyhow::Result;
use std::path::Path;

pub use sui_id_store::backup::{
    BackupError, BackupOptions, FORMAT_VERSION, Manifest, RestoreOptions, VerifyReport,
};

/// Write a backup of the configured database and master key to `dest`.
pub fn run_backup(cfg: &Config, dest: &Path, opts: &BackupOptions) -> Result<()> {
    sui_id_store::backup::run_backup(
        &cfg.storage.db_path,
        &cfg.storage.key_file,
        &cfg.server.issuer,
        dest,
        opts,
    )?;
    Ok(())
}

/// Restore a backup tarball into the configured storage paths.
pub fn run_restore(cfg: &Config, src: &Path, opts: &RestoreOptions) -> Result<()> {
    sui_id_store::backup::run_restore(&cfg.storage.db_path, &cfg.storage.key_file, src, opts)?;
    Ok(())
}

/// Read a backup file and report what it contains. Nothing is written except
/// a temporary copy of the snapshot for the SQLite integrity check.
pub fn run_verify(src: &Path, passphrase: Option<&str>) -> Result<VerifyReport> {
    Ok(sui_id_store::backup::run_verify(src, passphrase)?)
}
