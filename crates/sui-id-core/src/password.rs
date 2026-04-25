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
use rand::rngs::OsRng;

fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 2, 1, None).unwrap_or_else(|_| Params::default());
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a password and return its PHC-encoded string.
pub fn hash_password(password: &str) -> CoreResult<String> {
    let salt = SaltString::generate(&mut OsRng);
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
pub fn check_password_policy(password: &str) -> CoreResult<()> {
    if password.chars().count() < 12 {
        return Err(CoreError::BadRequest(
            "password must be at least 12 characters long".into(),
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
