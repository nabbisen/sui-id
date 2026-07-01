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

use getrandom;
use crate::errors::{CoreError, CoreResult};
use crate::jwt;
use crate::time::SharedClock;
use base64ct::{Base64UrlUnpadded, Encoding};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sui_id_shared::ids::{ClientId, UserId};
use sui_id_shared::RawRefreshToken;

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
    /// Authentication Context Class Reference (OIDC Core §2). The
    /// numeric ISO 29115 LoA strings `"1"`, `"2"`, `"3"`. Always
    /// present from v0.15.0 onward.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acr: Option<String>,
    /// Authentication Methods References (OIDC Core §2; values from
    /// RFC 8176). Tokens like `"pwd"`, `"otp"`, `"hwk"`, plus `"mfa"`
    /// when two or more distinct factors were used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amr: Option<Vec<String>>,
    /// Email address of the user (OIDC Core §5.1).
    ///
    /// Included in the ID token when **all** of the following hold:
    /// 1. The granted scope contains `"email"`.
    /// 2. The user has an email address on record.
    ///
    /// Omitted (not serialised) when absent so that `email` is never
    /// `null` in the JWT payload — some RP parsers treat an explicit
    /// `null` as a deserialization error for a `String` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the `email` address has been confirmed (OIDC Core §5.1).
    /// Always `false` until an email-verification flow is implemented;
    /// only present when `email` is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
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
    /// The plaintext refresh token, held as a `RawRefreshToken` so it is
    /// zeroed on drop and never appears in `Debug` output. The handler
    /// calls `.expose()` exactly once when serializing the HTTP response.
    pub refresh_token: RawRefreshToken,
    pub access_expires_in: i64,
    /// RFC 072: the user this token set was issued for. Present when the
    /// grant type is authorization_code or refresh_token; None for
    /// machine-to-machine grants (none shipped yet).
    pub user_id: Option<UserId>,
}

#[allow(clippy::too_many_arguments)]
pub async fn issue_token_set(
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
    auth_methods: &[sui_id_shared::AuthMethod],
    // user_email: The user's email address and verified status. Only embedded in
    // the ID token when (a) this is `Some` **and** (b) the granted
    // `scope` contains `"email"`. Callers that don't have the user
    // row available (or that know the scope cannot include `email`)
    // may pass `None` safely — the claims struct still omits the
    // field due to `#[serde(skip_serializing_if = "Option::is_none")]`.
    user_email: Option<(&str, bool)>,
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
        let (acr, amr) = if auth_methods.is_empty() {
            (None, None)
        } else {
            (
                Some(sui_id_shared::acr_from_methods(auth_methods).to_string()),
                Some(sui_id_shared::amr_from_methods(auth_methods)),
            )
        };
        // Include email claims in the ID token when the granted scope
        // includes "email" AND the caller supplied the user's email.
        // OIDC Core §5.1: the email scope maps to email + email_verified.
        let scope_has_email = scope.split_whitespace().any(|s| s == "email");
        let (email_claim, email_verified_claim) = if scope_has_email {
            match user_email {
                Some((addr, verified)) => (Some(addr.to_owned()), Some(verified)),
                None => (None, None),
            }
        } else {
            (None, None)
        };
        let claims = IdTokenClaims {
            iss: issuer.to_owned(),
            sub: user.to_string(),
            aud: client.to_string(),
            iat,
            exp: iat + lifetimes.id_secs,
            nonce: nonce.map(str::to_owned),
            jti: random_token(16),
            acr,
            amr,
            email: email_claim,
            email_verified: email_verified_claim,
        };
        Some(jwt::sign(kid, signing_key, &claims)?)
    } else {
        None
    };

    Ok(TokenSet {
        access_token,
        id_token,
        refresh_token: RawRefreshToken::generate(),
        access_expires_in: lifetimes.access_secs,
        user_id: Some(user),
    })
}

