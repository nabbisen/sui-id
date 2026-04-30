//! CORS for the OIDC public endpoints.
//!
//! Only a handful of routes legitimately want cross-origin access, and
//! each one calls for a different policy:
//!
//! | Route | Policy | Why |
//! |-------|--------|-----|
//! | `/.well-known/openid-configuration` | `*` | Public metadata. SPA OIDC libraries fetch it from the RP origin. Anyone reading it sees only what we'd serve to anyone else. |
//! | `/.well-known/jwks.json`            | `*` | Public keys. Same reasoning as discovery. |
//! | `/oauth2/userinfo`                  | `*` | Bearer-authenticated; CORS doesn't carry credentials with `*`, but `Authorization` is set explicitly by the SPA so it works regardless. |
//! | `/oauth2/token`                     | client redirect_uri origin allowlist | A public client running entirely in the browser exchanges a code via fetch. We allow only the origins of registered redirect_uris so that one client's CORS configuration cannot be abused to grind tokens from another. |
//! | `/oauth2/introspection` / `/oauth2/revocation` | none | Server-to-server. CORS not relevant; not setting a header is the right answer. |
//! | `/oauth2/authorize` / `/oauth2/logout`         | none | Top-level redirects, not fetched. CORS does not apply. |
//! | `/admin/*`                                     | none | Same-origin admin UI. |
//!
//! This module exposes one middleware per policy class. The router
//! attaches them route-by-route rather than as a global layer.

use axum::extract::Request;
use axum::http::{header, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;

const ACAO: header::HeaderName = header::ACCESS_CONTROL_ALLOW_ORIGIN;
const ACAM: header::HeaderName = header::ACCESS_CONTROL_ALLOW_METHODS;
const ACAH: header::HeaderName = header::ACCESS_CONTROL_ALLOW_HEADERS;
const ACMA: header::HeaderName = header::ACCESS_CONTROL_MAX_AGE;
const ORIGIN: header::HeaderName = header::ORIGIN;
const VARY: header::HeaderName = header::VARY;

/// Public-read CORS: `Access-Control-Allow-Origin: *`. Suitable for
/// endpoints whose body is fully public (discovery, JWKS) or whose
/// authentication is via `Authorization` header rather than cookies
/// (userinfo). Preflight responses are answered for `GET` only.
pub async fn public_read(req: Request, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        return preflight_response("GET, OPTIONS", "Authorization, Content-Type", "*");
    }
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(ACAO, HeaderValue::from_static("*"));
    // No Vary: Origin needed since we always answer with `*`.
    resp
}

/// Token-endpoint CORS: allow only the origins of registered
/// redirect_uris on *some* client. Computed at request time from the
/// `Origin` header against the database of clients. Returns no CORS
/// headers at all when the origin doesn't match a registered URI —
/// the browser will refuse the response, which is the correct
/// outcome for a non-browser-resident client.
///
/// Implemented as a closure factory rather than a plain function so
/// the router can wire it with `from_fn_with_state(state, ...)`.
pub async fn token_endpoint(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    req: Request,
    next: Next,
) -> Response {
    let origin = req
        .headers()
        .get(&ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    if req.method() == Method::OPTIONS {
        // For preflight we have to decide *now* whether to allow the
        // origin. We could be permissive for OPTIONS (it carries no
        // credentials), but echoing an origin we won't honour on the
        // POST is misleading; align with the actual policy.
        if let Some(o) = &origin {
            if origin_matches_any_redirect_uri(&state, o) {
                return preflight_response_with_origin(
                    "POST, OPTIONS",
                    "Authorization, Content-Type",
                    o,
                );
            }
        }
        // No matching origin: respond 204 with no CORS headers. The
        // browser will refuse the upcoming POST. A non-browser caller
        // (e.g. curl) won't have sent OPTIONS in the first place.
        return preflight_response_no_cors();
    }

    let mut resp = next.run(req).await;
    if let Some(o) = origin {
        if origin_matches_any_redirect_uri(&state, &o) {
            if let Ok(v) = HeaderValue::from_str(&o) {
                resp.headers_mut().insert(ACAO, v);
                // Caches must vary on Origin since the response is
                // origin-specific.
                append_vary(resp.headers_mut(), "Origin");
            }
        }
    }
    resp
}

fn origin_matches_any_redirect_uri(state: &crate::AppState, origin: &str) -> bool {
    // Pull every active client's redirect URIs and ask whether any
    // shares the supplied origin (scheme + host + port).
    //
    // For the foreseeable scale of a self-hosted IdP this is a few
    // dozen rows at most; an in-memory cache is unwarranted and
    // would only complicate the consistency story when an admin
    // adds or removes a URI. If this ever shows up in profiles,
    // cache with a short TTL.
    let target = parse_origin(origin);
    let target = match target {
        Some(t) => t,
        None => return false,
    };
    let clients = match sui_id_store::repos::clients::list(&state.db) {
        Ok(c) => c,
        Err(_) => return false,
    };
    for client in clients {
        if client.is_disabled || client.is_deleted {
            continue;
        }
        for uri in client.redirect_uris {
            if let Some(o) = parse_origin(&uri) {
                if o == target {
                    return true;
                }
            }
        }
    }
    false
}

/// `(scheme, host, port)` triple. Two URLs share an *origin* iff
/// these three pieces match; path/query are not part of an origin.
/// We avoid pulling in `url::Url` here — it's already a transitive
/// dep but parsing here is light enough to do by hand and keeps the
/// CORS surface entirely free of URL-equivalence subtleties.
fn parse_origin(s: &str) -> Option<(String, String, Option<u16>)> {
    // scheme://authority/...
    let (scheme, rest) = s.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(n) => (h.to_ascii_lowercase(), Some(n)),
            Err(_) => (authority.to_ascii_lowercase(), None),
        },
        None => (authority.to_ascii_lowercase(), None),
    };
    Some((scheme, host, port))
}

