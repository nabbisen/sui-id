//! RFC 096-B1 stage 11: F06, the federated WebAuthn ceremony (migration
//! 0047, RFC 096 `:756-758`). One row per in-flight challenge, at most
//! one per `federation_mfa_pending` row (the table's own `pending_id
//! TEXT NOT NULL UNIQUE`).
//!
//! ## Why this module has no `webauthn-rs` dependency
//!
//! `ceremony_state_sealed` is opaque here, the same way
//! `user_webauthn_credentials.passkey_enc` already is: `sui-id-store`
//! cannot depend on `webauthn-rs` (stage 10a's own placement argument —
//! it is `sui-id-core`'s dependency, and adding it here would be the
//! owner-visible trade that argument declined to make). This module
//! seals and opens the bytes; `sui-id-core`'s own code (the verifier,
//! and F06's caller) is the only thing that ever deserializes what is
//! inside them.
//!
//! ## AAD binding
//!
//! `ceremony_state_sealed` is bound to `pending_id`, `provider_id`,
//! `provider_config_version`, `provider_activation_generation` and
//! `user_id` — the same five-column, reconstruct-from-the-row
//! convention `federation_mfa_pending::build_aad` already uses for its
//! own `continuation_sealed`. `rp_id`/`origin` are deliberately **not**
//! bound: they are not secret and not cryptographic state (webauthn-rs's
//! own serialized `PasskeyAuthentication`, inside the sealed bytes,
//! already carries the origin/RP binding that matters for the actual
//! assertion verification); they are stored in the clear so a consumer
//! can recheck them against the *current* config without needing the
//! master key, mirroring how a provider's own `enabled` flag is
//! rechecked live rather than sealed.

use crate::errors::{StoreError, StoreResult};
use crate::models::FederationWebauthnCeremonyRow;
use rusqlite::params;
use sui_id_shared::ids::{
    FederationMfaPendingId, FederationProviderId, FederationWebauthnCeremonyId, UserId,
};

const SELECT_COLUMNS: &str = "id, pending_id, provider_id, provider_config_version, \
    provider_activation_generation, user_id, ceremony_state_sealed, rp_id, origin, \
    created_at, pending_expires_at, expires_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationWebauthnCeremonyRow> {
    let id_str: String = row.get(0)?;
    let pending_id_str: String = row.get(1)?;
    let provider_id_str: String = row.get(2)?;
    let user_id_str: String = row.get(5)?;
    Ok(FederationWebauthnCeremonyRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        pending_id: pending_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_id: provider_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_config_version: row.get(3)?,
        provider_activation_generation: row.get(4)?,
        user_id: user_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
        })?,
        ceremony_state_sealed: row.get(6)?,
        rp_id: row.get(7)?,
        origin: row.get(8)?,
        created_at: row.get(9)?,
        pending_expires_at: row.get(10)?,
        expires_at: row.get(11)?,
    })
}

/// The AAD bound into `ceremony_state_sealed` (module doc). Same
/// NUL-separated safety argument as `federation_mfa_pending::build_aad`.
pub fn build_aad(
    pending_id: FederationMfaPendingId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    user_id: UserId,
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(pending_id.to_string().as_bytes());
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

/// `contracts/write-commands.toml`'s own F06 row: "atomically replace
/// at most one ... ceremony" — one guarded `INSERT OR REPLACE`, keyed
/// on the table's own `pending_id UNIQUE`, not a separate
/// delete-then-insert this write could race past. A second F06 call
/// for the same still-pending row supersedes the first ceremony
/// outright, the same way a fresh TOTP code submission does not need
/// the previous one to be withdrawn first.
pub fn insert_or_replace_within_tx(
    conn: &rusqlite::Connection,
    row: &FederationWebauthnCeremonyRow,
) -> StoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO federation_webauthn_ceremony( \
            id, pending_id, provider_id, provider_config_version, provider_activation_generation, \
            user_id, ceremony_state_sealed, rp_id, origin, created_at, pending_expires_at, expires_at \
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            row.id.to_string(),
            row.pending_id.to_string(),
            row.provider_id.to_string(),
            row.provider_config_version,
            row.provider_activation_generation,
            row.user_id.to_string(),
            row.ceremony_state_sealed,
            row.rp_id,
            row.origin,
            row.created_at,
            row.pending_expires_at,
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

/// RFC 096-B1 stage 11: the verifier's own read (async, read-only —
/// stage 10a's own discipline, no writes from this path) and F03's
/// durable re-read (the `_within_tx` twin below) both need the full
/// row. `Ok(None)` for no ceremony, the same "absence is not an error"
/// shape `federation_mfa_pending::find_by_id` already uses.
pub async fn find_by_pending_id(
    db: &crate::Database,
    pending_id: FederationMfaPendingId,
) -> StoreResult<Option<FederationWebauthnCeremonyRow>> {
    let id_str = pending_id.to_string();
    db.with_read(move |read| {
        match read
            .prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM federation_webauthn_ceremony WHERE pending_id = ?1"
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

/// Same as [`find_by_pending_id`], on the caller's own transaction
/// (F03's own durable recheck — the verifier's earlier read is never
/// trusted a second time).
pub fn find_by_pending_id_within_tx(
    conn: &rusqlite::Connection,
    pending_id: FederationMfaPendingId,
) -> StoreResult<Option<FederationWebauthnCeremonyRow>> {
    match conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM federation_webauthn_ceremony WHERE pending_id = ?1"
        ))?
        .query_row([pending_id.to_string()], map_row)
    {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StoreError::from(e)),
    }
}

/// RFC 096 `:798-801`: F03 "consumes ceremony/pending" on a correct
/// completion, and "consumes only that failed ceremony" on a wrong one
/// — both are this same guarded-by-identity delete, called from two
/// different branches of F03's own match. Guarded on `id`, not merely
/// `pending_id`: the caller passes the exact ceremony id it already
/// durably re-read, so a ceremony replaced by a concurrent F06 call
/// between that read and this delete is not the one consumed.
pub fn consume_within_tx(
    conn: &rusqlite::Connection,
    id: FederationWebauthnCeremonyId,
) -> StoreResult<bool> {
    let n = conn.execute(
        "DELETE FROM federation_webauthn_ceremony WHERE id = ?1",
        [id.to_string()],
    )?;
    Ok(n == 1)
}

/// RFC 096 `:767-768`: exhaustion "deletes bound method ceremonies," and
/// `:798-801` a wrong WebAuthn completion "consumes only that failed
/// ceremony." Both are this same idempotent delete-by-`pending_id` —
/// the table's own `pending_id UNIQUE` already guarantees at most one
/// row for either caller to find, so there is no `id` to be guarded on
/// here the way [`consume_within_tx`] is: whichever ceremony is
/// currently bound to the row, if any, is the one either RFC sentence
/// means. `Ok(false)` for "nothing was there to delete" is not an
/// error — both callers call this unconditionally, including on the
/// common case of a TOTP/recovery wrong proof with no WebAuthn
/// ceremony ever created for the row.
pub fn delete_by_pending_id_within_tx(
    conn: &rusqlite::Connection,
    pending_id: FederationMfaPendingId,
) -> StoreResult<bool> {
    let n = conn.execute(
        "DELETE FROM federation_webauthn_ceremony WHERE pending_id = ?1",
        [pending_id.to_string()],
    )?;
    Ok(n == 1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
