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
use crate::errors::StoreResult;
use crate::models::{FederationLoginAttemptRow, FederationLoginAttemptStatus};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use sui_id_shared::ids::{FederationLoginAttemptId, FederationProviderId};

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
