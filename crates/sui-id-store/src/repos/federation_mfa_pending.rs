//! RFC 096-B1 stage 9: F02, the first writer of `federation_mfa_pending`
//! (migration 0047). RFC 096 `:719-722`: an existing, already-linked
//! federated identity whose local account requires MFA enters this row
//! rather than a session.
//!
//! ## AAD binding
//!
//! `continuation_sealed` is XChaCha20-Poly1305 sealed with AAD binding
//! `id`, `provider_id`, `provider_config_version`,
//! `provider_activation_generation` and `user_id` — the same
//! reconstruct-from-the-row convention `federation_login_attempt`'s own
//! `pkce_verifier_sealed` uses (migration 0046's comment).
//!
//! **What the plaintext is, and why it is empty.** `:721-722` lists what
//! the challenge must preserve: `primary_method=Fed`, provider/link
//! identity, next target, creation/expiry, and one-time consumption.
//! Migration 0047's own table gives every one of those its own plain
//! column already — `primary_method`, the provider triple and `user_id`,
//! `next_path`, `created_at`/`expires_at`, and `status`. There is nothing
//! left for a sealed payload to *store*. What it still buys, with zero
//! plaintext bytes, is answer 2 of the dispatch's three: a promotion (stage
//! 10, F03) must *open* this column with the row's own current values as
//! AAD before it may proceed, so a row edited in place out of band —
//! `user_id` repointed to a different victim by a direct `UPDATE` that
//! bypasses this crate's own write paths, for instance — fails to open and
//! cannot be promoted. That is a tamper check on the row's own identity,
//! not storage, and an empty plaintext says so as plainly as a non-empty
//! one would: XChaCha20-Poly1305 authenticates zero-length plaintext the
//! same way it authenticates any other length, and `tests.rs` proves it
//! fails to open after mutating any one of the five bound columns.
//!
//! ## Why two binding digests, not one written twice
//!
//! `browser_binding_sha256` and `csrf_binding_sha256` are both 32-byte
//! digests of a cookie-held opaque value, both compared the same way
//! (constant-time, at consumption — stage 10). That similarity is not
//! redundancy: the two values reach a consuming request through
//! different channels, and the channel is what each one is actually
//! checking.
//!
//! `browser_binding_sha256`'s raw value arrives **automatically**, as an
//! ordinary cookie the browser attaches to every request in its jar —
//! what it proves is "the same user agent that received this cookie is
//! presenting it now," independent of what page or form sent the
//! request. `csrf_binding_sha256`'s raw value must instead arrive **in
//! the request body**: `HttpOnly` keeps script from reading its cookie,
//! so the only way the value reaches a submission is a hidden form field
//! `/admin/login/mfa` renders server-side from the cookie it already
//! holds — which a cross-site POST cannot reproduce even if the cookie
//! itself rides along automatically the way the browser binding's does.
//! Collapsing the two into one column would keep the first property and
//! lose the second.
//!
//! ## Clock
//!
//! Same convention as `federation_login_attempt::insert`:
//! [`insert_within_tx`] takes an already-sealed row, built from one
//! `DateTime<Utc>` reading by its caller (`commands::issue_federated_mfa_pending`),
//! not two separate `now()` calls — the schema's own `expires_at =
//! created_at + 300s` `CHECK` would refuse a row straddling a clock tick
//! between two reads.

use crate::errors::{StoreError, StoreResult};
use crate::models::FederationMfaPendingRow;
use crate::registry::{
    ActorRequirement, AttributeSpec, AuditEventKind, MustAttemptDescriptor, TargetRequirement,
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use sui_id_shared::ids::{FederationMfaPendingId, FederationProviderId, UserId};

const SELECT_COLUMNS: &str = "id, provider_id, provider_config_version, \
    provider_activation_generation, user_id, continuation_sealed, next_path, \
    browser_binding_sha256, csrf_binding_sha256, failure_count, status, created_at, expires_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationMfaPendingRow> {
    let id_str: String = row.get(0)?;
    let provider_id_str: String = row.get(1)?;
    let user_id_str: String = row.get(4)?;
    let browser_binding_sha256: Vec<u8> = row.get(7)?;
    let csrf_binding_sha256: Vec<u8> = row.get(8)?;
    let status_str: String = row.get(10)?;
    let fixed = |v: Vec<u8>, col: usize| -> rusqlite::Result<[u8; 32]> {
        v.try_into().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                col,
                rusqlite::types::Type::Blob,
                "not 32 bytes".into(),
            )
        })
    };
    Ok(FederationMfaPendingRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_id: provider_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_config_version: row.get(2)?,
        provider_activation_generation: row.get(3)?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?,
        continuation_sealed: row.get(5)?,
        next_path: row.get(6)?,
        browser_binding_sha256: fixed(browser_binding_sha256, 7)?,
        csrf_binding_sha256: fixed(csrf_binding_sha256, 8)?,
        failure_count: row.get(9)?,
        status: status_str.parse().map_err(|_: StoreError| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                "unknown federation_mfa_pending status".into(),
            )
        })?,
        created_at: row.get(11)?,
        expires_at: row.get(12)?,
    })
}

/// RFC 096 `:732-733`: 300 seconds from `created_at`.
pub const PENDING_LIFETIME_SECS: i64 = 300;

/// The AAD bound into `continuation_sealed` (module doc). NUL-separated,
/// the same safety argument as `federation_login_attempt::build_aad`: a
/// UUID string and a decimal integer can each contain no NUL byte, so no
/// two distinct `(id, provider_id, version, generation, user_id)` tuples
/// collide on the same byte string.
pub fn build_aad(
    id: FederationMfaPendingId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: UserId,
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_config_version.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_activation_generation.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(user_id.to_string().as_bytes());
    aad
}

