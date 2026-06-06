//! Shared test helpers for e2e modules.
//!
//! Lives in its own module file so each themed e2e module
//! (`oidc.rs`, `mfa.rs`, …) can `use super::common::*` without
//! duplicating the boilerplate. Anything used by more than one
//! themed module belongs here; anything used by only one stays
//! co-located with its tests.

#![allow(dead_code)]

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use base64ct::{Base64UrlUnpadded, Encoding};
use sha2::{Digest, Sha256};
use sui_id::config::{Config, LogConfig, ServerConfig, StorageConfig, TokensConfig};
use sui_id::{build_router, AppState};
use sui_id_store::{crypto::MasterKey, Database};
use tower::ServiceExt;

pub const SETUP_TOKEN: &str = "test-setup-token-do-not-use-in-prod";
pub const USERNAME: &str = "alice";
pub const PASSWORD: &str = "alice-the-tester-password";

/// Build a clean test AppState with an `InMemoryMailSender`.
/// Use [`test_app_with_mailer`] when the test needs to inspect
/// what was sent.
pub fn test_app() -> AppState {
    test_app_with_mailer().0
}

/// Like `test_app` but also returns the in-memory mail sender so
/// the caller can assert on captures.
pub fn test_app_with_mailer() -> (AppState, std::sync::Arc<sui_id_core::mail::InMemoryMailSender>) {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open db");
    let cfg = Config {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".into(),
            issuer: "https://idp.test".into(),
            cookie_secure: false,
            trusted_proxies: Vec::new(),
        },
        storage: StorageConfig {
            db_path: "/tmp/unused.sqlite".into(),
            key_file: "/tmp/unused.key".into(),
        },
        tokens: TokensConfig::default(),
        log: LogConfig {
            format: "fmt".into(),
            filter: "off".into(),
            access_log: false,
            file: None,
        },
        security: sui_id::config::SecurityConfig::default(),
    };
    let mailer = std::sync::Arc::new(sui_id_core::mail::InMemoryMailSender::new());
    let mailer_dyn: std::sync::Arc<dyn sui_id_core::mail::MailSender> = mailer.clone();
    // Default HIBP client for tests is a clean (no-breach) in-memory
    // stub. Tests that want to assert breach behaviour use
    // `test_app_with_hibp` instead.
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::test_support::InMemoryHibpClient::new());
    let caches = std::sync::Arc::new(sui_id_core::cache::Caches::new());
    let state = AppState::new(db, cfg, SETUP_TOKEN.into(), mailer_dyn, hibp_client, caches);
    (state, mailer)
}

/// Variant of `test_app_with_mailer` that returns the HIBP stub
/// alongside, so tests can pre-load it with breach plans before
/// running the setup wizard or password change flows.
pub fn test_app_with_hibp() -> (
    AppState,
    std::sync::Arc<sui_id_core::mail::InMemoryMailSender>,
    std::sync::Arc<sui_id_core::hibp::test_support::InMemoryHibpClient>,
) {
    let key = MasterKey::generate();
    let db = Database::open_in_memory(key).expect("open db");
    let cfg = Config {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".into(),
            issuer: "https://idp.test".into(),
            cookie_secure: false,
            trusted_proxies: Vec::new(),
        },
        storage: StorageConfig {
            db_path: "/tmp/unused.sqlite".into(),
            key_file: "/tmp/unused.key".into(),
        },
        tokens: TokensConfig::default(),
        log: LogConfig {
            format: "fmt".into(),
            filter: "off".into(),
            access_log: false,
            file: None,
        },
        security: sui_id::config::SecurityConfig::default(),
    };
    let mailer = std::sync::Arc::new(sui_id_core::mail::InMemoryMailSender::new());
    let mailer_dyn: std::sync::Arc<dyn sui_id_core::mail::MailSender> = mailer.clone();
    let hibp = std::sync::Arc::new(sui_id_core::hibp::test_support::InMemoryHibpClient::new());
    let hibp_dyn: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> = hibp.clone();
    let caches = std::sync::Arc::new(sui_id_core::cache::Caches::new());
    let state = AppState::new(db, cfg, SETUP_TOKEN.into(), mailer_dyn, hibp_dyn, caches);
    (state, mailer, hibp)
}

/// Set the server-settings `hibp_mode` directly. Tests use this
/// to flip between modes without going through the admin settings
/// page.
pub async fn set_hibp_mode(state: &AppState, mode: sui_id_store::models::HibpMode) {
    sui_id_store::repos::server_settings::update_hibp_mode(
        &state.db,
        mode,
        chrono::Utc::now(),
    ).await
    .expect("update hibp_mode");
}

pub async fn read_body(body: Body) -> Vec<u8> {
    to_bytes(body, 64 * 1024).await.expect("body").to_vec()
}

