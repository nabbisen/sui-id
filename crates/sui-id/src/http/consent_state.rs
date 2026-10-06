//! The state a consent screen carries to its own `POST` (RFC 120 D3).
//!
//! `GET /oauth2/authorize` validates a request, finds that the user must be
//! asked, and renders a screen. The `POST` that answers it needs the request it
//! is answering. That request travels in a cookie, so the cookie is the one
//! place a value the server did not write could be presented as one it did.
//!
//! Three properties, each asserted by a test:
//!
//! - **It carries request parameters and no identity.** The subject of the
//!   decision and the authentication methods of the resulting code come from
//!   the session that answers, never from this value (RFC 120 D1, D2).
//! - **It is integrity-protected.** The value is `base64url(payload).
//!   base64url(tag)`, the tag an HMAC-SHA256 under a key derived from the
//!   master key for this purpose alone. A value that does not verify is not
//!   parsed.
//! - **It is bound to the session that received it.** The session id is part
//!   of what the tag covers and is not part of the value, so a value issued to
//!   one session verifies under no other, and a signed-out browser holds
//!   nothing that verifies at all.
//!
//! It expires ([`LIFETIME_SECS`]) and is scoped to the path that reads it
//! ([`COOKIE_PATH`]).
//!
//! **Not enforced: single use.** A value can be presented again until it
//! expires, by the session it was bound to, and yields the same request for
//! the same user, so a repeat only repeats what that user already approved.
//! Single use would need server-side state, which is the alternative this
//! design was chosen over (see the RFC 120 review package).

use axum_extra::extract::cookie::{Cookie, SameSite};
use base64ct::{Base64UrlUnpadded, Encoding};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sui_id_shared::ids::SessionId;
use sui_id_store::crypto::MasterKey;

/// Name of the cookie.
pub const COOKIE: &str = "sui_id_consent";

/// The only path the browser sends the cookie to.
pub const COOKIE_PATH: &str = "/oauth2/consent";

/// How long an issued value verifies, in seconds.
pub const LIFETIME_SECS: i64 = 300;

/// Domain-separation label for the derived key. Changing it invalidates every
/// value in flight, which only costs a user one restart of the consent screen.
const KEY_LABEL: &[u8] = b"sui-id/consent-state/v1";

type HmacSha256 = Hmac<Sha256>;

/// The authorization request a consent screen was rendered for. Nothing here
/// identifies a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Serialize, Deserialize)]
struct Payload {
    /// Unix time after which the value does not verify.
    exp: i64,
    req: ConsentRequest,
}

fn mac(key: &MasterKey, session: SessionId) -> HmacSha256 {
    // The key for this purpose is a PRF of the master key under a label, so the
    // master key itself never keys a MAC directly and no other purpose can
    // produce the same key.
    #[allow(clippy::expect_used)]
    let mut kdf = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
    kdf.update(KEY_LABEL);
    let subkey = kdf.finalize().into_bytes();
    #[allow(clippy::expect_used)]
    let mut mac = HmacSha256::new_from_slice(&subkey).expect("HMAC accepts any key length");
    // The session id is authenticated but not carried: length-prefixed so it
    // cannot be confused with the start of the payload.
    let session = session.to_string();
    mac.update(&(session.len() as u64).to_be_bytes());
    mac.update(session.as_bytes());
    mac
}

/// The `Set-Cookie` that carries `value`: scoped to [`COOKIE_PATH`], out of
/// script's reach, `Secure` under the operator's `cookie_secure`, and gone
/// after [`LIFETIME_SECS`] whether or not the server is asked.
pub fn cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(COOKIE, value);
    c.set_path(COOKIE_PATH);
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie::time::Duration::seconds(LIFETIME_SECS));
    c
}

/// The `Set-Cookie` that removes the cookie, at the path it was set for.
pub fn cleared<'a>(secure: bool) -> Cookie<'a> {
    let mut c = cookie(String::new(), secure);
    c.set_max_age(cookie::time::Duration::seconds(0));
    c
}

/// Produce the cookie value for `req`, bound to `session`, valid from `now`
/// (unix seconds) for [`LIFETIME_SECS`].
pub fn seal(key: &MasterKey, session: SessionId, now: i64, req: &ConsentRequest) -> String {
    let payload = Payload {
        exp: now + LIFETIME_SECS,
        req: req.clone(),
    };
    // Serialising plain strings and integers cannot fail.
    #[allow(clippy::expect_used)]
    let json = serde_json::to_vec(&payload).expect("consent payload serialises");
    let body = Base64UrlUnpadded::encode_string(&json);
    let mut m = mac(key, session);
    m.update(body.as_bytes());
    let tag = Base64UrlUnpadded::encode_string(&m.finalize().into_bytes());
    format!("{body}.{tag}")
}

/// The request in `value`, if `value` was produced by [`seal`] under this key
/// for this `session` and has not expired at `now`. Anything else is `None`,
/// and nothing in a value that fails verification is parsed.
pub fn open(key: &MasterKey, session: SessionId, now: i64, value: &str) -> Option<ConsentRequest> {
    let (body, tag) = value.split_once('.')?;
    let tag = Base64UrlUnpadded::decode_vec(tag).ok()?;
    let mut m = mac(key, session);
    m.update(body.as_bytes());
    // `verify_slice` compares in constant time.
    m.verify_slice(&tag).ok()?;
    let json = Base64UrlUnpadded::decode_vec(body).ok()?;
    let payload: Payload = serde_json::from_slice(&json).ok()?;
    if now > payload.exp {
        return None;
    }
    Some(payload.req)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "consent_state/tests.rs"]
mod tests;
