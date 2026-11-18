//! RFC 100 stage 1: the startup key check.
//!
//! A sentinel — a small fixed plaintext, sealed under the master key — is
//! checked before readiness on every start and by every CLI command that
//! opens the database. A key that does not open it refuses, naming
//! fingerprints, never key bytes. The key is generated only for a
//! database that has never been sealed; with a sentinel present, or with
//! sealed rows and no sentinel yet (an upgrade), a missing key file fails
//! closed, never silently replaced by a freshly generated one.
//!
//! This module holds the two facts this decides over, the pure decision
//! itself, and the sentinel's own seal/open. The `sui-id` binary crate
//! (`runtime::keyring`, `runtime::database`) owns reading the key file
//! and formatting the refusal; this crate owns the database side and the
//! crypto, neither of which it is.

use crate::crypto::{self, MasterKey};
use crate::errors::StoreResult;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

/// The sentinel's fixed plaintext (RFC 100 §3) and the AAD it is sealed
/// under. Never anything but this one value — the sentinel identifies
/// the key that sealed it, not any other fact.
const SENTINEL_PLAINTEXT: &[u8] = b"sui-id master key sentinel v1";
const SENTINEL_AAD: &[u8] = b"sui-id/master-key-sentinel/v1";

/// Seal the sentinel under `key`.
pub fn seal_sentinel(key: &MasterKey) -> StoreResult<Vec<u8>> {
    crypto::seal(key, SENTINEL_PLAINTEXT, SENTINEL_AAD)
}

/// `true` iff `key` opens `sealed` and the plaintext is exactly the
/// sentinel's own fixed value — AEAD authentication already guarantees
/// the plaintext is unmodified, so the equality check is only ever
/// reached having already proven that; it exists so this function's
/// contract does not quietly depend on nothing else ever sharing
/// [`SENTINEL_AAD`].
pub fn sentinel_opens(key: &MasterKey, sealed: &[u8]) -> bool {
    crypto::open(key, sealed, SENTINEL_AAD)
        .map(|plain| plain == SENTINEL_PLAINTEXT)
        .unwrap_or(false)
}

/// The `master_key_sentinel` row, read on a read-only connection with no
/// decryption — `key_fingerprint` is stored in the clear precisely so a
/// refusal can name it without opening anything.
#[derive(Debug, Clone)]
pub struct SentinelRow {
    pub sealed: Vec<u8>,
    pub key_fingerprint: String,
    pub written_at: DateTime<Utc>,
}

/// `true` for the one error shape this module treats as "does not exist
/// yet" rather than a real failure: a read-only preview connection, run
/// against a database from before this stage's migration (or, for the
/// six sealed tables, from before whichever migration first created
/// one), asks a table that genuinely is not there yet. Checked by
/// message text — rusqlite has no dedicated variant for it — against
/// the exact wording confirmed against the bundled SQLite (3.53.2):
/// `SqliteFailure(Error { code: Unknown, .. }, Some("no such table: …"))`.
fn is_missing_table(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(_, Some(msg)) if msg.starts_with("no such table")
    )
}

