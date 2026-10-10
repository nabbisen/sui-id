//! RFC 096-B1 stage 2: durable attempt start. RFC 096 `:583-589`, the
//! `pending` row only — claiming it (`pending -> exchanging`) is stage 3.
//!
//! ## AAD binding
//!
//! `pkce_verifier_sealed` is XChaCha20-Poly1305 sealed with AAD binding
//! `id`, `provider_id`, `provider_config_version` and
//! `provider_activation_generation` (RFC 096 `:586-587`). All four are
//! already columns on this same row, so [`build_aad`] reconstructs the AAD
//! from them rather than storing it a second time. Altering any one of the
//! four after the fact — a different attempt id, a different provider, or
//! a stale config version/activation generation — changes the AAD and makes
//! the sealed verifier fail to open; see `tests.rs` for the test proving
//! this for each of the four independently, which is the actual point of
//! binding them at all.
//!
//! ## Clock
//!
//! No `SharedClock` here: `sui-id-core` (where `SharedClock` lives) depends
//! on `sui-id-store`, not the other way around, so this crate cannot name
//! that type. [`insert`] takes an already-sampled `now: DateTime<Utc>`, the
//! same convention every other timestamp-taking function in this crate
//! already uses (e.g. `sessions::count_active_for_user`). The caller reads
//! the clock exactly once and derives both `created_at` and `expires_at`
//! from that one reading — see the module's own "clock regression" note in
//! the stage 2 package for why this is where that discipline lives.

use crate::db::Database;
use crate::errors::{StoreError, StoreResult};
use crate::models::{FederationLoginAttemptRow, FederationLoginAttemptStatus};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use sui_id_shared::ids::{FederationLoginAttemptId, FederationProviderId};

const SELECT_COLUMNS: &str = "id, provider_id, provider_config_version, \
    provider_activation_generation, state_sha256, nonce_sha256, browser_binding_sha256, \
    pkce_verifier_sealed, exact_redirect_uri, next_path, created_at, expires_at, status, \
    claimed_at";

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FederationLoginAttemptRow> {
    let id_str: String = row.get(0)?;
    let provider_id_str: String = row.get(1)?;
    let state_sha256: Vec<u8> = row.get(4)?;
    let nonce_sha256: Vec<u8> = row.get(5)?;
    let browser_binding_sha256: Vec<u8> = row.get(6)?;
    let status_str: String = row.get(12)?;
    let fixed = |v: Vec<u8>, col: usize| -> rusqlite::Result<[u8; 32]> {
        v.try_into().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                col,
                rusqlite::types::Type::Blob,
                "not 32 bytes".into(),
            )
        })
    };
    Ok(FederationLoginAttemptRow {
        id: id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_id: provider_id_str.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        provider_config_version: row.get(2)?,
        provider_activation_generation: row.get(3)?,
        state_sha256: fixed(state_sha256, 4)?,
        nonce_sha256: fixed(nonce_sha256, 5)?,
        browser_binding_sha256: fixed(browser_binding_sha256, 6)?,
        pkce_verifier_sealed: row.get(7)?,
        exact_redirect_uri: row.get(8)?,
        next_path: row.get(9)?,
        created_at: row.get(10)?,
        expires_at: row.get(11)?,
        status: status_str.parse().map_err(|_: StoreError| {
            rusqlite::Error::FromSqlConversionFailure(
                12,
                rusqlite::types::Type::Text,
                "unknown federation_login_attempt status".into(),
            )
        })?,
        claimed_at: row.get(13)?,
    })
}

/// RFC 096 `:588-589`: 600 seconds from `created_at`.
pub const ATTEMPT_LIFETIME_SECS: i64 = 600;

/// The AAD bound into `pkce_verifier_sealed` (module doc). NUL-separated:
/// safe because neither a UUID string (hex and hyphens only) nor a decimal
/// integer can contain a NUL byte, so no two distinct
/// `(id, provider_id, version, generation)` tuples can produce the same
/// byte string.
fn build_aad(
    id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
) -> Vec<u8> {
    let mut aad = Vec::new();
    aad.extend_from_slice(id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_id.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_config_version.to_string().as_bytes());
    aad.push(0);
    aad.extend_from_slice(provider_activation_generation.to_string().as_bytes());
    aad
}

