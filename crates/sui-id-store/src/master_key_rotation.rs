//! RFC 100 `0.94.0` P1 (database half): the `master_key_rotation` row's
//! full shape (migration 0053), read and write primitives used by K08
//! (`commands::reseal_master_key_for_rotation`) and K05
//! (`commands::complete_master_key_rotation`). Built here as functions
//! the live rotation did not call yet; wired in by P2.
//!
//! `review_at` (RFC 100's *Old-key custody*) is typed here, not free
//! text: [`RotationDisposition::RetainUntil`]'s field is
//! `chrono::DateTime<Utc>`, so a caller cannot construct one without an
//! already-parsed, valid RFC 3339 instant. The database column is
//! still `TEXT`, the same as every other `_at` column in this schema —
//! `rusqlite`'s `chrono` support reads and writes it losslessly.

use crate::crypto::MasterKey;
use crate::errors::{RotationRefusal, StoreError, StoreResult};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row};

/// The operator's bound choice for the pre-rotation backup of the old
/// key (RFC 100, *Old-key custody*). RFC 100 `0.94.0` P2 §3 point 4
/// takes this type into the journal body itself (`rotate::journal::
/// JournalBody::backup_disposition`), replacing that module's own,
/// `review_at: String` copy — one typed disposition, not two shapes of
/// the same fact. `Serialize`/`Deserialize` (`deny_unknown_fields`,
/// tagged on `kind`) make it `journal::canonical_body_bytes`-safe:
/// `DateTime<Utc>`'s own `serde` support (chrono's `serde` feature,
/// enabled workspace-wide) writes an RFC 3339 string, deterministically.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum RotationDisposition {
    RetainUntil {
        review_at: DateTime<Utc>,
        custody_reference: String,
    },
    RemoveAfterVerified {
        verification_reference: String,
    },
}

/// `master_key_rotation.state`, forward-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationState {
    DbCommittedPendingActivation,
    Complete,
}

impl RotationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::DbCommittedPendingActivation => "db_committed_pending_activation",
            Self::Complete => "complete",
        }
    }

    fn parse(s: &str) -> StoreResult<Self> {
        match s {
            "db_committed_pending_activation" => Ok(Self::DbCommittedPendingActivation),
            "complete" => Ok(Self::Complete),
            other => Err(StoreError::InvalidData(format!(
                "master_key_rotation.state holds an unrecognised value: {other:?}"
            ))),
        }
    }
}

/// The row, as read. `conn` may be a read-only preview connection
/// (recovery's own use, before the database is opened for normal use)
/// — nothing here writes.
#[derive(Debug, Clone, PartialEq)]
pub struct MasterKeyRotationRow {
    pub rotation_id: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub state: RotationState,
    pub prepared_at: DateTime<Utc>,
    pub committed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub backup_filename: String,
    pub disposition: RotationDisposition,
    /// RFC 100 `0.94.0` P3: set once, by K09, after it has unlinked the
    /// pre-rotation backup file. `None` while the backup is still (or
    /// was never) kept.
    pub backup_removed_at: Option<DateTime<Utc>>,
}

/// What K08 needs from its caller to write the row — everything
/// except `new_fingerprint`, which K08 derives itself from the
/// `new_key` it already takes as a parameter, never from a second,
/// redundant field a caller could let drift from the key actually
/// used.
#[derive(Debug, Clone, PartialEq)]
pub struct RotationRecord {
    pub rotation_id: String,
    /// Checked against the database's current key before anything is
    /// written (K08's own refusal, RFC 100 `0.94.0` P1 §2).
    pub old_fingerprint: String,
    pub backup_filename: String,
    pub disposition: RotationDisposition,
}

fn disposition_from_row(row: &Row<'_>) -> rusqlite::Result<RotationDisposition> {
    let kind: String = row.get("disposition_kind")?;
    match kind.as_str() {
        "retain_until" => {
            let review_at: DateTime<Utc> = row.get("review_at")?;
            let custody_reference: String = row.get("custody_reference")?;
            Ok(RotationDisposition::RetainUntil {
                review_at,
                custody_reference,
            })
        }
        "remove_after_verified" => {
            let verification_reference: String = row.get("verification_reference")?;
            Ok(RotationDisposition::RemoveAfterVerified {
                verification_reference,
            })
        }
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("master_key_rotation.disposition_kind holds an unrecognised value: {other:?}")
                .into(),
        )),
    }
}

