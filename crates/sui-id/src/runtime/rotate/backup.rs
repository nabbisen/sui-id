//! RFC 100 `0.94.0` P3 (stage 7): the file-side half of
//! `admin master-key backup remove` -- verifying and unlinking the
//! pre-rotation backup ([`crate::rotate::preconditions::RotationPaths::backup`]).
//! `admin master-key backup renew` touches no file at all (it only
//! moves `review_at`/`custody_reference` in the database row), so it
//! has no module here: `cli::handlers` calls
//! `sui_id_store::commands::renew_master_key_backup` directly.

use crate::key_adapter::KeyFileOps;
use crate::rotate::preconditions::RotationPaths;
use crate::rotate::recovery;
use anyhow::Context;
use std::path::Path;
use sui_id_store::crypto;

/// What [`remove`] did.
#[derive(Debug)]
pub enum RemovalOutcome {
    /// No file was there to remove -- a later `backup remove`, run
    /// against an already-absent backup, reports this and records
    /// nothing (RFC 100 `0.94.0` P3 §2).
    NothingToRemove,
    Removed {
        rotation_id: String,
        verification_reference: String,
    },
}

/// RFC 100 `0.94.0` P3 §2: verify, unlink, sync, then record -- the
/// dispatch's own exact order, so a crash between the unlink and the
/// database write is the one (accepted, narrow) risk, never the
/// reverse (a record with the file still there). `verification_
/// reference_override` is the operator's `--verification-reference`,
/// resolved against the row's own disposition *before* anything is
/// touched, so a missing override (required only for `RetainUntil`)
/// refuses before the file is ever unlinked.
pub async fn remove(
    adapter: &dyn KeyFileOps,
    db: &sui_id_store::Database,
    key_file: &Path,
    verification_reference_override: Option<String>,
    os_user: String,
) -> anyhow::Result<RemovalOutcome> {
    let paths = RotationPaths::for_key_file(key_file);

    if adapter
        .metadata("backup.remove.check", &paths.backup)
        .is_err()
    {
        return Ok(RemovalOutcome::NothingToRemove);
    }

    let row = sui_id_store::master_key_rotation::read(db)
        .await
        .context("reading the master-key rotation row")?
        .with_context(|| {
            format!(
                "a backup exists at {}, but no master-key rotation row names it -- do not \
                 remove it by hand; it may be the only copy of a key from before this \
                 database's own history began",
                paths.backup.display()
            )
        })?;
    let verification_reference =
        sui_id_store::master_key_rotation::resolve_removal_verification_reference(
            &row,
            verification_reference_override.as_deref(),
        )
        .map_err(anyhow::Error::new)?;

    verify_backup_file(adapter, &paths.backup, &row.old_fingerprint)?;

    adapter
        .remove("backup.remove.unlink", &paths.backup)
        .with_context(|| format!("removing {}", paths.backup.display()))?;
    crate::keyring::sync_parent_dir_impl(adapter, "backup.remove.sync_dir", &paths.backup)
        .context("syncing the key file's directory after removing the backup")?;

    sui_id_store::commands::remove_master_key_backup(db, verification_reference.clone(), os_user)
        .await
        .map_err(anyhow::Error::new)
        .context(
            "the backup file is removed, but recording that in the database failed -- the \
             removal itself is not undone",
        )?;

    Ok(RemovalOutcome::Removed {
        rotation_id: row.rotation_id,
        verification_reference,
    })
}

/// Before unlinking: not a symlink, owner-only (`0600`, this process's
/// own uid), and its content's fingerprint is the row's own
/// `old_fingerprint` -- the same three checks recovery's own
/// `check_published_artifact` makes of a published rotation artifact,
/// specialised here since "absent" was already handled by the caller.
fn verify_backup_file(
    adapter: &dyn KeyFileOps,
    path: &Path,
    expected_fingerprint: &str,
) -> anyhow::Result<()> {
    let meta = adapter
        .metadata("backup.remove.metadata", path)
        .with_context(|| format!("checking {}", path.display()))?;
    if meta.is_symlink {
        anyhow::bail!(
            "refusing to remove: {} is a symlink, not a regular file",
            path.display()
        );
    }
    let my_uid = rustix::process::geteuid().as_raw();
    if meta.uid != my_uid || meta.mode != 0o600 {
        anyhow::bail!(
            "refusing to remove: {} has owner/mode {}:{:o}, not {}:0600",
            path.display(),
            meta.uid,
            meta.mode,
            my_uid
        );
    }
    let key = recovery::read_key_file(adapter, path)
        .with_context(|| format!("reading {}", path.display()))?;
    let fingerprint = crypto::fingerprint(&key);
    if fingerprint != expected_fingerprint {
        anyhow::bail!(
            "refusing to remove: {} has fingerprint {fingerprint}; it should be \
             {expected_fingerprint} (the last rotation's old key)",
            path.display()
        );
    }
    Ok(())
}

/// RFC 100 `0.94.0` P3 §3: the kept-backup finding's own text, shared
/// by every place it appears (a server start's own `tracing::warn!`,
/// and `admin master-key rotate --check`'s own report), so the wording
/// never drifts between them. `None` when there is no row, the
/// disposition is `RemoveAfterVerified`, the backup is already
/// removed, or `review_at` has not passed `now`
/// ([`sui_id_store::master_key_rotation::overdue_backup_review`]'s own
/// rule).
fn overdue_finding_text(
    key_file: &Path,
    row: &sui_id_store::master_key_rotation::MasterKeyRotationRow,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let (review_at, custody_reference) =
        sui_id_store::master_key_rotation::overdue_backup_review(row, now)?;
    let paths = RotationPaths::for_key_file(key_file);
    Some(format!(
        "the pre-rotation backup at {} is past its review date ({}), under custody reference \
         {custody_reference:?}. Settle it with `sui-id admin master-key backup remove` (once \
         retired or re-encrypted) or `sui-id admin master-key backup renew --until \
         <RFC3339_INSTANT> --custody-reference <TEXT>`.",
        paths.backup.display(),
        review_at.to_rfc3339(),
    ))
}

/// `admin master-key rotate --check`'s own case: no database is open
/// (by design -- `--check` creates, opens and changes nothing), only a
/// read-only preview connection. A read-only, best-effort check: an
/// unreadable database (e.g. before it exists) is `None`, never an
/// error, since this is never the reason `--check` itself fails.
pub fn overdue_finding_line_from_paths(
    key_file: &Path,
    db_path: &Path,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let conn = sui_id_store::migrations::open_preview_connection(db_path).ok()?;
    let row = sui_id_store::master_key_rotation::read_within_tx(&conn).ok()??;
    overdue_finding_text(key_file, &row, now)
}

/// A server start's own case: the database is already open, through
/// `state.db`. Reading it the same way `--check` does (its own fresh
/// `rusqlite::Connection`) would contend with that already-open
/// connection -- `open_preview_connection` takes the `immutable=1` fast
/// path exactly when no `-wal` sibling is visible yet to it, which this
/// case cannot rely on, so this goes through the open [`crate`]-visible
/// [`sui_id_store::Database`] instead.
pub async fn overdue_finding_line_from_db(
    key_file: &Path,
    db: &sui_id_store::Database,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<String> {
    let row = sui_id_store::master_key_rotation::read(db).await.ok()??;
    overdue_finding_text(key_file, &row, now)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "backup/tests.rs"]
mod tests;
