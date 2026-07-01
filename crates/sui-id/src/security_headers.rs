//! Per-response security headers.
//!
//! Browsers honour several response headers that limit the damage a
//! compromised page or a misbehaving downstream proxy can do. We set
//! them globally on every response — there's no endpoint where a
//! looser policy is correct, and "default-deny" plus a single,
//! reviewed exception list is far easier to audit than per-route
//! decisions.
//!
//! What we set, and why:
//!
//! - `Strict-Transport-Security`: tell the browser this origin is
//!   HTTPS-only. We only emit it when `cookie_secure` is on, which
//!   is the same signal we use elsewhere to mean "we're behind
//!   HTTPS"; emitting it on plain-HTTP development would cause real
//!   problems for the operator's localhost work.
//!
//! - `Content-Security-Policy`: severely restrict where the admin UI
//!   can load anything from. The admin pages are server-rendered
//!   Leptos with one bundled JS file (`/static/webauthn.js`); inline
//!   scripts are not used. `frame-ancestors 'none'` makes clickjacking
//!   on `/oauth2/authorize` impossible — sui-id has no consent screen,
//!   but a logged-in user's authorize redirect could still be staged
//!   inside an attacker iframe to steal codes through the response
//!   redirect, so framing is denied unconditionally.
//!
//! - `X-Frame-Options: DENY`: belt-and-braces alongside
//!   `frame-ancestors 'none'` for older browsers.
//!
//! - `X-Content-Type-Options: nosniff`: no MIME-sniffing. Stops a
//!   browser from guessing that a JSON body is HTML.
//!
//! - `Referrer-Policy: strict-origin-when-cross-origin`: outbound
//!   navigation away from sui-id (e.g. RP redirects after logout)
//!   leaks only the origin, never the path. The `Authorization` /
//!   `code` parameters live in the path/query of internal URLs, so
//!   keeping them out of cross-origin Referer values is a real win.
//!
//! - `Permissions-Policy`: disable browser feature APIs that sui-id
//!   has no use for. A compromised page asking for the camera or
//!   geolocation would be denied at the browser before the user is
//!   even prompted.
//!
//! Headers we don't set, and why:
//!
//! - `X-XSS-Protection`: deprecated, replaced by `Content-Security-
//!   Policy`. Modern browsers ignore it; some older browsers do
//!   harmful things if it's set.
//!
//! - `Cross-Origin-Resource-Policy` / `Cross-Origin-Opener-Policy`:
//!   useful for sites that mix cross-origin scripts; sui-id doesn't,
//!   so adding them buys nothing today and risks breaking the OIDC
//!   flow for an SPA RP that fetches our public endpoints.

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;

/// The CSP we emit. WebAuthn registration / authentication needs the
/// JS bundle to be loadable; we serve it from `/static/webauthn.js`,
/// hence `script-src 'self'`. No inline scripts and no remote
/// scripts. `connect-src 'self'` is for the WebAuthn JS doing fetch
/// back to sui-id.
const CSP: &str = "\
default-src 'self'; \
script-src 'self'; \
style-src 'self' 'unsafe-inline'; \
img-src 'self' data:; \
font-src 'self'; \
connect-src 'self'; \
frame-ancestors 'none'; \
base-uri 'self'; \
form-action 'self'; \
object-src 'none'";

/// Permissions-Policy disables every browser feature API we don't
/// use. Camera, geolocation, microphone, payment, USB are all things
/// an OIDC IdP has no business asking for; if a compromised page
/// requests them the browser refuses *before* prompting the user.
const PERMISSIONS_POLICY: &str = "\
accelerometer=(), \
camera=(), \
geolocation=(), \
gyroscope=(), \
magnetometer=(), \
microphone=(), \
payment=(), \
usb=()";

/// HSTS lifetime: two years, with `includeSubDomains` so a careless
/// subdomain CNAMEd at the issuer host can't be used as a downgrade
/// foothold. We *don't* set `preload` — that requires a deliberate
/// commitment by the operator and shouldn't be a default.
const HSTS: &str = "max-age=63072000; includeSubDomains";

