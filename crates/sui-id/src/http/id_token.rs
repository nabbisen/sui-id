//! RFC 096-A prerequisite: moved verbatim out of `handlers/federation.rs`
//! (the preparatory split).
//!
//! **This is the point of the exercise.** Today this module only decodes
//! ID token claims without verifying the signature (`federation.rs` trusts
//! the upstream's `token_endpoint` over TLS instead — see
//! [`decode_id_token_claims`]'s own doc comment). RFC 096-A — not
//! dispatched by this split, which only prepares a module for it to land
//! in — replaces this with JOSE signature verification, a JWKS cache with
//! rotation, full claims validation, and one-time nonce consumption. That
//! code belongs here, not in `handlers/federation.rs`.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub nonce: Option<String>,
}

/// Decode JWT claims without verifying signature (we trust the upstream's
/// token_endpoint over TLS; full JWKS validation is a future hardening step).
pub fn decode_id_token_claims(jwt: &str) -> Option<IdTokenClaims> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let parts: Vec<&str> = jwt.split('.').collect();
    let payload = parts.get(1)?;
    // JWT compact serialization uses unpadded base64url.
    let decoded = Base64UrlUnpadded::decode_vec(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "id_token/tests.rs"]
mod tests;
