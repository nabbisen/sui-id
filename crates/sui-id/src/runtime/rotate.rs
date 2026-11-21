//! RFC 100 stage 0: the key-file half of master-key rotation, moved onto
//! the adapter, behaviour unchanged. `cli.rs`'s `run_admin_rotate_key`
//! (a separate crate target from this library) calls [`rotate_key_files`]
//! with [`key_adapter::default_adapter`] for production; this crate's own
//! `#[cfg(test)]` tests call it directly with
//! [`key_adapter::simulated::Simulated`] to reproduce the crash-injection
//! harness's prototype against the real code.

use crate::key_adapter::KeyFileOps;
use anyhow::Context;
use std::path::Path;
use sui_id_store::crypto::MasterKey;
use sui_id_store::{Database, StoreError};

#[path = "rotate/preconditions.rs"]
pub mod preconditions;

#[path = "rotate/journal.rs"]
pub mod journal;

/// What [`rotate_key_files`] reports on success: the same per-table
/// counts `reseal_master_key` already returns.
pub type RotateReport = sui_id_store::commands::RotationReport;

/// [`rotate_key_files`]'s error: [`NotDurable`](Self::NotDurable) keeps
/// the re-seal's own `StoreError::RotationNotDurable` typed, so the
/// caller can still special-case it exactly as `run_admin_rotate_key`
/// always has; everything else is an ordinary [`anyhow::Error`].
#[derive(Debug)]
pub enum RotateKeyFilesError {
    NotDurable(StoreError),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RotateKeyFilesError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

/// Stage, re-seal, and publish a new master key — the sequence
/// `run_admin_rotate_key` ran inline before this stage, now behind the
/// adapter. `bak_path` is computed by the caller (it carries a
/// timestamp cli.rs prints), not here.
pub async fn rotate_key_files(
    adapter: &dyn KeyFileOps,
    db: &Database,
    new_key: &MasterKey,
    old_path: &Path,
    staged_path: &Path,
    bak_path: &Path,
    new_key_b64: &str,
) -> Result<RotateReport, RotateKeyFilesError> {
    // Stage the new key at `staged_path`, durable before this returns —
    // the only durable copy of a generated key until the final rename.
    crate::keyring::write_key_file_impl(adapter, "rotate.stage", staged_path, new_key_b64)
        .with_context(|| format!("staging new key at {}", staged_path.display()))?;

    // Re-seal everything atomically: the re-seal and its audit record
    // commit in one transaction, and K07 (`0.91.0` A2) checkpoints
    // before returning, so the commit itself is durable by the time
    // this call returns.
    let report = match sui_id_store::commands::reseal_master_key(db, new_key).await {
        Err(e @ StoreError::RotationNotDurable) => return Err(RotateKeyFilesError::NotDurable(e)),
        r => r
            .map_err(anyhow::Error::new)
            .context("rotating sealed columns under the new key")?
            .into_inner(),
    };
    // Today both facts are one return of `reseal_master_key`, so they
    // are recorded one after the other here — [`Real`] ignores both;
    // [`Simulated`] records them as two separate crash points, letting
    // a test-only sequence drop the second without touching this one.
    //
    // [`Real`]: crate::key_adapter::Real
    // [`Simulated`]: crate::key_adapter::simulated::Simulated
    adapter.db_committed("rotate.reseal");
    adapter.db_durable("rotate.reseal");

    // Rename the old key file. Done AFTER the transaction commits so a
    // failure in re-sealing does not leave the operator with a renamed
    // file and an un-rotated DB.
    adapter
        .rename("rotate.rename.1", old_path, bak_path)
        .with_context(|| format!("renaming {} -> {}", old_path.display(), bak_path.display()))?;
    crate::keyring::sync_parent_dir_impl(adapter, "rotate.sync_dir.after_rename.1", old_path)
        .with_context(|| {
            format!(
                "syncing the directory holding {} after archiving the old key to {} -- the new \
                 key is still at {}, not yet in place, and nothing is at {}. Move {} to {} by \
                 hand before starting sui-id or running any sui-id command: with no file there, \
                 it would generate a new, unrelated key",
                old_path.display(),
                bak_path.display(),
                staged_path.display(),
                old_path.display(),
                staged_path.display(),
                old_path.display()
            )
        })?;

    // Move the already-durable staged key into place. A rename within
    // one directory — if it fails, the key is still at `staged_path`,
    // recoverable by hand.
    adapter
        .rename("rotate.rename.2", staged_path, old_path)
        .with_context(|| {
            format!(
                "renaming {} -> {}. The database is sealed under the key at {}, and nothing is \
                 at {}. Move it there by hand before starting sui-id or running any sui-id \
                 command: with no file there, it would generate a new, unrelated key",
                staged_path.display(),
                old_path.display(),
                staged_path.display(),
                old_path.display()
            )
        })?;
    crate::keyring::sync_parent_dir_impl(adapter, "rotate.sync_dir.after_rename.2", old_path)
        .with_context(|| {
            format!(
                "syncing the directory holding {} after moving the new key into place -- the \
                 new key is already at {}, archived old key at {}",
                old_path.display(),
                old_path.display(),
                bak_path.display()
            )
        })?;

    Ok(report)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "rotate/tests.rs"]
mod tests;