/// Create a `pending` `federation_login_attempt` row. `pkce_verifier_plain`
/// is sealed before the write; the plaintext is never stored.
///
/// `now` is sampled by the caller exactly once and used for both
/// `created_at` and `expires_at = now + 600s` — this function does not read
/// a clock of its own, so there is nothing for it to disagree with.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    db: &Database,
    provider_id: FederationProviderId,
    provider_config_version: i64,
    provider_activation_generation: i64,
    state_sha256: [u8; 32],
    nonce_sha256: [u8; 32],
    browser_binding_sha256: [u8; 32],
    pkce_verifier_plain: &[u8],
    exact_redirect_uri: String,
    next_path: Option<String>,
    now: DateTime<Utc>,
) -> StoreResult<FederationLoginAttemptRow> {
    let id = FederationLoginAttemptId::new();
    let aad = build_aad(
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
    );
    // Crypto is sync; sealing before entering `with_conn` avoids holding a
    // connection across it (same shape as `federation_provider::create`).
    let pkce_verifier_sealed = crate::crypto::seal(db.key(), pkce_verifier_plain, &aad)?;

    let row = FederationLoginAttemptRow {
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
        state_sha256,
        nonce_sha256,
        browser_binding_sha256,
        pkce_verifier_sealed,
        exact_redirect_uri,
        next_path,
        created_at: now,
        expires_at: now + Duration::seconds(ATTEMPT_LIFETIME_SECS),
        status: FederationLoginAttemptStatus::Pending,
        claimed_at: None,
    };

    let insert_row = row.clone();
    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_login_attempt( \
                id, provider_id, provider_config_version, provider_activation_generation, \
                state_sha256, nonce_sha256, browser_binding_sha256, pkce_verifier_sealed, \
                exact_redirect_uri, next_path, created_at, expires_at, status, claimed_at \
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                insert_row.id.to_string(),
                insert_row.provider_id.to_string(),
                insert_row.provider_config_version,
                insert_row.provider_activation_generation,
                insert_row.state_sha256.as_slice(),
                insert_row.nonce_sha256.as_slice(),
                insert_row.browser_binding_sha256.as_slice(),
                insert_row.pkce_verifier_sealed,
                insert_row.exact_redirect_uri,
                insert_row.next_path,
                insert_row.created_at,
                insert_row.expires_at,
                insert_row.status.as_str(),
                insert_row.claimed_at,
            ],
        )?;
        Ok(())
    })
    .await?;

    Ok(row)
}

/// RFC 096-B1 stage 3: claim a `pending` attempt, `pending -> exchanging`,
/// single-use (RFC 096 `:590-592`).
///
/// **Why this is a read, then a guarded check, then a conditional `UPDATE`
/// — not one statement folding every condition into the `WHERE` clause.**
/// `status`, `expires_at` and `created_at` are read once because the
/// dispatch asks for *distinguishable* refusals (`NotFound` / `Conflict` /
/// [`StoreError::AttemptExpired`] / [`StoreError::ClockRegression`]), and a
/// single `UPDATE ... WHERE status = 'pending' AND expires_at > ?3` would
/// collapse all of "never existed," "already claimed," and "expired" into
/// one indistinguishable zero-rows-affected outcome — correct for
/// `login_pending_mfa::consume_within_tx`'s single generic outcome, wrong
/// here. `expires_at` and `created_at` are immutable once inserted (never
/// written anywhere outside `insert`), so reading them separately from the
/// one thing that *does* change concurrently — `status` — introduces no
/// race: only the final `UPDATE`'s own `WHERE status = 'pending'` clause
/// needs to be atomic, because `status` is the one value two concurrent
/// callers could disagree about.
///
/// **The affected-row count is the authority for the claim itself**, exactly
/// as the dispatch states: if the `UPDATE` affects zero rows, this caller
/// did not win, whether because the row was never `pending` or because
/// another caller's `UPDATE` landed first — both report
/// [`StoreError::Conflict`], the same variant the read-time "not pending"
/// check above also uses, since both mean the same thing to a caller: this
/// attempt is not available to claim, right now, by you.
pub async fn claim(
    db: &Database,
    id: FederationLoginAttemptId,
    now_at_claim: DateTime<Utc>,
) -> StoreResult<FederationLoginAttemptRow> {
    let id_str = id.to_string();
    let row = db
        .with_read({
            let id_str = id_str.clone();
            move |read| {
                read.prepare(&format!(
                    "SELECT {SELECT_COLUMNS} FROM federation_login_attempt WHERE id = ?1"
                ))?
                .query_row([id_str], map_row)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
                    other => StoreError::from(other),
                })
            }
        })
        .await?;

    if row.status != FederationLoginAttemptStatus::Pending {
        return Err(StoreError::Conflict);
    }
    // Clock regression checked before expiry, and independently of it:
    // `expires_at <= now_at_claim` alone would be the wrong test under a
    // backward clock (RFC 096 `:589`) -- it makes the window look *less*
    // elapsed, not more, so a regressed clock could pass it when it must
    // not.
    if now_at_claim < row.created_at {
        return Err(StoreError::ClockRegression);
    }
    if row.expires_at <= now_at_claim {
        return Err(StoreError::AttemptExpired);
    }

    let affected = db
        .with_conn({
            let id_str = id_str.clone();
            move |conn| {
                Ok(conn.execute(
                    "UPDATE federation_login_attempt SET status = 'exchanging', claimed_at = ?1 \
                     WHERE id = ?2 AND status = 'pending'",
                    params![now_at_claim, id_str],
                )?)
            }
        })
        .await?;
    if affected == 0 {
        return Err(StoreError::Conflict);
    }

    // The tamper check (RFC 096-B1 stage 3, item 3): reconstruct the AAD
    // from the row's own four bound columns and open the sealed verifier.
    // If any of the four has changed since `insert` sealed it, this fails
    // -- the check this stage gets for free, not a hand-written field
    // comparison. The plaintext itself is not needed by this stage (nonce
    // consumption uses `nonce_sha256`, not the PKCE verifier) and is
    // dropped rather than returned: stage 4's token exchange is what needs
    // it, and presuming its shape here is exactly the kind of thing stage 2
    // already declined to do one layer down.
    let aad = build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
    );
    drop(crate::crypto::open(
        db.key(),
        &row.pkce_verifier_sealed,
        &aad,
    )?);

    Ok(FederationLoginAttemptRow {
        status: FederationLoginAttemptStatus::Exchanging,
        claimed_at: Some(now_at_claim),
        ..row
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