pub fn extract_set_cookie(headers: &http::HeaderMap, name: &str) -> Option<String> {
    for v in headers.get_all(header::SET_COOKIE) {
        let raw = v.to_str().ok()?;
        if let Some(rest) = raw.strip_prefix(&format!("{name}=")) {
            let value = rest.split(';').next()?.to_owned();
            return Some(value);
        }
    }
    None
}

pub fn pkce_pair() -> (String, String) {
    let verifier = "verifier-1234567890-abcdef1234567890-XYZ";
    let digest = Sha256::digest(verifier.as_bytes());
    let mut buf = vec![0u8; 64];
    let n = Base64UrlUnpadded::encode(&digest, &mut buf)
        .map(str::len)
        .expect("encode");
    buf.truncate(n);
    let challenge = String::from_utf8(buf).expect("ascii");
    (verifier.to_owned(), challenge)
}

pub async fn complete_setup_and_login(state: &AppState) -> String {
    let router = build_router(state.clone());
    // The 3-step wizard's form lives at /setup/admin and now
    // requires a matching `confirm_password` field. We don't supply
    // an email — empty is treated as None server-side.
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &display_name=Alice\
         &email=\
         &password={PASSWORD}\
         &confirm_password={PASSWORD}"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/setup/admin")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("setup");
    assert!(
        resp.status() == StatusCode::SEE_OTHER || resp.status() == StatusCode::TEMPORARY_REDIRECT
            || resp.status() == StatusCode::FOUND,
        "expected redirect after setup, got {}",
        resp.status()
    );
    extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie set")
}

/// Create a test client. Default `allowed_scopes` is `"openid profile"` (the
/// server default when the form field is left blank). Use
/// [`create_client_with_scopes`] when a test needs to exercise additional
/// scopes such as `"email"` or `"offline_access"`.
pub async fn create_client(state: &AppState, session_cookie: &str) -> (String, String) {
    // Use empty allowed_scopes so the server uses its own default ("openid profile").
    // This preserves the behaviour of the original helper so existing tests
    // that do not need wider scope continue to work unmodified.
    // Use create_client_with_scopes for tests that need email or offline_access.
    create_client_with_scopes(state, session_cookie, "").await
}

/// Like [`create_client`] but with an explicit `allowed_scopes` policy.
/// Pass a space-separated list; e.g. `"openid profile email"`.
pub async fn create_client_with_scopes(
    state: &AppState,
    session_cookie: &str,
    allowed_scopes: &str,
) -> (String, String) {
    // First, GET the clients page to obtain a CSRF cookie + token.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/clients")
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("clients GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie set on GET");

    // Then POST the form with both the cookie and a matching _csrf field.
    let router = build_router(state.clone());
    let encoded_scopes = urlencode(allowed_scopes);
    let body = format!(
        "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true&allowed_scopes={encoded_scopes}&_csrf={csrf}"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session_cookie}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("create client");
    assert_eq!(resp.status(), StatusCode::OK, "expected 200 from clients page");
    let body_bytes = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body_bytes).to_string();
    // The page surfacesthe new client's id+secret as <span class="code">value</span>.
    let mut codes: Vec<String> = Vec::new();
    let mut rest = html.as_str();
    while let Some(start) = rest.find("class=\"code\">") {
        rest = &rest[start + "class=\"code\">".len()..];
        if let Some(end) = rest.find("</span>") {
            codes.push(rest[..end].to_owned());
            rest = &rest[end..];
        } else {
            break;
        }
    }
    assert!(codes.len() >= 2, "expected client id and secret in HTML, found {codes:?}");
    (codes[0].clone(), codes[1].clone())
}

/// Fetch a fresh CSRF token from any admin GET. The Set-Cookie value is
/// the token; the same value goes in the form's `_csrf` field.
pub async fn fetch_csrf(state: &AppState, session_cookie: &str) -> String {
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin")
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("admin GET");
    extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie on admin GET")
}