/// Insert exactly one `pending` `federation_mfa_pending` row, on the
/// caller's own transaction. `status` and `failure_count` are named
/// explicitly now (stage 10a: `models::FederationMfaPendingRow`'s own
/// doc explains why) — always `'pending'`/`0`, the only state this
/// function ever constructs a row in. `primary_method` is still left to
/// the table's own `DEFAULT` ('fed'): nothing reads it back yet, and the
/// table's own `CHECK` pins it to that one value regardless.
///
/// A `FOREIGN KEY (user_id, provider_id) REFERENCES federation_link`
/// violation here (migration 0047's fix round) is this stage's "active
/// link" recheck, atomic with the insert itself — not a separate `SELECT`
/// this write could race past. Reported as [`StoreError::Conflict`], the
/// same variant every other constraint-violation recheck in this crate's
/// federation tables already uses.
pub fn insert_within_tx(
    conn: &rusqlite::Connection,
    row: &FederationMfaPendingRow,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO federation_mfa_pending( \
            id, provider_id, provider_config_version, provider_activation_generation, user_id, \
            continuation_sealed, next_path, browser_binding_sha256, csrf_binding_sha256, \
            failure_count, status, created_at, expires_at \
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            row.id.to_string(),
            row.provider_id.to_string(),
            row.provider_config_version,
            row.provider_activation_generation,
            row.user_id.to_string(),
            row.continuation_sealed,
            row.next_path,
            row.browser_binding_sha256.as_slice(),
            row.csrf_binding_sha256.as_slice(),
            row.failure_count,
            row.status.as_str(),
            row.created_at,
            row.expires_at,
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Conflict
        }
        other => StoreError::from(other),
    })?;
    Ok(())
}

/// RFC 096-B1 stage 10a: the first read of a `federation_mfa_pending` row
/// by its own id — the `FederatedMfaVerifier`'s binding gate needs the
/// full row (bindings, status, failure_count, expiry) to validate a
/// submission against. Returns `Ok(None)` for an unknown id rather than
/// an error: "no such row" and "a row that fails every other check" must
/// be indistinguishable to the verifier's own caller, and folding a
/// missing row into the same `Option::None` the verifier already has to
/// handle for other refusals is what makes that natural rather than
/// another case to remember.
pub async fn find_by_id(
    db: &crate::Database,
    id: FederationMfaPendingId,
) -> StoreResult<Option<FederationMfaPendingRow>> {
    let id_str = id.to_string();
    db.with_read(move |read| {
        match read
            .prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM federation_mfa_pending WHERE id = ?1"
            ))?
            .query_row([id_str], map_row)
        {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::from(e)),
        }
    })
    .await
}

/// RFC 096 `:734`: terminal rows are purged after 24 hours. **Not filtered
/// on `status`**, the same choice `federation_login_attempt::purge_expired`
/// already made and for the same reason restated for this table rather
/// than assumed to carry over: `:734`'s sentence is about terminal rows,
/// but a row abandoned mid-challenge (a user who closes the tab at the
/// TOTP prompt) is never terminal either, and `expires_at` here is
/// `created_at + 300s` — a 24-hour cutoff past that is generous for an
/// abandoned row too, so one `DELETE ... WHERE expires_at < ?1` reaches
/// both kinds the same way `federation_login_attempt`'s own purge reaches
/// `pending`/`exchanging` rows this old.
///
/// `federation_webauthn_ceremony` needs no purge of its own: its composite
/// `FOREIGN KEY ... ON DELETE CASCADE` (migration 0047's fix round) means
/// this `DELETE` collects any ceremony whose parent it removes, and the
/// same `FOREIGN KEY` makes a ceremony with no live parent unrepresentable
/// in the first place (`tests_rfc096_b1_stage8.rs`'s own
/// `a_ceremony_without_a_parent_pending_row_cannot_be_stored` already
/// proves the second half; this module's own tests prove the first).
pub async fn purge_expired(db: &crate::Database, now: DateTime<Utc>) -> StoreResult<usize> {
    let cutoff = now - Duration::hours(24);
    db.with_conn(move |conn| {
        let n = conn.execute(
            "DELETE FROM federation_mfa_pending WHERE expires_at < ?1",
            [cutoff],
        )?;
        Ok(n)
    })
    .await
}

/// RFC 096-B1 stage 9: the registry-declared Class-B observation that a
/// federated sign-in stopped at the local second factor. Replaces the
/// legacy, factually wrong `auth.login.password_ok_mfa_required` append
/// this branch used to make (no password was presented; for an F04-
/// provisioned user there is no password at all — RFC 096 `:723-724`
/// "never substitutes `Pwd`"). `actor`/`target` are both `Required`: unlike
/// `federation_provider.rs`'s own must-attempt descriptors (P2/F05, fired
/// before or without a resolved local account), this event only fires for
/// an existing, already-linked user — `user_id` is always known.
pub static AUDIT_MFA_REQUIRED_DESCRIPTOR: MustAttemptDescriptor = MustAttemptDescriptor {
    kind: AuditEventKind::AuthFederationMfaRequired,
    name: "auth.federation.mfa_required",
    actor: ActorRequirement::Required,
    target: TargetRequirement::Required,
    attributes: &[AttributeSpec {
        name: "provider_id",
        description: "internal id of the federation provider this sign-in was for",
        required: false,
    }],
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
