//! Password hashing and verification using Argon2id.
//!
//! Defaults: Argon2id, m=64 MiB, t=2, p=1. The `argon2` crate handles the
//! random salt and PHC encoding; we just supply parameters and verify in
//! constant time.

use crate::errors::{CoreError, CoreResult};
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::{Algorithm, Argon2, Params, Version};

fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 2, 1, None).unwrap_or_else(|_| Params::default());
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a password and return its PHC-encoded string.
pub fn hash_password(password: &str) -> CoreResult<String> {
    // RFC 069: generate salt via getrandom (16 bytes = 128 bits, then B64-encode
    // for argon2/password-hash). Replaces SaltString::generate(&mut OsRng) which
    // required rand_core 0.6's CryptoRng trait, incompatible with rand_core 0.10.
    let mut salt_bytes = [0u8; 16];
    getrandom::fill(&mut salt_bytes).expect("system RNG unavailable");
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| CoreError::Password)?;
    let phc = argon2()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| CoreError::Password)?;
    Ok(phc.to_string())
}

/// Verify `password` against a previously stored PHC hash. Returns `Ok(())`
/// on match, [`CoreError::InvalidCredentials`] on mismatch, and only returns
/// [`CoreError::Password`] for malformed stored hashes.
pub fn verify_password(password: &str, stored_phc: &str) -> CoreResult<()> {
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
        return Err(CoreError::BadRequest(
            format!("password must be at least {min_len} characters long"),
        ));
    }
    if password.chars().count() > 256 {
        return Err(CoreError::BadRequest(
            "password is unreasonably long (max 256)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