/// Read the singleton row, if one exists. `conn` may be a read-only
/// connection — recovery's own entry point, before the database is
/// opened normally.
pub fn read_within_tx(conn: &Connection) -> StoreResult<Option<MasterKeyRotationRow>> {
    conn.query_row(
        "SELECT rotation_id, old_fingerprint, new_fingerprint, state, prepared_at, \
                committed_at, completed_at, backup_filename, disposition_kind, review_at, \
                custody_reference, verification_reference, backup_removed_at \
         FROM master_key_rotation WHERE id = 1",
        [],
        |row| {
            let state_str: String = row.get("state")?;
            Ok(MasterKeyRotationRow {
                rotation_id: row.get("rotation_id")?,
                old_fingerprint: row.get("old_fingerprint")?,
                new_fingerprint: row.get("new_fingerprint")?,
                state: RotationState::parse(&state_str)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                prepared_at: row.get("prepared_at")?,
                committed_at: row.get("committed_at")?,
                completed_at: row.get("completed_at")?,
                backup_filename: row.get("backup_filename")?,
                disposition: disposition_from_row(row)?,
                backup_removed_at: row.get("backup_removed_at")?,
            })
        },
    )
    .optional()
    .map_err(StoreError::from)
}

/// [`read_within_tx`], through an already-open [`crate::Database`] --
/// the one path a caller outside this crate (RFC 100 `0.94.0` P3's own
/// `sui-id`-crate orchestration) may use to read the row, since
/// `Database::with_conn`/`with_tx` are themselves restricted to the
/// allow-listed module set in `contracts/write-authority.toml`, and
/// this file is on it.
pub async fn read(db: &crate::Database) -> StoreResult<Option<MasterKeyRotationRow>> {
    db.with_conn(read_within_tx).await
}

/// K08's own write: refuse if a row is already pending, or if
/// `record.old_fingerprint` does not match `old_key`'s real
/// fingerprint; otherwise insert (replacing a `complete` row, the
/// singleton's own rule — RFC 100 `0.94.0` P1 §1: "a second rotation
/// replaces a `complete` row, and is refused while one is pending").
/// Caller's own transaction; writes nothing on refusal.
pub(crate) fn insert_pending_within_tx(
    conn: &Connection,
    record: &RotationRecord,
    old_key: &MasterKey,
    new_key: &MasterKey,
    now: DateTime<Utc>,
) -> StoreResult<()> {
    if let Some(existing) = read_within_tx(conn)?
        && existing.state == RotationState::DbCommittedPendingActivation
    {
        return Err(StoreError::RotationRefused(RotationRefusal::AlreadyPending));
    }
    let old_fingerprint = crate::crypto::fingerprint(old_key);
    if record.old_fingerprint != old_fingerprint {
        return Err(StoreError::RotationRefused(
            RotationRefusal::OldFingerprintMismatch,
        ));
    }
    let new_fingerprint = crate::crypto::fingerprint(new_key);
    let (disposition_kind, review_at, custody_reference, verification_reference): (
        &str,
        Option<DateTime<Utc>>,
        Option<&str>,
        Option<&str>,
    ) = match &record.disposition {
        RotationDisposition::RetainUntil {
            review_at,
            custody_reference,
        } => (
            "retain_until",
            Some(*review_at),
            Some(custody_reference.as_str()),
            None,
        ),
        RotationDisposition::RemoveAfterVerified {
            verification_reference,
        } => (
            "remove_after_verified",
            None,
            None,
            Some(verification_reference.as_str()),
        ),
    };
    conn.execute(
        "INSERT INTO master_key_rotation \
         (id, rotation_id, old_fingerprint, new_fingerprint, state, prepared_at, committed_at, \
          completed_at, backup_filename, disposition_kind, review_at, custody_reference, \
          verification_reference, backup_removed_at) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?5, NULL, ?6, ?7, ?8, ?9, ?10, NULL) \
         ON CONFLICT(id) DO UPDATE SET \
           rotation_id = excluded.rotation_id, \
           old_fingerprint = excluded.old_fingerprint, \
           new_fingerprint = excluded.new_fingerprint, \
           state = excluded.state, \
           prepared_at = excluded.prepared_at, \
           committed_at = excluded.committed_at, \
           completed_at = excluded.completed_at, \
           backup_filename = excluded.backup_filename, \
           disposition_kind = excluded.disposition_kind, \
           review_at = excluded.review_at, \
           custody_reference = excluded.custody_reference, \
           verification_reference = excluded.verification_reference, \
           backup_removed_at = excluded.backup_removed_at",
        rusqlite::params![
            record.rotation_id,
            old_fingerprint,
            new_fingerprint,
            RotationState::DbCommittedPendingActivation.as_str(),
            now,
            record.backup_filename,
            disposition_kind,
            review_at,
            custody_reference,
            verification_reference,
        ],
    )?;
    Ok(())
}

/// K05's own write: mark the singleton row `complete`, refusing if
/// there is none, or if it names a different `rotation_id`. Idempotent
/// — a row already `complete` with this `rotation_id` succeeds without
/// changing `completed_at` again, so the caller's own idempotence (no
/// duplicate event) has a true fact to key on: whether this call wrote
/// anything.
pub(crate) fn mark_complete_within_tx(
    conn: &Connection,
    rotation_id: &str,
    now: DateTime<Utc>,
) -> StoreResult<bool> {
    let Some(existing) = read_within_tx(conn)? else {
        return Err(StoreError::RotationRefused(RotationRefusal::NoRow));
    };
    if existing.rotation_id != rotation_id {
        return Err(StoreError::RotationRefused(
            RotationRefusal::RotationIdMismatch,
        ));
    }
    if existing.state == RotationState::Complete {
        return Ok(false);
    }
    conn.execute(
        "UPDATE master_key_rotation SET state = ?1, completed_at = ?2 WHERE id = 1",
        rusqlite::params![RotationState::Complete.as_str(), now],
    )?;
    Ok(true)
}