/// Cryptographically random URL-safe token string.
pub fn random_token(byte_len: usize) -> String {
    let mut buf = vec![0u8; byte_len];
    getrandom::fill(&mut buf).expect("system RNG unavailable");
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
///
/// sui-id only supports `S256`. The `plain` method described in
/// RFC 7636 is intentionally rejected here as a defense-in-depth
/// layer behind the same check at the `/oauth2/authorize` entry
/// point — if that check ever regresses, this layer still refuses
/// to verify. A relying party that needs `plain` (none should in
/// 2026) must run a different IdP.
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
        _ => {
            // Includes the literal string "plain" — which is the
            // most likely thing to land here if the upstream
            // defence regresses. Refuse with the same error as for
            // any other unknown method.
            return Err(CoreError::Protocol {
                code: crate::errors::ProtocolError::InvalidGrant,
                description: format!("unsupported code_challenge_method: {method}"),
            });
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

/// Verify a sui-id ID token against the active and recently-rotated
/// signing keys. Returns the validated claims.
///
/// `accept_expired` allows passing tokens that have aged out — used by
/// RP-initiated logout, where the spec encourages accepting expired hints
/// so the user can still sign out after their token has aged.


/// Verify a sui-id access token against the active and recently-rotated
/// signing keys. Returns the validated claims.
///
/// This wraps the `jwt::verify` + JWKS lookup + expiry check so that the
/// HTTP layer does not have to know about Ed25519 specifics.


// ── JWT verification helpers (RFC 014) ───────────────────────────────────────

/// Resolve a signing key by kid from a cache snapshot and verify the token.
fn verify_from_snapshot<C: serde::de::DeserializeOwned>(
    keys: &[crate::cache::CachedSigningKey],
    token: &str,
) -> crate::CoreResult<crate::jwt::Decoded<C>> {
    use ed25519_dalek::VerifyingKey;
    let resolver = |kid: &str| -> Option<VerifyingKey> {
        let entry = keys.iter().find(|k| k.kid == kid)?;
        let arr: [u8; 32] = entry.public_key_bytes.as_slice().try_into().ok()?;
        VerifyingKey::from_bytes(&arr).ok()
    };
    crate::jwt::verify(token, resolver)
}

fn published_to_cached(
    rows: Vec<sui_id_store::models::SigningKeyRow>,
) -> Vec<crate::cache::CachedSigningKey> {
    rows.into_iter()
        .map(|k| crate::cache::CachedSigningKey {
            kid: k.id.to_string(),
            algorithm: k.algorithm,
            public_key_bytes: k.public_key,
        })
        .collect()
}

/// Verify an ID token from the DB-fetched key list.
pub async fn verify_id_token(
    db: &sui_id_store::Database,
    clock: &crate::time::SharedClock,
    token: &str,
    accept_expired: bool,
) -> crate::CoreResult<IdTokenClaims> {
    let keys = published_to_cached(
        sui_id_store::repos::signing_keys::list_published(db)
            .await
            .unwrap_or_default(),
    );
    let decoded: crate::jwt::Decoded<IdTokenClaims> = verify_from_snapshot(&keys, token)?;
    if !accept_expired && decoded.claims.exp < clock.now().timestamp() {
        return Err(crate::CoreError::Unauthenticated);
    }
    Ok(decoded.claims)
}

/// Verify an ID token using the JWKS cache snapshot (RFC 014 hot path).
pub async fn verify_id_token_cached(
    caches: &crate::cache::Caches,
    clock: &crate::time::SharedClock,
    token: &str,
    accept_expired: bool,
) -> crate::CoreResult<IdTokenClaims> {
    let keys = caches.jwks.snapshot().await;
    let decoded: crate::jwt::Decoded<IdTokenClaims> = verify_from_snapshot(&keys, token)?;
    if !accept_expired && decoded.claims.exp < clock.now().timestamp() {
        return Err(crate::CoreError::Unauthenticated);
    }
    Ok(decoded.claims)
}

/// Verify an access token from the DB-fetched key list.
pub async fn verify_access_token(
    db: &sui_id_store::Database,
    clock: &crate::time::SharedClock,
    token: &str,
) -> crate::CoreResult<AccessTokenClaims> {
    let keys = published_to_cached(
        sui_id_store::repos::signing_keys::list_published(db)
            .await
            .unwrap_or_default(),
    );
    let decoded: crate::jwt::Decoded<AccessTokenClaims> = verify_from_snapshot(&keys, token)?;
    if decoded.claims.exp < clock.now().timestamp() {
        return Err(crate::CoreError::Unauthenticated);
    }
    Ok(decoded.claims)
}

/// Verify an access token using the JWKS cache snapshot (RFC 014 hot path).
pub async fn verify_access_token_cached(
    caches: &crate::cache::Caches,
    clock: &crate::time::SharedClock,
    token: &str,
) -> crate::CoreResult<AccessTokenClaims> {
    let keys = caches.jwks.snapshot().await;
    let decoded: crate::jwt::Decoded<AccessTokenClaims> = verify_from_snapshot(&keys, token)?;
    if decoded.claims.exp < clock.now().timestamp() {
        return Err(crate::CoreError::Unauthenticated);
    }
    Ok(decoded.claims)
}

#[cfg(test)]
mod tests;
