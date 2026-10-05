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
mod tests {
    use super::decode_id_token_claims;
    use base64ct::{Base64UrlUnpadded, Encoding};

    fn jwt_with_payload(payload: &[u8]) -> String {
        let header = Base64UrlUnpadded::encode_string(br#"{"alg":"none"}"#);
        let payload = Base64UrlUnpadded::encode_string(payload);
        format!("{header}.{payload}.")
    }

    #[test]
    fn decode_id_token_claims_accepts_unpadded_jwt_payload() {
        let jwt = jwt_with_payload(
            br#"{"sub":"upstream-123","email":"alice@example.com","email_verified":true,"nonce":"n"}"#,
        );

        let claims = decode_id_token_claims(&jwt).expect("claims should decode");

        assert_eq!(claims.sub, "upstream-123");
        assert_eq!(claims.email.as_deref(), Some("alice@example.com"));
        assert!(claims.email_verified);
        assert_eq!(claims.nonce.as_deref(), Some("n"));
    }

    #[test]
    fn decode_id_token_claims_rejects_malformed_payload() {
        assert!(decode_id_token_claims("header.not-base64url.signature").is_none());
    }
}