/// RFC 100 `0.94.0` P3 §2: which `verification_reference` removing
/// `row`'s backup will record -- the row's own, for `RemoveAfterVerified`
/// (nothing else to reuse), or the operator's `--verification-reference`
/// for `RetainUntil` (that disposition carries none of its own).
/// Pure, and called by the orchestrating caller (`sui-id`'s own
/// `rotate::backup::remove`) *before* it touches the backup file at
/// all, so a missing override refuses before anything is unlinked.
pub fn resolve_removal_verification_reference(
    row: &MasterKeyRotationRow,
    override_given: Option<&str>,
) -> StoreResult<String> {
    match &row.disposition {
        RotationDisposition::RemoveAfterVerified {
            verification_reference,
        } => Ok(verification_reference.clone()),
        RotationDisposition::RetainUntil { .. } => {
            override_given
                .map(str::to_owned)
                .ok_or(StoreError::RotationRefused(
                    RotationRefusal::VerificationReferenceRequired,
                ))
        }
    }
}

/// K09's own return: what the caller (`commands::remove_master_key_backup`)
/// needs to build `admin.master_key.backup_removed`'s attributes.
pub(crate) struct BackupRemoval {
    pub rotation_id: String,
}

/// K09's own write (RFC 100 `0.94.0` P3 §2): record that the pre-rotation
/// backup file has been unlinked. Called *after* the file is actually
/// gone (the dispatch's own order: verify, unlink, sync, then the row
/// records it) — this function never touches a file itself.
/// `verification_reference` is already resolved by
/// [`resolve_removal_verification_reference`]; this just records it.
pub(crate) fn mark_backup_removed_within_tx(
    conn: &Connection,
    now: DateTime<Utc>,
) -> StoreResult<BackupRemoval> {
    let Some(existing) = read_within_tx(conn)? else {
        return Err(StoreError::RotationRefused(RotationRefusal::NoRow));
    };
    conn.execute(
        "UPDATE master_key_rotation SET backup_removed_at = ?1 WHERE id = 1",
        rusqlite::params![now],
    )?;
    Ok(BackupRemoval {
        rotation_id: existing.rotation_id,
    })
}

/// K10's own return: what `commands::renew_master_key_backup` needs to
/// build `admin.master_key.backup_renewed`'s attributes.
pub(crate) struct BackupRenewal {
    pub rotation_id: String,
}

/// K10's own write (RFC 100 `0.94.0` P3 §2): move a `RetainUntil`
/// backup's `review_at` forward and record a new `custody_reference`.
/// Refuses a row whose disposition is `RemoveAfterVerified` (nothing to
/// renew) or a `new_review_at` that is not after `now`.
pub(crate) fn renew_backup_within_tx(
    conn: &Connection,
    new_review_at: DateTime<Utc>,
    new_custody_reference: &str,
    now: DateTime<Utc>,
) -> StoreResult<BackupRenewal> {
    let Some(existing) = read_within_tx(conn)? else {
        return Err(StoreError::RotationRefused(RotationRefusal::NoRow));
    };
    match existing.disposition {
        RotationDisposition::RetainUntil { .. } => {}
        RotationDisposition::RemoveAfterVerified { .. } => {
            return Err(StoreError::RotationRefused(
                RotationRefusal::RenewalRequiresRetainUntil,
            ));
        }
    }
    if new_review_at <= now {
        return Err(StoreError::RotationRefused(
            RotationRefusal::RenewalNotInFuture,
        ));
    }
    conn.execute(
        "UPDATE master_key_rotation SET review_at = ?1, custody_reference = ?2 WHERE id = 1",
        rusqlite::params![new_review_at, new_custody_reference],
    )?;
    Ok(BackupRenewal {
        rotation_id: existing.rotation_id,
    })
}

/// RFC 100 `0.94.0` P3 §3: `Some((review_at, custody_reference))`
/// exactly when a kept `RetainUntil` backup's own `review_at` has
/// passed and it has not already been removed -- never for
/// `RemoveAfterVerified`, and never once `backup_removed_at` is set.
/// Pure, with `now` supplied by the caller, so a server start (its own
/// `SharedClock`) and a one-off CLI run (`Utc::now()`) share one rule,
/// and a test can inject either.
pub fn overdue_backup_review(
    row: &MasterKeyRotationRow,
    now: DateTime<Utc>,
) -> Option<(DateTime<Utc>, &str)> {
    if row.backup_removed_at.is_some() {
        return None;
    }
    match &row.disposition {
        RotationDisposition::RetainUntil {
            review_at,
            custody_reference,
        } if *review_at <= now => Some((*review_at, custody_reference.as_str())),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests;
