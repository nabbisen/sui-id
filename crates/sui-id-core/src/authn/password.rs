//! Password hashing and verification using Argon2id.
//!
//! Defaults: Argon2id, m=64 MiB, t=2, p=1. The `argon2` crate handles the
//! random salt and PHC encoding; we just supply parameters and verify in
//! constant time.
//!
//! RFC 126: both operations are expensive by design (one call measured at
//! ~34 ms — see the RFC) and neither runs on a Tokio runtime worker thread.
//! `hash_password`/`verify_password` are the **one boundary** every caller
//! goes through (D2): each clones its inputs to owned data, acquires a
//! permit from [`HASH_SEMAPHORE`] (bounding *concurrent* 64 MiB allocations,
//! not thread count — D4), and runs the actual Argon2id call inside
//! `tokio::task::spawn_blocking`, the same idiom this project already uses
//! for SQLite work (`sui-id-store/src/backend.rs`). A caller that meets the
//! concurrency bound queues on `Semaphore::acquire().await`, which does not
//! hold a worker thread while pending — it does not reintroduce the defect
//! this RFC exists to fix.

use crate::errors::{CoreError, CoreResult};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use std::sync::OnceLock;
use tokio::sync::Semaphore;

fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 2, 1, None).unwrap_or_else(|_| Params::default());
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// A fixed, well-formed Argon2id PHC string that matches no real password —
/// a decoy for a caller to call [`verify_password`] against on any path that
/// would otherwise skip hashing entirely, so that path costs the same
/// wall-clock time as a real verification (RFC 123 D4). Originally private to
/// `authn::session`; promoted here so `oidc::oauth_token::authenticate_client`
/// can use the same decoy rather than defining its own.
pub const DUMMY_PHC: &str =
    "$argon2id$v=19$m=65536,t=2,p=1$c2FsdHNhbHRzYWx0$ZHVtbXloYXNoZHVtbXloYXNoZHVtbXloYXNoZHVtbQ";

/// RFC 126 D4: bounds concurrent Argon2id invocations (hashing or
/// verifying — both cost the same 64 MiB), not the Tokio blocking pool's
/// thread count, which is what would matter if the concern were threads
/// rather than memory. Derived from `available_parallelism`, not picked: on
/// the one-to-four-core shape this project describes, 2-8 permits, 128-512
/// MiB peak — far below what the pool's own 512-thread default would allow
/// (32 GiB). The ceiling is a starting point for an operator to retune, not
/// a measured limit.
static HASH_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

/// The derivation itself, pulled out so it can be tested as a pure function
/// across core counts without touching the live, process-wide semaphore
/// (which every concurrently running test also shares — a wall-clock timing
/// test of the real semaphore cannot reliably isolate its effect from
/// Argon2's own memory-bandwidth contention, which grows with concurrent
/// instances regardless of whether a software bound exists at all).
fn concurrency_bound(cores: usize) -> usize {
    cores.saturating_mul(2).min(16)
}

fn hash_semaphore() -> &'static Semaphore {
    HASH_SEMAPHORE.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Semaphore::new(concurrency_bound(cores))
    })
}

/// Hash a password and return its PHC-encoded string.
pub async fn hash_password(password: &str) -> CoreResult<String> {
    let password = password.to_owned();
    let _permit = hash_semaphore()
        .acquire()
        .await
        .map_err(|_| CoreError::Internal)?;
    tokio::task::spawn_blocking(move || hash_password_sync(&password))
        .await
        .map_err(|_| CoreError::Internal)?
}

fn hash_password_sync(password: &str) -> CoreResult<String> {
    // RFC 069: generate salt via getrandom (16 bytes = 128 bits, then B64-encode
    // for argon2/password-hash). Replaces SaltString::generate(&mut OsRng) which
    // required rand_core 0.6's CryptoRng trait, incompatible with rand_core 0.10.
    let mut salt_bytes = [0u8; 16];
    getrandom::fill(&mut salt_bytes).map_err(|_| CoreError::Internal)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| CoreError::Password)?;
    let phc = argon2()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| CoreError::Password)?;
    Ok(phc.to_string())
}

/// Verify `password` against a previously stored PHC hash. Returns `Ok(())`
/// on match, [`CoreError::InvalidCredentials`] on mismatch, and only returns
/// [`CoreError::Password`] for malformed stored hashes.
pub async fn verify_password(password: &str, stored_phc: &str) -> CoreResult<()> {
    let password = password.to_owned();
    let stored_phc = stored_phc.to_owned();
    let _permit = hash_semaphore()
        .acquire()
        .await
        .map_err(|_| CoreError::Internal)?;
    tokio::task::spawn_blocking(move || verify_password_sync(&password, &stored_phc))
        .await
        .map_err(|_| CoreError::Internal)?
}

fn verify_password_sync(password: &str, stored_phc: &str) -> CoreResult<()> {
    let parsed = PasswordHash::new(stored_phc).map_err(|_| CoreError::Password)?;
    argon2()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| CoreError::InvalidCredentials)
}

/// Reasonable minimum-length policy. Intentionally lenient on character
/// classes: NIST SP 800-63B advises *against* composition rules.
///
/// `min_len` comes from `SecurityLevel::password_min_len()` — 12 for
/// production, 8 in `--dev` mode. Core functions receive the value
/// from their callers so this function stays unaware of the run mode.
pub fn check_password_policy(password: &str, min_len: usize) -> CoreResult<()> {
    if password.chars().count() < min_len {
        return Err(CoreError::BadRequest(format!(
            "password must be at least {min_len} characters long"
        )));
    }
    if password.chars().count() > 256 {
        return Err(CoreError::BadRequest(
            "password is unreasonably long (max 256)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "password/tests.rs"]
mod tests;
