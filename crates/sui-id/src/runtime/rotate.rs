//! RFC 100 `0.94.0` P2: `admin master-key rotate`'s live sequence,
//! rewritten onto stages 2-4's own pieces (preconditions, the journal,
//! recovery) and P1's K08/K05 -- the protocol RFC 100 actually
//! specifies, not the pre-100 stage-and-rename sequence this module
//! used to hold. [`rotate`] is every step after the lock and the
//! precondition check (both the caller's own job -- see
//! `cli::handlers::run_admin_rotate_key`): the workspace, the journal,
//! the new key, the backup, K08, activation, K05, and the clean.
//!
//! `cli::handlers::run_admin_rotate_key` (a separate crate target from
//! this library) calls [`rotate`] with [`key_adapter::default_adapter`]
//! for production; this crate's own `#[cfg(test)]` tests call it
//! directly with [`key_adapter::simulated::Simulated`] to enumerate
//! `crash-matrix.md`'s preparation and activation points (1-10) against
//! the real code, the same way stage 0's own (now-removed) harness did
//! for the sequence this replaces.

use crate::key_adapter::KeyFileOps;
use crate::rotate::journal::JournalState;
use crate::rotate::preconditions::RotationPaths;
use anyhow::Context;
use std::path::Path;
use sui_id_store::crypto::MasterKey;
use sui_id_store::master_key_rotation::{RotationDisposition, RotationRecord};
use sui_id_store::{Database, StoreError};

#[path = "rotate/preconditions.rs"]
pub mod preconditions;

#[path = "rotate/journal.rs"]
pub mod journal;

#[path = "rotate/recovery.rs"]
pub mod recovery;

#[path = "rotate/backup.rs"]
pub mod backup;

/// [`rotate`]'s success: K08's own per-table counts, alongside the
/// rotation id it minted -- the caller prints both.
pub struct RotateOutcome {
    pub rotation_id: String,
    pub report: sui_id_store::commands::RotationReport,
}