const X_FRAME_OPTIONS: HeaderName = HeaderName::from_static("x-frame-options");
const X_CONTENT_TYPE_OPTIONS: HeaderName = HeaderName::from_static("x-content-type-options");
const REFERRER_POLICY: HeaderName = HeaderName::from_static("referrer-policy");
const PERMISSIONS_POLICY_HDR: HeaderName = HeaderName::from_static("permissions-policy");

/// Whether to emit `Strict-Transport-Security`. Operators set this
/// to true via `server.cookie_secure = true`, which is also the
/// "we're behind HTTPS" flag elsewhere — bundling the two is fine
/// in practice and keeps configuration small.
#[derive(Clone, Copy)]
pub struct SecurityHeaderConfig {
    pub enable_hsts: bool,
}

pub async fn middleware(
    axum::extract::State(cfg): axum::extract::State<SecurityHeaderConfig>,
    req: Request,
    next: Next,
) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();

    // Don't overwrite headers a handler already set deliberately.
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        if let Ok(v) = HeaderValue::from_str(CSP) {
            h.insert(header::CONTENT_SECURITY_POLICY, v);
        }
    }
    if !h.contains_key(&X_FRAME_OPTIONS) {
        h.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    }
    if !h.contains_key(&X_CONTENT_TYPE_OPTIONS) {
        h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    }
    if !h.contains_key(&REFERRER_POLICY) {
        h.insert(
            REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
    }
    if !h.contains_key(&PERMISSIONS_POLICY_HDR) {
        if let Ok(v) = HeaderValue::from_str(PERMISSIONS_POLICY) {
            h.insert(PERMISSIONS_POLICY_HDR, v);
        }
    }
    if cfg.enable_hsts && !h.contains_key(header::STRICT_TRANSPORT_SECURITY) {
        if let Ok(v) = HeaderValue::from_str(HSTS) {
            h.insert(header::STRICT_TRANSPORT_SECURITY, v);
        }
    }

    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    fn app(enable_hsts: bool) -> Router {
        Router::new().route("/", get(|| async { "ok" })).layer(
            axum::middleware::from_fn_with_state(SecurityHeaderConfig { enable_hsts }, middleware),
        )
    }

    #[tokio::test]
    async fn baseline_headers_are_set() {
        let resp = app(false)
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let h = resp.headers();
        assert!(h.contains_key(header::CONTENT_SECURITY_POLICY));
        assert_eq!(
            h.get(&X_FRAME_OPTIONS).map(|v| v.to_str().unwrap()),
            Some("DENY")
        );
        assert_eq!(
            h.get(&X_CONTENT_TYPE_OPTIONS).map(|v| v.to_str().unwrap()),
            Some("nosniff")
        );
        assert!(
            h.get(&REFERRER_POLICY)
                .map(|v| v.to_str().unwrap())
                .unwrap_or("")
                .contains("strict-origin")
        );
        assert!(h.contains_key(&PERMISSIONS_POLICY_HDR));
        // HSTS is *not* set when enable_hsts=false.
        assert!(!h.contains_key(header::STRICT_TRANSPORT_SECURITY));
    }

    #[tokio::test]
    async fn hsts_set_only_when_enabled() {
        let resp = app(true)
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let h = resp.headers();
        let v = h
            .get(header::STRICT_TRANSPORT_SECURITY)
            .map(|v| v.to_str().unwrap())
            .unwrap_or("");
        assert!(v.contains("max-age="));
        assert!(v.contains("includeSubDomains"));
    }

    #[tokio::test]
    async fn handler_set_headers_are_not_overwritten() {
        let app = Router::new()
            .route(
                "/",
                get(|| async {
                    let mut r = axum::response::Response::new(Body::from("ok"));
                    r.headers_mut().insert(
                        header::CONTENT_SECURITY_POLICY,
                        HeaderValue::from_static("custom-policy"),
                    );
                    r
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                SecurityHeaderConfig { enable_hsts: false },
                middleware,
            ));
        let resp = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .map(|v| v.to_str().unwrap()),
            Some("custom-policy")
        );
    }
}