pub fn utf8_encode(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// Helper: enable SMTP so forgot-password / reset-password endpoints
/// stop returning 404. Uses the test mailer that's already wired in.
pub async fn enable_smtp(state: &AppState) {
    use chrono::Utc;
    let now = Utc::now();
    sui_id_store::repos::smtp_config::upsert(
        &state.db,
        &sui_id_store::models::SmtpConfigRow {
            enabled: true,
            host: "smtp.test".into(),
            port: 587,
            tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
            username: Some("user".into()),
            password_enc: Some(
                sui_id_store::crypto::seal(
                    state.db.key(),
                    b"smtp-pw",
                    sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
                )
                .expect("seal"),
            ),
            from_address: "noreply@example.test".into(),
            from_name: None,
            base_url: "https://idp.test".into(),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("upsert smtp");
}

/// Extract `_csrf` token from rendered HTML. Used by tests that
/// drive form-POSTed flows where the GET-rendered page contains
/// the token as a hidden input.
pub fn extract_csrf_token(html: &str) -> String {
    let needle = "name=\"_csrf\" value=\"";
    let i = html.find(needle).expect("find _csrf input");
    let start = i + needle.len();
    let rest = &html[start..];
    let end = rest.find('"').expect("end of _csrf value");
    rest[..end].to_owned()
}

// ---------- Helpers used by multiple themed modules ----------

/// Look up a `sui_id_csrf` cookie in a response's `Set-Cookie`
/// headers. Equivalent to `extract_set_cookie(headers, "sui_id_csrf")`
/// but kept as a named helper for readability at call sites that
/// only ever care about CSRF.
pub fn extract_csrf_cookie(headers: &http::HeaderMap) -> Option<String> {
    extract_set_cookie(headers, "sui_id_csrf")
}

/// `application/x-www-form-urlencoded` percent-encoder, identical
/// to the one used by `utf8_encode` but kept under a more
/// idiomatic name for tests that build login form bodies.
pub fn urlencode(s: &str) -> String {
    utf8_encode(s)
}

/// Issue a fresh session for the given username/password by hitting
/// `/admin/login` directly. The bootstrap helper already does the
/// setup wizard; this is the path tests use to add a *parallel*
/// session under the same identity (e.g. for testing concurrent
/// session caps and revoke-others flows).
pub async fn login_again_for_admin(state: &AppState, username: &str, password: &str) -> String {
    let body = format!(
        "username={}&password={}",
        urlencode(username),
        urlencode(password)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("login");
    for v in resp.headers().get_all(header::SET_COOKIE) {
        let s = v.to_str().expect("utf8");
        if let Some(rest) = s.strip_prefix("sui_id_session=") {
            return rest.split(';').next().unwrap_or("").to_owned();
        }
    }
    panic!("login_again_for_admin: no session cookie set");
}

/// Decode a Base32 (RFC 4648, no padding) secret string into raw
/// bytes. Used by MFA tests to recover the TOTP shared secret out
/// of a rendered enrolment page so the test can compute the
/// expected current code.
pub fn decode_b32(s: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out: Vec<u8> = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for c in s.chars() {
        let c = c.to_ascii_uppercase();
        let idx = ALPHABET.iter().position(|&a| a as char == c);
        let v = match idx {
            Some(v) => v as u32,
            None => continue,
        };
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    out
}

/// Drive the `/me/security/mfa/enroll/start` + `/confirm`
/// sequence end-to-end for a logged-in session, returning the
/// (secret_b32, recovery_codes) pair. The caller can use the
/// secret to compute valid TOTP codes for subsequent assertions.
pub async fn enroll_mfa_for(state: &AppState, session: &str) -> (String, Vec<String>) {
    use sui_id_core::totp;

    // Start enrolment.
    let csrf = fetch_csrf(state, session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/me/security/mfa/enroll/start")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("enroll start");
    assert_eq!(resp.status(), StatusCode::OK, "enroll start failed");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    // Pull the Base32 secret out of the page. The Japanese label
    // ("秘密鍵:") plus the next `<span class="code">` wrap the
    // value; the English "Secret:" is rendered the same shape.
    let secret_b32 = {
        let label_at = html
            .find("秘密鍵:")
            .or_else(|| html.find("Secret:"))
            .expect("secret label rendered");
        let rest = &html[label_at..];
        let span_at = rest.find("<span class=\"code\"").expect("secret span");
        let after_open = &rest[span_at..];
        let gt = after_open.find('>').expect("span open close");
        let inner = &after_open[gt + 1..];
        let end = inner.find("</span>").expect("secret end");
        inner[..end].to_owned()
    };
    let secret = decode_b32(&secret_b32);
    let now = chrono::Utc::now().timestamp();
    let step = now / 30;
    let code = totp::code_for_step(&secret, step).await;

    // Confirm.
    let csrf = fetch_csrf(state, session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/me/security/mfa/enroll/confirm")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("enroll confirm");
    assert_eq!(resp.status(), StatusCode::OK, "enroll confirm failed");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    // Pull the recovery codes out of the rendered page (each in <span class="code">).
    let mut codes = Vec::new();
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<li><span class=\"code\">") {
        rest = &rest[start + "<li><span class=\"code\">".len()..];
        if let Some(end) = rest.find("</span>") {
            codes.push(rest[..end].to_owned());
            rest = &rest[end..];
        } else {
            break;
        }
    }
    assert_eq!(codes.len(), 8, "expected 8 recovery codes, got {}: {codes:?}", codes.len());
    (secret_b32, codes)
}