fn append_vary(h: &mut axum::http::HeaderMap, value: &str) {
    let existing = h
        .get(&VARY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let merged = if existing.is_empty() {
        value.to_owned()
    } else if existing.split(',').any(|s| s.trim().eq_ignore_ascii_case(value)) {
        existing
    } else {
        format!("{existing}, {value}")
    };
    if let Ok(v) = HeaderValue::from_str(&merged) {
        h.insert(VARY, v);
    }
}

fn preflight_response(allow_methods: &str, allow_headers: &str, origin: &str) -> Response {
    let mut resp = Response::new(axum::body::Body::empty());
    *resp.status_mut() = axum::http::StatusCode::NO_CONTENT;
    let h = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(origin) {
        h.insert(ACAO, v);
    }
    if let Ok(v) = HeaderValue::from_str(allow_methods) {
        h.insert(ACAM, v);
    }
    if let Ok(v) = HeaderValue::from_str(allow_headers) {
        h.insert(ACAH, v);
    }
    h.insert(ACMA, HeaderValue::from_static("600"));
    resp
}

fn preflight_response_with_origin(
    allow_methods: &str,
    allow_headers: &str,
    origin: &str,
) -> Response {
    let resp = preflight_response(allow_methods, allow_headers, origin);
    let mut resp = resp;
    append_vary(resp.headers_mut(), "Origin");
    resp
}

fn preflight_response_no_cors() -> Response {
    let mut resp = Response::new(axum::body::Body::empty());
    *resp.status_mut() = axum::http::StatusCode::NO_CONTENT;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_origin_reads_scheme_host_port() {
        assert_eq!(
            parse_origin("https://app.example.com/cb"),
            Some(("https".into(), "app.example.com".into(), None))
        );
        assert_eq!(
            parse_origin("http://localhost:3000/cb?x=1"),
            Some(("http".into(), "localhost".into(), Some(3000)))
        );
    }

    #[test]
    fn parse_origin_rejects_non_http_schemes() {
        assert_eq!(parse_origin("ftp://x"), None);
        assert_eq!(parse_origin("javascript:alert(1)"), None);
        assert_eq!(parse_origin("file:///etc/passwd"), None);
    }

    #[test]
    fn parse_origin_lowercases_host_and_scheme() {
        // Origins are case-insensitive on scheme and host but the
        // browser-supplied `Origin` header tends to be lower-case.
        // Our equality check is case-sensitive on the parsed tuple,
        // so normalise both sides.
        let a = parse_origin("HTTPS://Example.COM/x");
        let b = parse_origin("https://example.com/y");
        assert_eq!(a, b);
    }

    #[test]
    fn parse_origin_rejects_empty_authority() {
        assert_eq!(parse_origin("https:///path"), None);
    }
}