/// [`rotate`]'s error. [`NotDurable`](Self::NotDurable) keeps K08's or
/// K05's own `StoreError::RotationNotDurable` typed and names which
/// phase hit it, so the caller can give an accurate, phase-specific
/// refusal; everything else is an ordinary [`anyhow::Error`].
#[derive(Debug)]
pub enum RotateError {
    NotDurable {
        phase: &'static str,
        source: StoreError,
    },
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RotateError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

/// Write `body_b64` to a fresh temporary at `path` (`create_new`, `0600`,
/// checked write, `sync_file`) -- the shape every rotation-written key
/// file shares, `next_temp` and `backup_temp` alike.
fn write_key_temp(
    adapter: &dyn KeyFileOps,
    point_prefix: &str,
    path: &Path,
    body_b64: &str,
) -> anyhow::Result<()> {
    let mut data = body_b64.as_bytes().to_vec();
    data.push(b'\n');
    adapter
        .create_new(&format!("{point_prefix}.create_new"), path)
        .with_context(|| format!("creating {}", path.display()))?;
    adapter
        .write_all(&format!("{point_prefix}.write"), path, &data)
        .with_context(|| format!("writing {}", path.display()))?;
    adapter
        .sync_file(&format!("{point_prefix}.sync_file"), path)
        .with_context(|| format!("syncing {}", path.display()))
}

/// Read `path` back, decode it, and check its fingerprint -- RFC 100
/// `0.94.0` P2 §3 point 5/6's own "read it back, decode it, and check
/// its fingerprint" step, shared by the new key and the backup.
fn verify_key_temp(
    adapter: &dyn KeyFileOps,
    point: &str,
    path: &Path,
    expected_fingerprint: &str,
) -> anyhow::Result<()> {
    let bytes = adapter
        .read(point, path)
        .with_context(|| format!("reading {} back", path.display()))?;
    let s = String::from_utf8(bytes)
        .with_context(|| format!("{} is not valid UTF-8", path.display()))?;
    let key = MasterKey::from_base64(s.trim())
        .with_context(|| format!("{} is malformed", path.display()))?;
    let fingerprint = sui_id_store::crypto::fingerprint(&key);
    if fingerprint != expected_fingerprint {
        anyhow::bail!(
            "refusing to publish {}: it decodes to fingerprint {fingerprint}, not the \
             expected {expected_fingerprint}",
            path.display()
        );
    }
    Ok(())
}

/// Publish `from` (a verified temporary, inside the workspace) to `to`
/// (its sibling beside the key file), no-replace, then sync **both**
/// directories -- the workspace and the key file's own -- since a
/// cross-directory rename is durable on POSIX only when both sides are
/// synced (stage 3's own carried item).
fn publish_and_sync_both(
    adapter: &dyn KeyFileOps,
    point_prefix: &str,
    from: &Path,
    to: &Path,
) -> anyhow::Result<()> {
    adapter
        .publish_no_replace(&format!("{point_prefix}.publish"), from, to)
        .with_context(|| format!("publishing {}", to.display()))?;
    crate::keyring::sync_parent_dir_impl(
        adapter,
        &format!("{point_prefix}.sync_dir.workspace"),
        from,
    )
    .context("syncing the workspace after publication")?;
    crate::keyring::sync_parent_dir_impl(adapter, &format!("{point_prefix}.sync_dir.active"), to)
        .context("syncing the key file's directory after publication")
}

/// RFC 100 `0.94.0` P2 §3 point 4: every field [`journal::write_intent`]
/// needs, bound to this exact deployment -- [`JournalState::Intent`].
fn build_journal_body(
    paths: &RotationPaths,
    rotation_id: &str,
    old_key: &MasterKey,
    new_key: &MasterKey,
    disposition: RotationDisposition,
) -> anyhow::Result<journal::JournalBody> {
    Ok(journal::JournalBody {
        rotation_id: rotation_id.to_owned(),
        old_fingerprint: sui_id_store::crypto::fingerprint(old_key),
        new_fingerprint: sui_id_store::crypto::fingerprint(new_key),
        active_path: journal::path_to_string(&paths.active)?,
        workspace_path: journal::path_to_string(&paths.workspace)?,
        next_path: journal::path_to_string(&paths.next)?,
        backup_path: journal::path_to_string(&paths.backup)?,
        intent_temp_path: journal::path_to_string(&paths.intent_temp)?,
        update_temp_path: journal::path_to_string(&paths.update_temp)?,
        next_temp_path: journal::path_to_string(&paths.next_temp)?,
        backup_temp_path: journal::path_to_string(&paths.backup_temp)?,
        expected_uid: rustix::process::geteuid().as_raw(),
        expected_mode: 0o600,
        backup_disposition: disposition,
        state: JournalState::Intent,
    })
}

/// RFC 100 `0.94.0` P2 §3: the sequence, as the dispatch orders it --
/// everything from the workspace (point 3) through the clean (point
/// 11). The lock (point 1) and the precondition check (point 2) are the
/// caller's own job (`cli::handlers::run_admin_rotate_key`), run before
/// this is ever called, and so is releasing the lock and printing
/// success (point 12) afterward -- this function's own return is
/// exactly "K05 is durable", the one fact that gate depends on.
///
/// `db` is already open under `old_key`, with its sentinel written.
/// `rotate_master_key` -- plumbing through `database::open` for the
/// **second** handle this needs, opened under `new_key` after
/// activation, the same policy as the first.
#[allow(clippy::too_many_arguments)]
pub async fn rotate(
    adapter: &dyn KeyFileOps,
    db: &Database,
    db_path: &Path,
    key_file: &Path,
    old_key: &MasterKey,
    new_key: &MasterKey,
    new_key_b64: &str,
    disposition: RotationDisposition,
    backup: crate::database::PreMigrationBackup,
) -> Result<RotateOutcome, RotateError> {
    let paths = RotationPaths::for_key_file(key_file);
    let rotation_id = uuid::Uuid::new_v4().to_string();

    // Point 3: the workspace.
    journal::lock_workspace(adapter, &paths.workspace)?;

    // Point 4: Intent.
    let body = build_journal_body(&paths, &rotation_id, old_key, new_key, disposition.clone())?;
    let mut envelope = journal::write_intent(adapter, &paths.workspace, body, old_key, new_key)?;

    // Point 5: the new key.
    write_key_temp(adapter, "rotate.next", &paths.next_temp, new_key_b64)?;
    verify_key_temp(
        adapter,
        "rotate.next.readback",
        &paths.next_temp,
        &sui_id_store::crypto::fingerprint(new_key),
    )?;
    envelope = journal::advance(
        adapter,
        &paths.workspace,
        &envelope,
        JournalState::NextTempVerified,
        old_key,
        new_key,
    )?;
    publish_and_sync_both(adapter, "rotate.next", &paths.next_temp, &paths.next)?;
    envelope = journal::advance(
        adapter,
        &paths.workspace,
        &envelope,
        JournalState::NextPublished,
        old_key,
        new_key,
    )?;

    // Point 6: the backup, from the active (old) key's own bytes --
    // its fingerprint must be the old key's, never the new one's.
    let old_key_b64 = old_key.to_base64();
    write_key_temp(adapter, "rotate.backup", &paths.backup_temp, &old_key_b64)?;
    verify_key_temp(
        adapter,
        "rotate.backup.readback",
        &paths.backup_temp,
        &sui_id_store::crypto::fingerprint(old_key),
    )?;
    envelope = journal::advance(
        adapter,
        &paths.workspace,
        &envelope,
        JournalState::BackupTempVerified,
        old_key,
        new_key,
    )?;
    publish_and_sync_both(adapter, "rotate.backup", &paths.backup_temp, &paths.backup)?;
    envelope = journal::advance(
        adapter,
        &paths.workspace,
        &envelope,
        JournalState::BackupPublished,
        old_key,
        new_key,
    )?;

    // Point 7: Prepared.
    journal::advance(
        adapter,
        &paths.workspace,
        &envelope,
        JournalState::Prepared,
        old_key,
        new_key,
    )?;

    // Point 8: K08 -- the reseal and the pending row, one transaction,
    // durable before this returns (K08's own contract). The two
    // markers bracket it exactly as the retired `rotate_key_files` did
    // for K07: a no-op outside `Simulated`, where they are this
    // sequence's own crash points for "committed" and "durable."
    let backup_filename = paths
        .backup
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("key.pre-rotation")
        .to_owned();
    let record = RotationRecord {
        rotation_id: rotation_id.clone(),
        old_fingerprint: sui_id_store::crypto::fingerprint(old_key),
        backup_filename,
        disposition,
    };
    let report =
        match sui_id_store::commands::reseal_master_key_for_rotation(db, new_key, record).await {
            Err(e @ StoreError::RotationNotDurable) => {
                return Err(RotateError::NotDurable {
                    phase: "K08 (the database re-seal)",
                    source: e,
                });
            }
            r => r
                .map_err(anyhow::Error::new)
                .context("resealing under the new key")?
                .into_inner(),
        };
    adapter.db_committed("rotate.reseal");
    adapter.db_durable("rotate.reseal");

    // Point 9: activation -- shared with recovery's own row 8, so the
    // two primitives can never drift apart.
    recovery::activate(adapter, &paths, "rotate.activate")?;

    // Point 10: K05, with the database re-opened under the now-active
    // new key (K05's own contract: it needs an open database already
    // keyed with what it is marking complete).
    let db_new = crate::database::open(db_path, dup_key(new_key), backup)
        .context("opening the database under the new key to complete the rotation")?;
    match sui_id_store::commands::complete_master_key_rotation(&db_new, rotation_id.clone()).await {
        Err(e @ StoreError::RotationNotDurable) => {
            return Err(RotateError::NotDurable {
                phase: "K05 (recording completion)",
                source: e,
            });
        }
        Err(e) => {
            return Err(anyhow::Error::new(e)
                .context("marking the rotation complete")
                .into());
        }
        Ok(_) => {}
    }
    adapter.db_committed("rotate.rotated");
    adapter.db_durable("rotate.rotated");

    // Point 11: the clean -- the backup stays, under its disposition,
    // until P3.
    recovery::clean_workspace(adapter, &paths, false)
        .context("cleaning the rotation workspace after a successful rotation")?;

    Ok(RotateOutcome {
        rotation_id,
        report,
    })
}

fn dup_key(key: &MasterKey) -> MasterKey {
    use zeroize::Zeroize;
    let mut bytes = *key.as_bytes();
    let copy = MasterKey::from_bytes(bytes);
    bytes.zeroize();
    copy
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "rotate/tests.rs"]
mod tests;
