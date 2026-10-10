//! RFC 096-B1 stage 3: the claim (`pending -> exchanging`) and one-time
//! nonce consumption. RFC 096 `:67-68`: 096-A delivered the nonce
//! *validation rule* (`nonce_claim::validate_nonce`, stage 7, zero
//! production callers until now); the durable attempt state that makes a
//! nonce genuinely one-time is this stage's, because it is a mutation.
//!
//! The actual claim mutation — the guarded `UPDATE ... WHERE status =
//! 'pending'`, the clock-regression/expiry checks, and the AAD tamper
//! check on the sealed PKCE verifier — lives in
//! `sui_id_store::repos::federation_login_attempt::claim`, one layer down,
//! for the same reason stage 2's `insert` does: this crate is the first
//! one that can see both that function and [`crate::nonce_claim::
//! validate_nonce`] (`sui-id-core`, where a clock would live, cannot name
//! `validate_nonce` at all — it is declared in this crate, not
//! `sui-id-core`). This module is the thin orchestration that calls one
//! then the other; it owns no SQL of its own.
//!
//! **No code exchange, no session, no identity mapping** (stage 3's own
//! scope boundary) — and no transition to `failed` on a nonce mismatch
//! either: that is F05's guarded `pending`/`exchanging -> failed`
//! transition, a separate registered command, not this stage's to invent
//! a second path to.
//!
//! ## The `nonce_sha256` / `expected_digest` boundary (stage 3, item 5)
//!
//! [`crate::nonce_claim::validate_nonce`] takes `expected_digest: &str`
//! and refuses anything that is not exactly 64 hex characters
//! (`is_64_hex`); `nonce_sha256` is a 32-byte `BLOB`. The conversion is a
//! byte loop (`{b:02x}` per byte, lowercase) here, not a change to either
//! side: `validate_nonce` already compares digests, not nonces, and
//! `is_64_hex` was written to accept either case — `sha256_hex` (its own
//! token-side digest, `sui_id_core::tokens`) happens to always emit
//! lowercase, so encoding `nonce_sha256` the same way means the two sides
//! are never case-mismatched to begin with, not merely tolerated if they
//! were. (096-A's own review already recorded `is_64_hex` accepting
//! uppercase while `sha256_hex` emits lowercase as a deferred tidy — not
//! this stage's to fix, and not a reason to encode `nonce_sha256` any
//! differently than lowercase here.)

use chrono::{DateTime, Utc};
use sui_id_shared::ids::FederationLoginAttemptId;
use sui_id_store::models::FederationLoginAttemptRow;
use sui_id_store::{Database, StoreError};

use crate::id_token::VerifiedIdTokenClaims;
use crate::nonce_claim::{NonceError, validate_nonce};

/// Either stage of this module's work can fail, with different shaped
/// errors from different crates — named rather than flattened into one
/// variant set, so a caller can tell "the claim itself was refused" from
/// "the claim succeeded but the nonce did not match" without string
/// matching.
#[derive(Debug)]
pub enum ClaimAndNonceError {
    Claim(StoreError),
    Nonce(NonceError),
}

impl std::fmt::Display for ClaimAndNonceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Claim(e) => write!(f, "attempt claim failed: {e}"),
            Self::Nonce(e) => write!(f, "nonce consumption failed: {e}"),
        }
    }
}

impl std::error::Error for ClaimAndNonceError {}

/// Lowercase hex, matching `sui_id_core::tokens::sha256_hex`'s own output
/// shape exactly (module doc).
fn hex_lower(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(64);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

/// Claim `attempt_id` (`pending -> exchanging`, single-use, RFC 096
/// `:590-592`) and consume its nonce against `claims`. `now_at_claim` is
/// sampled by the caller exactly once, the same convention
/// `federation_login_attempt::insert`/`claim` already use — this function
/// reads no clock of its own.
///
/// On success, the attempt is already `exchanging` in the database; a
/// `Nonce` error here does **not** roll that back — see the module doc for
/// why that is F05's job, not this function's.
pub async fn claim_and_consume_nonce(
    db: &Database,
    attempt_id: FederationLoginAttemptId,
    claims: &VerifiedIdTokenClaims,
    now_at_claim: DateTime<Utc>,
) -> Result<FederationLoginAttemptRow, ClaimAndNonceError> {
    let row = sui_id_store::repos::federation_login_attempt::claim(db, attempt_id, now_at_claim)
        .await
        .map_err(ClaimAndNonceError::Claim)?;
    let expected_digest = hex_lower(&row.nonce_sha256);
    validate_nonce(claims, &expected_digest).map_err(ClaimAndNonceError::Nonce)?;
    Ok(row)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation_attempt_claim/tests.rs"]
mod tests;
