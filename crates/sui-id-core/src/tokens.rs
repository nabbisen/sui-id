//! Token issuance and claim shapes.
//!
//! sui-id issues:
//!
//! * **Access Token** — short-lived (default 15 min) JWT, audience is the
//!   resource server pattern; carries `sub`, `aud`, `scope`.
//! * **ID Token** — short-lived JWT identifying the user to the relying
//!   party; carries `sub`, `aud` (the client id), `nonce` echoed from the
//!   authorization request.
//! * **Refresh Token** — opaque random string; stored sealed in the database.
//!   Lifetime defaults to 14 days; rotates on each use.

use crate::errors::{CoreError, CoreResult};
use crate::jwt;
use crate::time::SharedClock;
use base64ct::{Base64UrlUnpadded, Encoding};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sui_id_shared::ids::{ClientId, UserId};

/// Standard claims for an access token.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// Issuer (sui-id base URL).
    pub iss: String,
    /// Subject (user id).
    pub sub: String,
    /// Audience (client id) - we keep it simple and use a single string.
    pub aud: String,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds).
    pub exp: i64,
    /// Space-separated scope.
    pub scope: String,
    /// JWT id (random per-token).
    pub jti: String,
}

/// Standard claims for an OIDC ID token.
#[derive(Debug, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    /// Original `nonce` from the authorization request, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// Convenience identifier used by some clients.
    pub jti: String,
}

#[derive(Debug, Clone, Copy)]
pub struct TokenLifetimes {
    pub access_secs: i64,
    pub id_secs: i64,
    pub refresh_secs: i64,
}

impl Default for TokenLifetimes {
    fn default() -> Self {
        Self {
            access_secs: 15 * 60,
            id_secs: 15 * 60,
            refresh_secs: 14 * 24 * 60 * 60,
        }
    }
}

/// Output bundle of [`issue_token_set`].
pub struct TokenSet {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: String,
    pub access_expires_in: i64,
}

/// Generate one issuance of access + id + refresh tokens for `(user, client)`.
///
/// `nonce` is the original OIDC `nonce` from the authorization request, if
/// present. `include_id_token` is true when `openid` is among the requested
/// scopes.
#[allow(clippy::too_many_arguments)]
pub fn issue_token_set(
    issuer: &str,
    user: UserId,
    client: ClientId,
    scope: &str,
    nonce: Option<&str>,
    include_id_token: bool,
    kid: &str,
    signing_key: &SigningKey,
    lifetimes: TokenLifetimes,
    clock: &SharedClock,
) -> CoreResult<TokenSet> {
    let now = clock.now();
    let iat = now.timestamp();

    let access_claims = AccessTokenClaims {
        iss: issuer.to_owned(),
        sub: user.to_string(),
        aud: client.to_string(),
        iat,
        exp: iat + lifetimes.access_secs,
        scope: scope.to_owned(),
        jti: random_token(16),
    };
    let access_token = jwt::sign(kid, signing_key, &access_claims)?;

    let id_token = if include_id_token {
        let claims = IdTokenClaims {
            iss: issuer.to_owned(),
            sub: user.to_string(),
            aud: client.to_string(),
            iat,
            exp: iat + lifetimes.id_secs,
            nonce: nonce.map(str::to_owned),
            jti: random_token(16),
        };
        Some(jwt::sign(kid, signing_key, &claims)?)
    } else {
        None
    };

    Ok(TokenSet {
        access_token,
        id_token,
        refresh_token: random_token(32),
        access_expires_in: lifetimes.access_secs,
    })
}

/// Cryptographically random URL-safe token string.
pub fn random_token(byte_len: usize) -> String {
    let mut buf = vec![0u8; byte_len];
    OsRng.fill_bytes(&mut buf);
    let mut out = vec![0u8; byte_len * 2 + 4];
    let n = Base64UrlUnpadded::encode(&buf, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).expect("base64url is ascii")
}

/// SHA-256 of the bytes, hex-lowercase. Used to index authorization codes
/// without storing the plaintext.
pub fn sha256_hex(s: &str) -> String {
    let digest = Sha256::digest(s.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

/// PKCE challenge verification per RFC 7636 §4.6.
pub fn verify_pkce(method: &str, verifier: &str, expected_challenge: &str) -> CoreResult<()> {
    use subtle::ConstantTimeEq;
    let computed = match method {
        "S256" => {
            let digest = Sha256::digest(verifier.as_bytes());
            let mut out = vec![0u8; 64];
            let n = Base64UrlUnpadded::encode(&digest, &mut out)
                .map(str::len)
                .unwrap_or(0);
            out.truncate(n);
            String::from_utf8(out).map_err(|_| CoreError::Internal)?
        }
        "plain" => verifier.to_owned(),
        _ => {
            return Err(CoreError::BadRequest(format!(
                "unsupported code_challenge_method: {method}"
            )));
        }
    };
    if computed.as_bytes().ct_eq(expected_challenge.as_bytes()).into() {
        Ok(())
    } else {
        Err(CoreError::Protocol {
            code: crate::errors::ProtocolError::InvalidGrant,
            description: "PKCE verification failed".into(),
        })
    }
}

/// Verify a sui-id access token against the active and recently-rotated
/// signing keys. Returns the validated claims.
///
/// This wraps the `jwt::verify` + JWKS lookup + expiry check so that the
/// HTTP layer does not have to know about Ed25519 specifics.
pub fn verify_access_token(
    db: &sui_id_store::Database,
    clock: &crate::time::SharedClock,
    token: &str,
) -> crate::CoreResult<AccessTokenClaims> {
    use ed25519_dalek::VerifyingKey;
    use sui_id_store::repos::signing_keys;

    let resolver = |kid: &str| -> Option<VerifyingKey> {
        let rows = signing_keys::list_published(db).ok()?;
        let m = rows.into_iter().find(|r| r.id.to_string() == kid)?;
        let arr: [u8; 32] = m.public_key.as_slice().try_into().ok()?;
        VerifyingKey::from_bytes(&arr).ok()
    };
    let decoded: crate::jwt::Decoded<AccessTokenClaims> =
        crate::jwt::verify(token, resolver)?;
    if decoded.claims.exp < clock.now().timestamp() {
        return Err(crate::CoreError::Unauthenticated);
    }
    Ok(decoded.claims)
}

#[cfg(test)]
mod tests;