/// Read the sentinel row, if one exists. `conn` may be a read-only
/// (`open_preview_connection`) connection — this never writes. A
/// database from before migration 0052 has no `master_key_sentinel`
/// table at all yet; that is "no sentinel", not an error.
pub fn read_sentinel(conn: &Connection) -> rusqlite::Result<Option<SentinelRow>> {
    match conn
        .query_row(
            "SELECT sealed, key_fingerprint, written_at FROM master_key_sentinel WHERE id = 1",
            [],
            |r| {
                Ok(SentinelRow {
                    sealed: r.get(0)?,
                    key_fingerprint: r.get(1)?,
                    written_at: r.get(2)?,
                })
            },
        )
        .optional()
    {
        Ok(v) => Ok(v),
        Err(e) if is_missing_table(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Write the sentinel row under `key`, if none exists yet. Idempotent:
/// a database that already has one is untouched. Runs inside its own
/// transaction (RFC 100 §4 — not K07's, not any caller's); if a power
/// loss takes the write, the next start finds sealed rows (if any) and
/// no sentinel, and the upgrade rule below verifies the key and writes
/// it again, so this needs no stronger durability than an ordinary
/// commit.
pub async fn ensure_sentinel_written(db: &crate::Database, key: &MasterKey) -> StoreResult<()> {
    let sealed = seal_sentinel(key)?;
    let fingerprint = crypto::fingerprint(key);
    let now = Utc::now();
    let wrote = db
        .with_tx(move |tx| -> StoreResult<bool> {
            if read_sentinel(tx)?.is_some() {
                return Ok(false);
            }
            tx.execute(
                "INSERT INTO master_key_sentinel (id, sealed, key_fingerprint, written_at) \
                 VALUES (1, ?1, ?2, ?3)",
                rusqlite::params![sealed, fingerprint, now],
            )?;
            Ok(true)
        })
        .await?;
    if wrote {
        tracing::info!(key_fingerprint = %crypto::fingerprint(key), "master key sentinel written");
    }
    Ok(())
}

/// Reseal the sentinel from `old_key` to `new_key`, **inside K07's own
/// transaction** (RFC 100 stage 6's rule, brought forward with the
/// sentinel itself: *"K07 reseals the sentinel with every sealed
/// column, in its one Class-A transaction"*). So the sentinel always
/// names the key the data is sealed under. A crash before K07 commits
/// leaves both under the old key; after it commits, both under the new,
/// and a start with the old key file refuses, naming the new key's
/// fingerprint, rather than starting over data it cannot read.
///
/// A sentinel that does not open under `old_key` aborts the whole
/// reseal (an error rolls the transaction back): the key being rotated
/// from is not the key this database is sealed under. A database with no
/// sentinel yet (rotated before its first start under `0.93.0`) gets one,
/// under `new_key`, since that is what the data is sealed under once this
/// transaction commits.
pub fn reseal_sentinel_within_tx(
    conn: &Connection,
    old_key: &MasterKey,
    new_key: &MasterKey,
) -> StoreResult<()> {
    if let Some(row) = read_sentinel(conn)?
        && !sentinel_opens(old_key, &row.sealed)
    {
        return Err(crate::errors::StoreError::Crypto);
    }
    let sealed = seal_sentinel(new_key)?;
    conn.execute(
        "INSERT INTO master_key_sentinel (id, sealed, key_fingerprint, written_at) \
         VALUES (1, ?1, ?2, ?3) \
         ON CONFLICT(id) DO UPDATE SET \
           sealed = excluded.sealed, \
           key_fingerprint = excluded.key_fingerprint, \
           written_at = excluded.written_at",
        rusqlite::params![sealed, crypto::fingerprint(new_key), Utc::now()],
    )?;
    Ok(())
}

/// Try each of the six tables K07 reseals, in this fixed order, and
/// return the first sealed row found — its ciphertext and the AAD its
/// own table seals under. `conn` may be a read-only connection. A
/// database old enough that one of these six tables does not exist yet
/// (each shipped in its own migration, from 0001 to 0023) has no sealed
/// row in it either — treated the same as an empty table, not an error.
pub fn first_sealed_row(conn: &Connection) -> rusqlite::Result<Option<(Vec<u8>, &'static [u8])>> {
    for probe in [
        crate::repos::signing_keys::first_for_key_check,
        crate::repos::refresh_tokens::first_for_key_check,
        crate::repos::user_totp::first_for_key_check,
        crate::repos::user_webauthn_credentials::first_for_key_check,
        crate::repos::smtp_config::first_for_key_check,
        crate::repos::email_outbox::first_for_key_check,
    ] {
        match probe(conn) {
            Ok(Some(found)) => return Ok(Some(found)),
            Ok(None) => continue,
            Err(e) if is_missing_table(&e) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// What the database, read read-only, says about whether it has ever
/// been sealed. The two facts this stage's decision runs over: this,
/// and whether a key is present (not this module's concern — the
/// `sui-id` binary reads the key file).
#[derive(Debug, Clone)]
pub enum Inspection {
    /// No database file exists yet.
    NoFile,
    /// A sentinel row exists.
    Sentinel(SentinelRow),
    /// No sentinel, but at least one of the six sealed tables has a row
    /// — an upgrade from before this stage.
    SealedNoSentinel { sample: Vec<u8>, aad: &'static [u8] },
    /// No sentinel, and none of the six sealed tables has any row: this
    /// database has never been sealed.
    EmptyNoSentinel,
}

/// Read `conn` (a read-only preview connection, or any connection) and
/// decide which [`Inspection`] variant it is — never [`Inspection::NoFile`],
/// which only the caller can know (it has no connection to ask). Checks
/// the sentinel first; only if absent does it probe the six sealed
/// tables, since a database recording a sentinel never needs them read
/// to answer this question.
pub fn inspect(conn: &Connection) -> rusqlite::Result<Inspection> {
    if let Some(row) = read_sentinel(conn)? {
        return Ok(Inspection::Sentinel(row));
    }
    match first_sealed_row(conn)? {
        Some((sample, aad)) => Ok(Inspection::SealedNoSentinel { sample, aad }),
        None => Ok(Inspection::EmptyNoSentinel),
    }
}

/// The decision RFC 100 stage 1's table names, for one [`Inspection`]
/// and whether a key is present. Pure — no I/O, no crypto — so the
/// chooser itself is tested directly, not only through the actions it
/// leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCheckDecision {
    /// Use the given key as it is; the sentinel is written once the
    /// database is open and migrated.
    UseThenWriteSentinel,
    /// No key exists yet; generate one; the sentinel is written once
    /// the database is open and migrated.
    GenerateThenWriteSentinel,
    /// A key exists; it must open the sentinel, or the caller refuses.
    VerifyAgainstSentinel,
    /// A key exists; it must open the one sealed row [`Inspection::SealedNoSentinel`]
    /// carries, or the caller refuses; the sentinel is written once
    /// verified and the database is open.
    VerifyOneSealedRowThenWriteSentinel,
    /// No key, but the database records a sentinel: refuse, never
    /// generate.
    RefuseMissingKeyOverSentinel,
    /// No key, but the database holds sealed rows with no sentinel yet
    /// (an upgrade): refuse, never generate.
    RefuseMissingKeyOverSealedRows,
}

/// The pure chooser. `key_present` is whether a key was found (by file
/// or by `SUI_ID_MASTER_KEY`) — generation is never attempted before
/// this decides it is safe to.
pub fn decide(key_present: bool, inspection: &Inspection) -> KeyCheckDecision {
    use KeyCheckDecision::*;
    match (inspection, key_present) {
        (Inspection::NoFile, true) => UseThenWriteSentinel,
        (Inspection::NoFile, false) => GenerateThenWriteSentinel,
        (Inspection::Sentinel(_), true) => VerifyAgainstSentinel,
        (Inspection::Sentinel(_), false) => RefuseMissingKeyOverSentinel,
        (Inspection::SealedNoSentinel { .. }, true) => VerifyOneSealedRowThenWriteSentinel,
        (Inspection::SealedNoSentinel { .. }, false) => RefuseMissingKeyOverSealedRows,
        (Inspection::EmptyNoSentinel, true) => UseThenWriteSentinel,
        (Inspection::EmptyNoSentinel, false) => GenerateThenWriteSentinel,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests;
