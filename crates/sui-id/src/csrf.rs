//! CSRF protection for admin forms (synchronizer token pattern).
//!
//! ## Threat being addressed
//!
//! `SameSite=Lax` cookies already block the textbook CSRF attack (a
//! malicious site issuing a top-level POST that smuggles the user's
//! session cookie). This is enough today, but is brittle: any future
//! routing change that introduces a same-site cross-origin POST, or any
//! browser regression, would silently lose the property. We add a real
//! synchronizer token so the CSRF defence does not depend on cookie
//! attributes alone.
//!
//! ## Mechanism
//!
//! On every admin response, sui-id sets a `sui_id_csrf` cookie containing
//! a 32-byte random token. The same token is embedded as a hidden
//! `_csrf` field in every form rendered by `sui-id-web`. On a state-
//! changing POST under `/admin/`, sui-id reads both values and compares
//! them in constant time. A missing or mismatching token returns
//! 403 Forbidden.
//!
//! ## Cookie attributes
//!
//! `sui_id_csrf` is **not** `HttpOnly`. The form-rendering layer needs to
//! be able to write the value into a hidden field. The session cookie
//! that actually authenticates the request stays HttpOnly; the CSRF
//! cookie alone is useless to an attacker because it is also expected
//! in the form body.
//!
//! `SameSite=Lax` is set on both. `Secure` follows the operator's
//! `cookie_secure` config, the same as the session cookie.

use crate::handlers::AppStateExt;
use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64ct::{Base64UrlUnpadded, Encoding};
use rand::{rngs::OsRng, RngCore};
use subtle::ConstantTimeEq;

/// Cookie name used to carry the CSRF token alongside the session.
pub const CSRF_COOKIE: &str = "sui_id_csrf";

/// Form field name expected to echo the cookie's value.
pub const CSRF_FIELD: &str = "_csrf";

/// Generate a fresh 32-byte URL-safe token.
pub fn new_token() -> String {
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let mut out = vec![0u8; 64];
    let n = Base64UrlUnpadded::encode(&buf, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).expect("base64url is ascii")
}

/// Return the existing CSRF token from the jar, or mint a new one if the
/// cookie is missing. Used by every admin GET handler so that the form
/// it renders has a token available.
pub fn ensure_token(jar: &CookieJar) -> String {
    if let Some(c) = jar.get(CSRF_COOKIE) {
        let v = c.value();
        if !v.is_empty() {
            return v.to_owned();
        }
    }
    new_token()
}

/// Build the `Set-Cookie` for a CSRF token.
pub fn csrf_cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(CSRF_COOKIE, value);
    c.set_path("/");
    // NOT HttpOnly: the rendering layer needs to read it. The cookie
    // alone has no authority — only when paired with a matching form
    // field on a session-authenticated request does it grant anything.
    c.set_http_only(false);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c
}

/// Extract and validate the CSRF token for a state-changing admin POST.
///
/// The form body must contain `_csrf` and the cookie jar must contain a
/// matching `sui_id_csrf`. Comparison is constant-time. Returns the
/// caller's existing token on success (the same token can be reused for
/// any subsequent forms in this session) or `None` on failure.
pub fn check_token(jar: &CookieJar, form_field: Option<&str>) -> Option<String> {
    let cookie_value = jar.get(CSRF_COOKIE)?.value().to_owned();
    let provided = form_field?;
    if cookie_value.is_empty() || provided.is_empty() {
        return None;
    }
    if cookie_value
        .as_bytes()
        .ct_eq(provided.as_bytes())
        .into()
    {
        Some(cookie_value)
    } else {
        None
    }
}

/// Convenience helper for handlers: pull the form's `_csrf` field out of
/// a `serde_urlencoded`-decoded body. Returns `None` if absent.
pub fn extract_field<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<&'a str> {
    for (k, v) in pairs {
        if k == CSRF_FIELD {
            return Some(v);
        }
    }
    None
}

/// Verify the CSRF token from `headers` (cookie) against the
/// already-parsed `form_token` (the `_csrf` field of the form body), and
/// return a refreshed cookie value to set on the response. On failure,
/// return `None`; the caller should produce a 403.
pub fn verify_with_headers(headers: &HeaderMap, form_token: Option<&str>) -> Option<String> {
    let jar = CookieJar::from_headers(headers);
    check_token(&jar, form_token)
}

/// Re-export for handlers that need to silence the unused-import warning
/// when this module is only used through one entry point.
#[allow(dead_code)]
pub fn _ensure_state_arg(_: &AppStateExt) {}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn jar_with_cookie(value: &str) -> CookieJar {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("{CSRF_COOKIE}={value}")).expect("static"),
        );
        CookieJar::from_headers(&headers)
    }

    #[test]
    fn new_token_has_expected_format() {
        let t = new_token();
        // Base64URL no-pad of 32 bytes is 43 chars.
        assert_eq!(t.len(), 43);
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn ensure_token_reuses_existing_value() {
        let jar = jar_with_cookie("abc-existing");
        assert_eq!(ensure_token(&jar), "abc-existing");
    }

    #[test]
    fn ensure_token_mints_new_value_when_cookie_missing() {
        let jar = CookieJar::from_headers(&HeaderMap::new());
        let t = ensure_token(&jar);
        assert_eq!(t.len(), 43);
    }

    #[test]
    fn check_token_accepts_matching_pair() {
        let jar = jar_with_cookie("the-secret");
        assert_eq!(check_token(&jar, Some("the-secret")).as_deref(), Some("the-secret"));
    }

    #[test]
    fn check_token_rejects_mismatch() {
        let jar = jar_with_cookie("the-secret");
        assert!(check_token(&jar, Some("not-the-secret")).is_none());
    }

    #[test]
    fn check_token_rejects_missing_cookie() {
        let jar = CookieJar::from_headers(&HeaderMap::new());
        assert!(check_token(&jar, Some("anything")).is_none());
    }

    #[test]
    fn check_token_rejects_missing_form_field() {
        let jar = jar_with_cookie("the-secret");
        assert!(check_token(&jar, None).is_none());
    }

    #[test]
    fn check_token_rejects_empty_strings() {
        let jar = jar_with_cookie("");
        assert!(check_token(&jar, Some("")).is_none());
    }
}
