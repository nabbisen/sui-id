//! End-to-end test of the full OIDC flow against the in-process router.
//!
//! Boots an `AppState` with an in-memory SQLite database, completes setup,
//! registers a client, drives an Authorization Code + PKCE flow, exchanges
//! the code, calls userinfo with the resulting Bearer token, and rotates a
//! refresh token. Negative cases verify that PKCE failure, redirect-uri
//! mismatch, and replayed codes are rejected.

use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use base64ct::{Base64UrlUnpadded, Encoding};
use sha2::{Digest, Sha256};
use sui_id::config::{Config, LogConfig, ServerConfig, StorageConfig, TokensConfig};
use sui_id::{build_router, AppState};
use sui_id_store::{crypto::MasterKey, Database};
use tower::ServiceExt;
use url::Url;

const SETUP_TOKEN: &str = "test-setup-token-do-not-use-in-prod";
const USERNAME: &str = "alice";
const PASSWORD: &str = "alice-the-tester-password";

/// Build a clean test AppState with an `InMemoryMailSender`.
/// Use [`test_app_with_mailer`] when the test needs to inspect
/// what was sent.
fn test_app() -> AppState {
    test_app_with_mailer().0
}

/// Like `test_app` but also returns the in-memory mail sender so
/// the caller can assert on captures.
fn test_app_with_mailer() -> (AppState, std::sync::Arc<sui_id_core::mail::InMemoryMailSender>) {
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
    let state = AppState::new(db, cfg, SETUP_TOKEN.into(), mailer_dyn, hibp_client);
    (state, mailer)
}

async fn read_body(body: Body) -> Vec<u8> {
    to_bytes(body, 64 * 1024).await.expect("body").to_vec()
}

fn extract_set_cookie(headers: &http::HeaderMap, name: &str) -> Option<String> {
    for v in headers.get_all(header::SET_COOKIE) {
        let raw = v.to_str().ok()?;
        if let Some(rest) = raw.strip_prefix(&format!("{name}=")) {
            let value = rest.split(';').next()?.to_owned();
            return Some(value);
        }
    }
    None
}

fn pkce_pair() -> (String, String) {
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

async fn complete_setup_and_login(state: &AppState) -> String {
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

async fn create_client(state: &AppState, session_cookie: &str) -> (String, String) {
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
    let body = format!(
        "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true&_csrf={csrf}"
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
    // The page surfaces the new client's id+secret as <span class="code">value</span>.
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
async fn fetch_csrf(state: &AppState, session_cookie: &str) -> String {
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

#[tokio::test]
async fn full_flow_setup_authorize_token_userinfo_refresh() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // /authorize - should redirect with ?code=...
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    assert!(resp.status().is_redirection(), "expected redirect, got {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    let parsed = Url::parse(&location).expect("absolute redirect");
    assert_eq!(parsed.host_str(), Some("rp.test"));
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");

    // /token authorization_code grant
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK, "/token should succeed");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access = json["access_token"].as_str().expect("access_token").to_owned();
    let refresh = json["refresh_token"].as_str().expect("refresh_token").to_owned();
    assert!(json["id_token"].is_string(), "openid scope should yield id_token");
    assert_eq!(json["token_type"].as_str(), Some("Bearer"));

    // /userinfo with the bearer access token
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let info: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(info["preferred_username"].as_str(), Some(USERNAME));

    // refresh_token grant
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh");
    assert_eq!(resp.status(), StatusCode::OK, "refresh should succeed");
    let body_bytes = read_body(resp.into_body()).await;
    let json2: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let new_refresh = json2["refresh_token"].as_str().expect("rotated refresh");
    assert_ne!(new_refresh, refresh, "refresh tokens must rotate");

    // Old refresh must now be rejected.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("replay");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "replayed refresh must fail");
}

#[tokio::test]
async fn pkce_mismatch_is_rejected() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let (_verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned()).expect("code");

    // Use a *wrong* verifier.
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier=not-the-right-verifier-at-all-x"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn redirect_uri_mismatch_is_rejected_at_authorize() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _) = create_client(&state, &session).await;

    let (_v, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Fattacker.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    // Should NOT be a redirect to the attacker.
    if resp.status().is_redirection() {
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            !loc.contains("attacker.test"),
            "redirect must not point at attacker.test, got {loc}"
        );
    } else {
        // 400-class response also acceptable.
        assert!(resp.status().is_client_error(), "expected error, got {}", resp.status());
    }
}

#[tokio::test]
async fn discovery_advertises_only_supported_features() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let algs = json["id_token_signing_alg_values_supported"]
        .as_array()
        .expect("array");
    assert!(algs.iter().any(|v| v.as_str() == Some("EdDSA")));
    assert!(!algs.iter().any(|v| v.as_str() == Some("RS256")));
}

#[tokio::test]
async fn healthz_returns_ok_and_does_not_leak_state() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(json["status"].as_str(), Some("ok"));
    // Must NOT leak whether the system is initialized, who is logged in, etc.
    assert!(json.get("initialized").is_none());
    assert!(json.get("user_count").is_none());
}

#[tokio::test]
async fn login_rate_limit_returns_429_with_retry_after() {
    let state = test_app();
    let _session = complete_setup_and_login(&state).await;

    // The login limiter is configured to 10 requests per 60 seconds. Burn
    // through the budget with deliberately wrong credentials.
    for i in 0..10 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong"))
            .expect("req");
        let resp = router.oneshot(req).await.expect("login");
        assert_ne!(
            resp.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "request {i} unexpectedly throttled"
        );
    }
    // The eleventh attempt should be throttled with 429 + Retry-After.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=wrong"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login throttled");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry = resp.headers().get(header::RETRY_AFTER).expect("retry-after header");
    let secs: i64 = retry.to_str().expect("ascii").parse().expect("integer");
    assert!(secs > 0 && secs <= 60);
}

#[tokio::test]
async fn gc_purges_expired_auth_codes() {
    use chrono::{Duration, Utc};
    use sui_id_shared::ids::{ClientId, UserId};
    use sui_id_store::models::AuthorizationCodeRow;
    use sui_id_store::repos::auth_codes;

    let state = test_app();

    // Insert a code that already expired one minute ago.
    let row = AuthorizationCodeRow {
        code_hash: "deadbeef".repeat(8),
        client_id: ClientId::new(),
        user_id: UserId::new(),
        redirect_uri: "https://rp.test/cb".into(),
        scope: "openid".into(),
        nonce: None,
        code_challenge: "x".into(),
        code_challenge_method: "S256".into(),
        expires_at: Utc::now() - Duration::minutes(1),
        consumed: false,
        created_at: Utc::now() - Duration::minutes(2),
        auth_methods: vec![],
    };
    auth_codes::insert(&state.db, &row).expect("insert");

    // Confirm the row exists before GC.
    let count_before: i64 = state
        .db
        .with_conn(|conn| {
            Ok(conn
                .query_row("SELECT COUNT(*) FROM auth_codes", [], |r| r.get(0))
                .expect("count"))
        })
        .expect("query");
    assert!(count_before >= 1);

    sui_id::gc::run_once(&state);

    let count_after: i64 = state
        .db
        .with_conn(|conn| {
            Ok(conn
                .query_row("SELECT COUNT(*) FROM auth_codes", [], |r| r.get(0))
                .expect("count"))
        })
        .expect("query");
    assert_eq!(count_after, 0, "expired auth code should have been GCed");
}

#[tokio::test]
async fn logout_with_id_token_hint_revokes_session_and_redirects() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Drive an authorization to obtain an id_token.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned()).expect("code");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let id_token = json["id_token"].as_str().expect("id_token").to_owned();

    let logout_url = format!(
        "/oauth2/logout?id_token_hint={}&post_logout_redirect_uri=https%3A%2F%2Frp.test%2Fcb&state=xyz",
        utf8_encode(&id_token)
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&logout_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with("https://rp.test/cb"),
        "should redirect back to RP: {location}"
    );
    assert!(location.contains("state=xyz"));
    let set_cookie = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("sui_id_session="))
        .expect("session cookie cleared");
    assert!(set_cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn logout_rejects_unregistered_post_redirect() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    let logout_url = format!(
        "/oauth2/logout?client_id={client_id}&post_logout_redirect_uri=https%3A%2F%2Fattacker.test%2F"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&logout_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    if resp.status().is_redirection() {
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(!loc.contains("attacker.test"));
    } else {
        assert!(resp.status().is_success());
    }
}

#[tokio::test]
async fn discovery_advertises_end_session_endpoint() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let ese = json["end_session_endpoint"]
        .as_str()
        .expect("end_session_endpoint");
    assert!(ese.ends_with("/oauth2/logout"), "{ese}");
}

#[tokio::test]
async fn signing_key_rotation_publishes_both_keys_in_jwks() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Initially JWKS has exactly one key (created during setup).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let keys_before = json["keys"].as_array().expect("keys").len();
    assert_eq!(keys_before, 1);
    let kid_before = json["keys"][0]["kid"].as_str().expect("kid").to_owned();

    // Rotate via the admin endpoint.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection(), "expected redirect, got {}", resp.status());

    // After rotation, JWKS should publish two keys: the new active one
    // plus the retired previous one (grace window).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks 2");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let keys_after: Vec<_> = json["keys"].as_array().expect("keys").iter().collect();
    assert_eq!(keys_after.len(), 2, "JWKS should publish active + retired");
    let kids: Vec<&str> = keys_after
        .iter()
        .filter_map(|k| k["kid"].as_str())
        .collect();
    assert!(kids.contains(&kid_before.as_str()), "old kid {kid_before} should still be present");

    // The active row should be the *newer* one — verified by checking that
    // the store reports a different active kid than before.
    let active = sui_id_store::repos::signing_keys::active(&state.db).expect("active");
    assert_ne!(active.id.to_string(), kid_before);
}

#[tokio::test]
async fn rotation_does_not_break_existing_authorization_flow() {
    // Pre-rotation: register a client and grab a token. Rotate. The
    // already-issued token should still verify (its kid is still in JWKS).
    // A *new* exchange should produce a token signed with the new key.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned()).expect("code");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access_old = json["access_token"].as_str().expect("access_token").to_owned();

    // Rotate.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection());

    // The pre-rotation access token should still verify against /userinfo.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access_old}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK, "old token should still verify in grace window");
}

#[tokio::test]
async fn cannot_delete_active_signing_key() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let active = sui_id_store::repos::signing_keys::active(&state.db).expect("active");
    let active_id = active.id.to_string();

    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{active_id}/delete"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("delete attempt");
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Active key should still exist.
    let still_active =
        sui_id_store::repos::signing_keys::active(&state.db).expect("still active");
    assert_eq!(still_active.id.to_string(), active_id);
}

#[tokio::test]
async fn delete_retired_signing_key_drops_it_from_jwks() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // First key (created during setup), then rotate to retire it.
    let original_id = sui_id_store::repos::signing_keys::active(&state.db)
        .expect("active")
        .id
        .to_string();
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection());

    // Now delete the retired (original) one.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{original_id}/delete"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("delete");
    assert!(resp.status().is_redirection() || resp.status().is_success());

    // JWKS should now publish exactly one key.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let kids: Vec<&str> = json["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .filter_map(|k| k["kid"].as_str())
        .collect();
    assert_eq!(kids.len(), 1);
    assert!(!kids.contains(&original_id.as_str()), "retired+deleted key must be gone");
}

fn utf8_encode(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

#[tokio::test]
async fn backup_then_restore_preserves_users_and_clients() {
    use sui_id::backup;
    use sui_id::config::{LogConfig, ServerConfig, StorageConfig, TokensConfig};
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::Database;

    // Step 1: build a real on-disk database with users + a client.
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("source.sqlite");
    let key_path = tmp.path().join("source.key");

    let key = MasterKey::generate();
    let key_b64 = key.to_base64();
    std::fs::write(&key_path, &key_b64).expect("write key");

    let key2 = MasterKey::from_base64(&key_b64).expect("decode key");
    let db = Database::open(&db_path, key2).expect("open db");
    let cfg_src = sui_id::config::Config {
        server: ServerConfig {
            listen_addr: "127.0.0.1:0".into(),
            issuer: "https://idp.test".into(),
            cookie_secure: false,
            trusted_proxies: Vec::new(),
        },
        storage: StorageConfig {
            db_path: db_path.clone(),
            key_file: key_path.clone(),
        },
        tokens: TokensConfig::default(),
        log: LogConfig {
            format: "fmt".into(),
            filter: "off".into(),
        },
        security: sui_id::config::SecurityConfig::default(),
    };
    let mailer: std::sync::Arc<dyn sui_id_core::mail::MailSender> =
        std::sync::Arc::new(sui_id_core::mail::InMemoryMailSender::new());
    let hibp_client: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> =
        std::sync::Arc::new(sui_id_core::hibp::test_support::InMemoryHibpClient::new());
    let state = sui_id::AppState::new(db, cfg_src.clone(), SETUP_TOKEN.into(), mailer, hibp_client);
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    // Step 2: take a backup.
    let archive = tmp.path().join("backup.tar");
    backup::run_backup(&cfg_src, &archive, &backup::BackupOptions::default()).expect("backup");
    assert!(archive.exists());

    // Step 3: restore into a fresh location and re-open.
    let cfg_dst = sui_id::config::Config {
        storage: StorageConfig {
            db_path: tmp.path().join("restored.sqlite"),
            key_file: tmp.path().join("restored.key"),
        },
        ..cfg_src.clone()
    };
    backup::run_restore(
        &cfg_dst,
        &archive,
        &backup::RestoreOptions {
            force: false,
            passphrase: None,
        },
    )
    .expect("restore");

    // Step 4: open the restored DB with the restored key and verify the
    // user and client are still there.
    let restored_key_b64 = std::fs::read_to_string(&cfg_dst.storage.key_file).expect("read key");
    let restored_key = MasterKey::from_base64(restored_key_b64.trim()).expect("decode");
    let db2 = Database::open(&cfg_dst.storage.db_path, restored_key).expect("open restored");
    let users = sui_id_store::repos::users::list(&db2).expect("list users");
    assert_eq!(users.len(), 1, "the admin user should survive the round trip");
    let clients = sui_id_store::repos::clients::list(&db2).expect("list clients");
    assert_eq!(clients.len(), 1, "the client should survive the round trip");
    assert_eq!(clients[0].id.to_string(), client_id);
}

// ---------- CSRF tests ----------

#[tokio::test]
async fn admin_get_sets_csrf_cookie() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/users")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("admin GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf");
    assert!(csrf.is_some(), "admin GET must set sui_id_csrf");
    let value = csrf.unwrap();
    assert!(!value.is_empty());
    assert_eq!(value.len(), 43, "32 bytes b64url no-pad = 43 chars");
}

#[tokio::test]
async fn admin_post_without_csrf_cookie_is_forbidden() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // POST to clients/create with the session cookie but NO csrf cookie
    // and NO _csrf field. Should be 403.
    let router = build_router(state.clone());
    let body = "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_post_with_mismatched_csrf_is_forbidden() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Real csrf cookie, but the form's _csrf field is something else.
    let real = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let body = format!(
        "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true&_csrf=tampered-value-not-the-real-one"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={real}"),
        )
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_post_with_matching_csrf_succeeds() {
    // Sanity: this is what every other admin test relies on. If this
    // breaks, csrf has gone wrong systemically.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;
    assert!(!client_id.is_empty());
}

#[tokio::test]
async fn oidc_endpoints_are_not_subject_to_csrf() {
    // The /oauth2/* protocol surface must not require sui_id_csrf — it
    // is protocol traffic, not an admin form.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("").to_owned();
    let parsed = Url::parse(&location).expect("redirect");
    let code = parsed.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.into_owned()).expect("code");

    // /token exchange with NO csrf cookie or field — must succeed.
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK, "OIDC token endpoint must not require CSRF");
}

// ---------- scope policy and post_logout_redirect_uris (v0.6.0) ----------

#[tokio::test]
async fn authorize_rejects_scope_outside_client_policy() {
    use sui_id_core::admin::CreateClientSpec;
    use sui_id_store::repos::clients;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;

    // Create a client whose policy permits "openid" only.
    let created = sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        admin_id,
        CreateClientSpec {
            name: "scoped-rp",
            redirect_uris: &["https://rp.test/cb".into()],
            confidential: true,
            allowed_scopes: "openid",
            post_logout_redirect_uris: &[],
        },
    )
    .expect("create");
    let client_id = created.row.id.to_string();

    // A request that asks for "openid email" must be rejected because
    // "email" isn't in the policy.
    let (_v, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid+email&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    // sui-id should redirect with `error=invalid_scope` per RFC 6749 §4.1.2.1.
    if resp.status().is_redirection() {
        let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(
            loc.contains("error=invalid_scope"),
            "expected invalid_scope error redirect, got: {loc}"
        );
    } else {
        // Fallback: a non-redirect error is also acceptable as long as
        // the request didn't issue a code.
        assert!(resp.status().is_client_error());
    }

    // Sanity: with a permitted scope, the same flow succeeds.
    let _ = clients::get(&state.db, created.row.id).expect("still there");
    let auth_url_ok = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url_ok)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize ok");
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(
        loc.starts_with("https://rp.test/cb?code="),
        "expected success redirect with code, got: {loc}"
    );
}

#[tokio::test]
async fn authorize_with_empty_policy_permits_any_scope() {
    // Backwards-compatibility path: legacy clients (with allowed_scopes
    // = "") behave as before — any scope is accepted.
    use sui_id_core::admin::CreateClientSpec;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;
    let created = sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        admin_id,
        CreateClientSpec {
            name: "legacy-rp",
            redirect_uris: &["https://rp.test/cb".into()],
            confidential: true,
            allowed_scopes: "", // permit any
            post_logout_redirect_uris: &[],
        },
    )
    .expect("create");
    let client_id = created.row.id.to_string();
    let (_v, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid+email+anything_else\
         &code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(loc.starts_with("https://rp.test/cb?code="), "expected code, got: {loc}");
}

#[tokio::test]
async fn logout_uses_post_logout_redirect_uris_when_registered() {
    use sui_id_core::admin::CreateClientSpec;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;

    // Client with a logout URI that is NOT in redirect_uris. With the
    // new field present, sui-id should accept this for logout even
    // though the URI is not a valid redirect_uri for authorization.
    let created = sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        admin_id,
        CreateClientSpec {
            name: "logout-rp",
            redirect_uris: &["https://rp.test/cb".into()],
            confidential: true,
            allowed_scopes: "openid",
            post_logout_redirect_uris: &["https://rp.test/goodbye".into()],
        },
    )
    .expect("create");
    let client_id = created.row.id.to_string();

    // Logout with the dedicated post-logout URI: should redirect.
    let url = format!(
        "/oauth2/logout?client_id={client_id}&post_logout_redirect_uri=https%3A%2F%2Frp.test%2Fgoodbye"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    assert!(resp.status().is_redirection());
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(loc.starts_with("https://rp.test/goodbye"), "got: {loc}");

    // Conversely: a redirect_uri that is *not* in post_logout list
    // must NOT be accepted at logout when post_logout list is non-empty.
    let url2 = format!(
        "/oauth2/logout?client_id={client_id}&post_logout_redirect_uri=https%3A%2F%2Frp.test%2Fcb"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&url2)
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout 2");
    if resp.status().is_redirection() {
        let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(
            !loc.starts_with("https://rp.test/cb"),
            "redirect_uris must not leak into logout when post_logout list is set"
        );
    }
}

#[tokio::test]
async fn logout_falls_back_to_redirect_uris_when_post_logout_list_empty() {
    // Backwards compat: clients with no post_logout_redirect_uris still
    // get the v0.5.0 behaviour (logout matches redirect_uris).
    use sui_id_core::admin::CreateClientSpec;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;
    let created = sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        admin_id,
        CreateClientSpec {
            name: "legacy-rp",
            redirect_uris: &["https://rp.test/cb".into()],
            confidential: true,
            allowed_scopes: "openid",
            post_logout_redirect_uris: &[], // empty -> fallback
        },
    )
    .expect("create");
    let client_id = created.row.id.to_string();
    let url = format!(
        "/oauth2/logout?client_id={client_id}&post_logout_redirect_uri=https%3A%2F%2Frp.test%2Fcb"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("logout");
    assert!(resp.status().is_redirection());
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(loc.starts_with("https://rp.test/cb"));
}

// ---------- MFA / TOTP (v0.7.0) ----------

/// Helper: log in via password and follow the LoginOutcome::MfaRequired
/// branch by hand. Returns (pending_mfa cookie value, session cookie
/// once promoted via TOTP).
async fn enroll_mfa_for(state: &AppState, session: &str) -> (String, Vec<String>) {
    use sui_id_core::totp;

    // Start enrolment.
    let csrf = fetch_csrf(state, session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/profile/mfa/enroll/start")
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
    // Pull the Base32 secret out of the page. The label is in
    // Japanese ("秘密鍵:") and the surrounding span carries an
    // inline style after the v0.20.1 design refresh, so we anchor
    // on the label and skip past the next `<span class="code"` to
    // the `>` that closes the opening tag.
    let secret_b32 = {
        let label_at = html.find("秘密鍵:").expect("secret label rendered");
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
    let code = totp::code_for_step(&secret, step);

    // Confirm.
    let csrf = fetch_csrf(state, session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/profile/mfa/enroll/confirm")
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

fn decode_b32(s: &str) -> Vec<u8> {
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

#[tokio::test]
async fn mfa_enroll_then_login_with_totp_succeeds() {
    use sui_id_core::totp;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol.
    let (secret_b32, _codes) = enroll_mfa_for(&state, &session).await;
    let secret = decode_b32(&secret_b32);

    // Now: a fresh password login should *NOT* yield a session cookie
    // — instead it should set sui_id_pending_mfa and redirect.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "expected redirect to MFA");
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(loc.ends_with("/admin/login/mfa"), "got: {loc}");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa")
        .expect("pending_mfa cookie set");
    let session_cookie = extract_set_cookie(resp.headers(), "sui_id_session");
    assert!(session_cookie.is_none(), "session cookie must not be set before MFA");

    // Submit a fresh TOTP code.
    let now = chrono::Utc::now().timestamp();
    // Use step+1 to avoid the replay-defence cursor we set at enrolment time.
    let step = now / 30 + 1;
    let code = totp::code_for_step(&secret, step);

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "expected redirect to /admin");
    let session_cookie = extract_set_cookie(resp.headers(), "sui_id_session")
        .expect("session cookie issued after MFA success");
    assert!(!session_cookie.is_empty());
}

#[tokio::test]
async fn mfa_login_with_wrong_code_returns_401() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let _ = enroll_mfa_for(&state, &session).await;

    // Password login → pending.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending");

    // Wrong code.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code=000000&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_none());
}

#[tokio::test]
async fn mfa_login_with_recovery_code_succeeds_and_consumes_code() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (_secret_b32, codes) = enroll_mfa_for(&state, &session).await;
    let one_code = codes[0].clone();

    // Password login → pending.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending");

    // Submit recovery code.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!(
            "code={}&_csrf={csrf}",
            utf8_encode(&one_code)
        )))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "recovery code should accept");
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());

    // Reusing the same recovery code must fail. We need a new pending row.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login 2");
    let pending2 = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending2");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending2}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET 2");
    let csrf2 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf2");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending2}; sui_id_csrf={csrf2}"),
        )
        .body(Body::from(format!(
            "code={}&_csrf={csrf2}",
            utf8_encode(&one_code)
        )))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST replay");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "recovery code must be single-use"
    );
}

#[tokio::test]
async fn mfa_disable_lets_user_log_in_with_password_only() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let _ = enroll_mfa_for(&state, &session).await;

    // Disable MFA.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/profile/mfa/disable")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("disable");
    assert!(resp.status().is_redirection());

    // Password login should now go straight to a session.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login post-disable");
    assert!(resp.status().is_redirection());
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(!loc.ends_with("/admin/login/mfa"));
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());
}

// ---------- admin-initiated MFA reset (v0.10.0) ----------

#[tokio::test]
async fn admin_can_reset_users_mfa_factors() {
    use sui_id_core::admin::{admin_reset_mfa, CreateUserSpec};
    use sui_id_core::mfa;
    use sui_id_core::time::system_clock;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;
    let clock = system_clock();

    // Create a second user (the target) and enrol TOTP for them.
    sui_id_core::admin::create_user(
        &state.db,
        &state.clock,
        admin_id,
        CreateUserSpec {
            username: "bob",
            password: "bob-very-strong-password",
            display_name: None,
            email: None,
            is_admin: false,
        },
    )
    .expect("create");
    let bob = sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .expect("bob")
        .id;
    let ticket = mfa::start_enrollment(&state.db, "sui-id", bob, "bob").expect("start");
    let step = clock.now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&ticket.secret, step);
    let _ = mfa::confirm_enrollment(&state.db, &clock, bob, code).expect("confirm");
    assert!(mfa::is_mfa_enabled(&state.db, bob).unwrap());

    // Admin resets it.
    let report = admin_reset_mfa(&state.db, admin_id, bob).expect("reset");
    assert!(report.totp_removed);
    assert_eq!(report.passkeys_removed, 0);

    // MFA is now off for bob, and the audit log captured the reset.
    assert!(!mfa::is_mfa_enabled(&state.db, bob).unwrap());
    let audit = sui_id_store::repos::audit::recent(&state.db, 50).expect("audit");
    let reset_entries: Vec<_> = audit
        .iter()
        .filter(|e| e.action == "mfa.admin_reset")
        .collect();
    assert_eq!(reset_entries.len(), 1, "exactly one reset row expected");
    assert_eq!(reset_entries[0].actor, Some(admin_id));
    assert_eq!(reset_entries[0].target, Some(bob.to_string()));
    let note = reset_entries[0].note.as_deref().unwrap_or("");
    assert!(
        note.contains("totp=removed") && note.contains("passkeys=0"),
        "audit note should describe what was removed; got {note:?}"
    );
}

#[tokio::test]
async fn admin_mfa_reset_via_http_redirects_and_disables_mfa_requirement() {
    use sui_id_core::admin::CreateUserSpec;
    use sui_id_core::mfa;
    use sui_id_core::time::system_clock;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;
    let clock = system_clock();

    sui_id_core::admin::create_user(
        &state.db,
        &state.clock,
        admin_id,
        CreateUserSpec {
            username: "carol",
            password: "carol-very-strong-password",
            display_name: None,
            email: None,
            is_admin: false,
        },
    )
    .expect("create");
    let carol = sui_id_store::repos::users::find_by_username(&state.db, "carol")
        .expect("carol")
        .id;
    let ticket = mfa::start_enrollment(&state.db, "sui-id", carol, "carol").expect("start");
    let step = clock.now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&ticket.secret, step);
    let _ = mfa::confirm_enrollment(&state.db, &clock, carol, code).expect("confirm");

    // Sanity: a fresh password login for carol now goes to MFA challenge.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=carol&password=carol-very-strong-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.ends_with("/admin/login/mfa"), "got: {loc}");

    // Admin issues the reset via the HTTP endpoint.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/users/{carol}/mfa-reset"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("reset");
    assert!(resp.status().is_redirection(), "got {}", resp.status());
    assert_eq!(
        resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/admin/users")
    );

    // Now carol's password login goes straight to a session.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=carol&password=carol-very-strong-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login post-reset");
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(!loc.ends_with("/admin/login/mfa"), "should not require MFA; got: {loc}");
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());
}

// ---------- RFC 7662 introspection + RFC 7009 revocation (v0.11.0) ----------

/// Helper: full setup → authorize → token, returning
/// (client_id, client_secret, access_token, refresh_token).
async fn obtain_tokens(state: &AppState) -> (String, String, String, String) {
    let session = complete_setup_and_login(state).await;
    let (client_id, client_secret) = create_client(state, &session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=x&nonce=n&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&loc)
        .expect("url")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access = json["access_token"].as_str().unwrap().to_owned();
    let refresh = json["refresh_token"].as_str().unwrap().to_owned();
    (client_id, client_secret, access, refresh)
}

#[tokio::test]
async fn introspect_returns_active_for_valid_access_token() {
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;
    let body = format!(
        "token={access}&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    assert_eq!(v["active"].as_bool(), Some(true));
    assert_eq!(v["token_type"].as_str(), Some("Bearer"));
    assert_eq!(v["client_id"].as_str(), Some(client_id.as_str()));
    assert_eq!(v["username"].as_str(), Some("alice"));
    assert!(v["sub"].is_string());
    assert!(v["exp"].is_i64());
}

#[tokio::test]
async fn introspect_returns_active_for_valid_refresh_token() {
    let state = test_app();
    let (client_id, client_secret, _access, refresh) = obtain_tokens(&state).await;
    let body = format!(
        "token={refresh}&token_type_hint=refresh_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(true));
    assert_eq!(v["client_id"].as_str(), Some(client_id.as_str()));
}

#[tokio::test]
async fn introspect_returns_inactive_for_garbage_token() {
    let state = test_app();
    let (client_id, client_secret, _access, _refresh) = obtain_tokens(&state).await;
    let body = format!(
        "token=this.is.not.a.token&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(false));
    // RFC 7662 §2.2: when active=false, no other fields should be sent.
    assert!(v.get("scope").is_none(), "inactive must not leak scope");
    assert!(v.get("sub").is_none(), "inactive must not leak sub");
}

#[tokio::test]
async fn introspect_rejects_unauthenticated_request() {
    let state = test_app();
    let (_cid, _cs, access, _refresh) = obtain_tokens(&state).await;
    let body = format!("token={access}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_then_introspect_shows_inactive_for_access_token() {
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;

    // Revoke.
    let body = format!(
        "token={access}&token_type_hint=access_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke");
    assert_eq!(resp.status(), StatusCode::OK);

    // Now introspect should report inactive.
    let body = format!(
        "token={access}&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect2");
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(v["active"].as_bool(), Some(false));

    // userinfo with the same revoked token must now reject.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert!(
        resp.status().is_client_error(),
        "userinfo must reject revoked token; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_refresh_token_invalidates_subsequent_refresh_grant() {
    let state = test_app();
    let (client_id, client_secret, _access, refresh) = obtain_tokens(&state).await;

    // Revoke the refresh.
    let body = format!(
        "token={refresh}&token_type_hint=refresh_token\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke");
    assert_eq!(resp.status(), StatusCode::OK);

    // Subsequent refresh-grant attempt must fail.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh");
    assert!(
        resp.status().is_client_error(),
        "refresh of revoked token must fail; got {}",
        resp.status()
    );
}

#[tokio::test]
async fn revoke_is_idempotent() {
    // RFC 7009 §2.2: revoking an already-revoked or invalid token
    // must still return 200.
    let state = test_app();
    let (client_id, client_secret, access, _refresh) = obtain_tokens(&state).await;

    for _ in 0..3 {
        let body = format!(
            "token={access}&client_id={client_id}&client_secret={client_secret}"
        );
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/oauth2/revoke")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("req");
        let resp = router.oneshot(req).await.expect("revoke");
        assert_eq!(resp.status(), StatusCode::OK, "revoke must be idempotent");
    }

    // And a totally bogus token also returns 200.
    let body = format!(
        "token=garbage&client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("revoke garbage");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn introspect_other_clients_token_returns_inactive() {
    // Two clients exist; client B tries to introspect a token issued
    // to client A. RFC 7662 §2.2 requires inactive (no leakage).
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_a, secret_a) = create_client(&state, &session).await;
    let (client_b, secret_b) = create_client(&state, &session).await;

    // Get a token for client A.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_a}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap()
        .to_owned();
    let code = Url::parse(&loc)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .unwrap();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_a}&client_secret={secret_a}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    let access = v["access_token"].as_str().unwrap().to_owned();

    // Client B introspects A's token. Must come back inactive.
    let body = format!(
        "token={access}&client_id={client_b}&client_secret={secret_b}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(
        v["active"].as_bool(),
        Some(false),
        "client B must not see client A's token"
    );
}

#[tokio::test]
async fn discovery_advertises_introspect_and_revoke_endpoints() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert!(v["introspection_endpoint"].as_str().unwrap().ends_with("/oauth2/introspect"));
    assert!(v["revocation_endpoint"].as_str().unwrap().ends_with("/oauth2/revoke"));
    let methods = v["introspection_endpoint_auth_methods_supported"]
        .as_array()
        .unwrap();
    let methods_set: Vec<_> = methods.iter().filter_map(|x| x.as_str()).collect();
    assert!(methods_set.contains(&"client_secret_basic"));
    // Public clients (auth method "none") must NOT be listed.
    assert!(!methods_set.contains(&"none"));
}

// `http` is brought in transitively by axum; we only need its HeaderMap.
use axum::http;

// ---------- client edit (v0.8.0) ----------

#[tokio::test]
async fn client_edit_updates_name_and_scopes() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    // GET the edit page first to obtain a CSRF token.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/admin/clients/{client_id}/edit"))
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("edit GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    // The form should be pre-filled with the existing redirect_uri.
    assert!(
        html.contains("https://rp.test/cb"),
        "edit form should display the existing redirect URI"
    );

    // POST a change: rename, swap redirect_uri, tighten scopes,
    // register a dedicated logout URI.
    let body = format!(
        "name=renamed-rp&redirect_uris=https%3A%2F%2Frp.test%2Fnew-cb\
         &allowed_scopes=openid&post_logout_redirect_uris=https%3A%2F%2Frp.test%2Fbye\
         &_csrf={csrf}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/clients/{client_id}/edit"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("edit POST");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    assert_eq!(
        resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/admin/clients")
    );

    // Verify the row was actually updated.
    use sui_id_shared::ids::ClientId;
    let id = client_id.parse::<ClientId>().expect("parse");
    let row = sui_id_store::repos::clients::get(&state.db, id).expect("get");
    assert_eq!(row.name, "renamed-rp");
    assert_eq!(row.redirect_uris, vec!["https://rp.test/new-cb".to_string()]);
    assert_eq!(row.allowed_scopes, "openid");
    assert_eq!(
        row.post_logout_redirect_uris,
        vec!["https://rp.test/bye".to_string()]
    );
}

#[tokio::test]
async fn client_edit_then_authorize_uses_new_scope_policy() {
    // Tightening allowed_scopes via the edit page must immediately
    // affect /oauth2/authorize without a server restart.
    use sui_id_core::admin::CreateClientSpec;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .expect("admin")
        .id;
    let created = sui_id_core::admin::create_client(
        &state.db,
        &state.clock,
        admin_id,
        CreateClientSpec {
            name: "rp",
            redirect_uris: &["https://rp.test/cb".into()],
            confidential: true,
            allowed_scopes: "", // initially permissive
            post_logout_redirect_uris: &[],
        },
    )
    .expect("create");
    let client_id = created.row.id.to_string();

    // Initially: scope=email is accepted (empty policy means "any").
    let (_v, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid+email&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        loc.starts_with("https://rp.test/cb?code="),
        "initial open policy must accept any scope, got: {loc}"
    );

    // Tighten via the edit page.
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb\
         &allowed_scopes=openid&post_logout_redirect_uris=\
         &_csrf={csrf}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/clients/{client_id}/edit"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("edit POST");
    assert!(resp.status().is_redirection());

    // Now: scope=email must be rejected.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize 2");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if resp.status().is_redirection() {
        assert!(
            loc.contains("error=invalid_scope"),
            "tightened policy should produce invalid_scope, got: {loc}"
        );
    } else {
        assert!(resp.status().is_client_error());
    }
}

// ---------- request-id middleware (v0.12.0) ----------

#[tokio::test]
async fn response_carries_a_generated_x_request_id_when_caller_omits_one() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    // UUIDv4 is 36 characters with dashes.
    assert_eq!(id.len(), 36, "got: {id}");
    assert!(id.matches('-').count() == 4);
}

#[tokio::test]
async fn caller_supplied_x_request_id_is_echoed_back() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", "client-trace-abc123")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    assert_eq!(
        resp.headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok()),
        Some("client-trace-abc123")
    );
}

#[tokio::test]
async fn caller_supplied_x_request_id_thats_too_long_is_replaced() {
    let state = test_app();
    let router = build_router(state);
    // 100 bytes — well over our MAX_LEN of 64. Middleware should
    // discard and generate a UUID instead. (We use only safe chars
    // here to isolate the length check from the alphabet check;
    // some unsafe bytes are pre-rejected by the http crate before
    // they reach our code, which is the right kind of defence in
    // depth but not what this test is exercising.)
    let long_id = "a".repeat(100);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", long_id.clone())
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    assert_ne!(id, long_id, "long id should have been replaced");
    assert_eq!(id.len(), 36, "should be a UUID; got: {id}");
}

#[tokio::test]
async fn caller_supplied_x_request_id_with_unsafe_chars_is_replaced() {
    let state = test_app();
    let router = build_router(state);
    // Space is in the http-permitted range but in our reject set.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", "has spaces in it")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    assert!(!id.contains(' '));
    assert_eq!(id.len(), 36);
}


// ---------- acr / amr in ID tokens (v0.15.0) ----------

/// Decode the unverified payload of a JWT and return it as JSON.
/// Signature verification is exercised by the JWKS-driven tests; here
/// we only need to read claims back.
fn decode_jwt_payload(jwt: &str) -> serde_json::Value {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let segments: Vec<&str> = jwt.split('.').collect();
    assert_eq!(segments.len(), 3, "JWT must have header.payload.signature");
    let payload = Base64UrlUnpadded::decode_vec(segments[1]).expect("base64url payload");
    serde_json::from_slice(&payload).expect("payload JSON")
}

/// Drive the same authorize→token flow as full_flow_*, but stop at the
/// /token response and decode the ID token claims so individual tests
/// can assert on `acr` / `amr`.
async fn drive_to_id_token_claims(state: &AppState, session_cookie: &str) -> serde_json::Value {
    let (client_id, client_secret) = create_client(state, session_cookie).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    let parsed = Url::parse(&location).expect("absolute redirect");
    let code = parsed
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let id_token = json["id_token"].as_str().expect("id_token");
    decode_jwt_payload(id_token)
}

#[tokio::test]
async fn id_token_carries_acr_1_and_amr_pwd_for_password_only_login() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let claims = drive_to_id_token_claims(&state, &session).await;
    assert_eq!(
        claims["acr"].as_str(),
        Some("1"),
        "password-only login must produce acr=\"1\"; got: {claims}"
    );
    let amr: Vec<&str> = claims["amr"]
        .as_array()
        .expect("amr is an array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd"]);
}

/// Helper: enrol TOTP for the freshly-set-up admin and return the
/// shared secret bytes ready for `totp::code_for_step`.
async fn enroll_totp_for_test(
    state: &AppState,
    session_cookie: &str,
) -> Vec<u8> {
    let (secret_b32, _codes) = enroll_mfa_for(state, session_cookie).await;
    decode_b32(&secret_b32)
}

/// Helper: log in with password+TOTP and return the resulting session
/// cookie. Mirrors `mfa_enroll_then_login_with_totp_succeeds` but
/// extracted so MFA-related tests can re-use it.
async fn login_with_totp_for_test(state: &AppState, secret: &[u8]) -> String {
    use sui_id_core::totp;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa")
        .expect("pending_mfa cookie");

    let step = chrono::Utc::now().timestamp() / 30 + 1;
    let code = totp::code_for_step(secret, step);

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    extract_set_cookie(resp.headers(), "sui_id_session")
        .expect("session cookie after MFA success")
}

#[tokio::test]
async fn id_token_carries_acr_2_and_amr_with_mfa_after_totp_login() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let secret = enroll_totp_for_test(&state, &session).await;
    let new_session = login_with_totp_for_test(&state, &secret).await;
    let claims = drive_to_id_token_claims(&state, &new_session).await;

    assert_eq!(
        claims["acr"].as_str(),
        Some("2"),
        "MFA-with-TOTP must produce acr=\"2\"; got: {claims}"
    );
    let amr: Vec<&str> = claims["amr"]
        .as_array()
        .expect("amr is an array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd", "otp", "mfa"]);
}

#[tokio::test]
async fn refresh_grant_preserves_acr_and_amr_from_original_session() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let secret = enroll_totp_for_test(&state, &session).await;
    let new_session = login_with_totp_for_test(&state, &secret).await;

    let (client_id, client_secret) = create_client(&state, &new_session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={new_session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .expect("url")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refresh_token = json["refresh_token"].as_str().expect("rt").to_owned();
    let initial = decode_jwt_payload(json["id_token"].as_str().expect("idt"));
    assert_eq!(initial["acr"].as_str(), Some("2"));

    // Exchange refresh — new ID token must echo original acr/amr.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh_token}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("refresh token");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refreshed = decode_jwt_payload(json["id_token"].as_str().expect("idt"));
    assert_eq!(
        refreshed["acr"].as_str(),
        Some("2"),
        "refresh must preserve acr; got: {refreshed}"
    );
    let amr: Vec<&str> = refreshed["amr"]
        .as_array()
        .expect("amr array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(amr, vec!["pwd", "otp", "mfa"]);
}

// ---------- account lockout (v0.16.0) ----------

#[tokio::test]
async fn three_consecutive_wrong_passwords_lock_the_account() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // First two failures: counter bumps but no lock.
    for attempt in 0..2 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let resp = router.oneshot(req).await.expect("login");
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {attempt}: expected 401"
        );
    }

    // Third failure: still 401 — the lock is now stamped, but the
    // response shape is identical to the previous failures (timing
    // and status code both indistinguishable to a remote observer).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=wrong-password-here"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Fourth attempt with the *correct* password must still be
    // refused: the account is locked.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "locked account must refuse even the correct password"
    );

    // Confirm the audit log records `auth.login.locked`.
    let recent = sui_id_store::repos::audit::recent(&state.db, 50).expect("audit list");
    let count = recent
        .iter()
        .filter(|r| r.action == "auth.login.locked")
        .count();
    assert!(
        count >= 1,
        "expected at least one auth.login.locked audit row; got {count}"
    );
}

#[tokio::test]
async fn admin_unlock_clears_an_active_lock() {
    use sui_id_store::repos::users;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // Drive the account to a locked state.
    for _ in 0..3 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let _ = router.oneshot(req).await.expect("login");
    }
    let alice = users::find_by_username(&state.db, "alice").expect("alice");
    assert!(alice.locked_until.is_some(), "expected locked");
    assert!(alice.failed_login_count >= 3);

    // Direct admin_unlock — same call the CLI subcommand makes.
    users::admin_unlock(&state.db, alice.id).expect("unlock");

    let alice2 = users::find_by_username(&state.db, "alice").expect("alice");
    assert!(alice2.locked_until.is_none(), "lock should be cleared");
    assert_eq!(alice2.failed_login_count, 0);

    // After the unlock, the correct password must succeed.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "post-unlock login should succeed"
    );
}

#[tokio::test]
async fn successful_login_clears_partial_failure_count() {
    use sui_id_store::repos::users;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // Two failures (under the threshold; no lock yet).
    for _ in 0..2 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let _ = router.oneshot(req).await.expect("login");
    }
    let alice = users::find_by_username(&state.db, "alice").expect("alice");
    assert_eq!(alice.failed_login_count, 2);
    assert!(alice.locked_until.is_none());

    // Then a successful login. The counter must reset to 0.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let alice2 = users::find_by_username(&state.db, "alice").expect("alice");
    assert_eq!(
        alice2.failed_login_count, 0,
        "successful login must reset counter"
    );
}

// ---------- security headers + CORS (v0.17.0) ----------

#[tokio::test]
async fn admin_responses_carry_security_headers() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("admin login GET");
    let h = resp.headers();
    // CSP must be present and forbid framing.
    let csp = h
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(csp.contains("frame-ancestors 'none'"), "csp={csp}");
    assert!(csp.contains("default-src 'self'"), "csp={csp}");
    // X-Frame-Options DENY for older browsers.
    assert_eq!(
        h.get("x-frame-options").and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        h.get("x-content-type-options").and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert!(h
        .get("referrer-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .contains("strict-origin"));
    assert!(h.contains_key("permissions-policy"));
}

#[tokio::test]
async fn discovery_endpoint_allows_cross_origin_fetch() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/openid-configuration")
        .header(header::ORIGIN, "https://spa.example")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("discovery");
    assert_eq!(resp.status(), StatusCode::OK);
    let acao = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert_eq!(acao, Some("*"), "discovery must allow cross-origin");
}

#[tokio::test]
async fn jwks_endpoint_allows_cross_origin_fetch() {
    let state = test_app();
    // Admin must exist for active signing key to be present, but we
    // can hit the JWKS endpoint regardless — it just returns the
    // current keys (or an empty set on a totally fresh DB).
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/.well-known/jwks.json")
        .header(header::ORIGIN, "https://spa.example")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("jwks");
    let acao = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert_eq!(acao, Some("*"));
}

#[tokio::test]
async fn userinfo_response_carries_no_store_cache_control() {
    // Drive a token issuance so we have an access token to call
    // userinfo with. We piggyback the existing full-flow setup.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let access = json["access_token"].as_str().expect("access_token").to_owned();

    // Now hit userinfo and assert Cache-Control.
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK);
    let cc = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok());
    assert_eq!(cc, Some("no-store"), "userinfo must not be cacheable");
}

// ---------- refresh token theft detection (v0.17.0) ----------

#[tokio::test]
async fn replaying_a_rotated_refresh_token_revokes_the_whole_family() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Initial issuance via authorization-code grant.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let original_refresh = json["refresh_token"]
        .as_str()
        .expect("refresh_token")
        .to_owned();

    // First legitimate rotation.
    let body = format!(
        "grant_type=refresh_token&refresh_token={original_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("first refresh");
    assert_eq!(resp.status(), StatusCode::OK, "first refresh must succeed");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let new_refresh = json["refresh_token"].as_str().expect("rt").to_owned();

    // Now an attacker (who captured the original refresh token before
    // rotation) replays it. This must fail and revoke the entire family.
    let body = format!(
        "grant_type=refresh_token&refresh_token={original_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("replay");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "replay of rotated refresh token must be refused"
    );

    // The new (legitimately rotated) refresh token must also be
    // revoked as part of the family-wide revoke. The legitimate
    // client will discover this on its next refresh and re-auth.
    let body = format!(
        "grant_type=refresh_token&refresh_token={new_refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("post-revoke refresh");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "legitimate token from the same family must also be revoked"
    );
}

#[tokio::test]
async fn theft_detection_writes_audit_event() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    // Issue, rotate once, then replay the original.
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=s&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let code = Url::parse(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap(),
    )
    .unwrap()
    .query_pairs()
    .find(|(k, _)| k == "code")
    .map(|(_, v)| v.into_owned())
    .unwrap();

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let rt0 = json["refresh_token"].as_str().expect("rt").to_owned();

    // First rotation.
    let body = format!(
        "grant_type=refresh_token&refresh_token={rt0}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let _ = router.oneshot(req).await.expect("rotate");

    // Replay original.
    let body = format!(
        "grant_type=refresh_token&refresh_token={rt0}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let _ = router.oneshot(req).await.expect("replay");

    // Audit log should contain `auth.refresh.theft_detected`.
    let recent = sui_id_store::repos::audit::recent(&state.db, 50).expect("audit list");
    let count = recent
        .iter()
        .filter(|r| r.action == "auth.refresh.theft_detected")
        .count();
    assert!(
        count >= 1,
        "expected at least one auth.refresh.theft_detected audit row"
    );
}

// ---------- /me/security (v0.18.0) ----------

#[tokio::test]
async fn me_security_page_renders_for_authenticated_user() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("me/security GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The page must mention what we expect to be there: the
    // section headings, the username, and the "current session"
    // marker for the row that matches the cookie.
    assert!(body.contains("アカウントセキュリティ"), "missing heading");
    assert!(body.contains("サインイン中の場所"), "missing sessions section");
    assert!(body.contains("最近のアクティビティ"), "missing audit section");
    assert!(body.contains("current session"), "current session not marked");
}

#[tokio::test]
async fn me_security_redirects_when_not_signed_in() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("me/security GET");
    // The CurrentUser extractor maps a missing cookie to
    // Unauthenticated, which the HTML error path renders as a
    // login redirect or an HTML error — either way, *not* OK.
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "unauthenticated request must not see /me/security"
    );
}

#[tokio::test]
async fn me_security_revoke_one_signs_target_session_out() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);

    // Sanity: GET /me/security with s1 should list both rows and
    // have a Revoke button for s2.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={s1}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("page");
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("Revoke"));

    // Re-fetch the page to get a CSRF token (the cookie is set on
    // the response, so we extract it from there).
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf cookie");

    // Revoke s2.
    let body = format!("_csrf={csrf}");
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/me/security/sessions/{s2}/revoke"))
        .header(
            header::COOKIE,
            format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke");
    assert!(
        resp.status().is_redirection(),
        "expected redirect got {}",
        resp.status()
    );

    // s2 must no longer authenticate.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={s2}"))
        .body(Body::empty())
        .expect("req");
    let resp = build_router(state).oneshot(req).await.expect("post-revoke s2");
    assert_ne!(resp.status(), StatusCode::OK, "s2 must be dead now");
}

#[tokio::test]
async fn me_security_revoke_all_others_keeps_current_session() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    let s3 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);
    assert_ne!(s2, s3);

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf cookie");

    let body = format!("_csrf={csrf}&current_session={s1}");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/me/security/sessions/revoke-all-others")
        .header(
            header::COOKIE,
            format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke-all-others");
    assert!(resp.status().is_redirection());

    // s1 must still work; s2 and s3 must not.
    for (sid, alive) in [(&s1, true), (&s2, false), (&s3, false)] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/me/security")
                    .header(header::COOKIE, format!("sui_id_session={sid}"))
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("post-revoke probe");
        if alive {
            assert_eq!(resp.status(), StatusCode::OK, "s1 (current) must remain alive");
        } else {
            assert_ne!(resp.status(), StatusCode::OK, "{sid} should be revoked");
        }
    }
}

#[tokio::test]
async fn me_security_cannot_revoke_someone_elses_session() {
    let state = test_app();
    // First user (the bootstrap admin = "alice").
    let s_admin = complete_setup_and_login(&state).await;

    // Create a second user "bob" via the admin UI.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/users")
                .header(header::COOKIE, format!("sui_id_session={s_admin}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("users page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");
    let body = format!(
        "_csrf={csrf}&username=bob&display_name=Bob&password=bob-the-tester-password"
    );
    let _ = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/users")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s_admin}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("create bob");
    let s_bob = login_again_for_admin(&state, "bob", "bob-the-tester-password").await;

    // The admin tries to revoke bob's session through /me/security.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s_admin}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf2 = extract_csrf_cookie(resp.headers()).expect("csrf");
    let body = format!("_csrf={csrf2}");
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/me/security/sessions/{s_bob}/revoke"))
        .header(
            header::COOKIE,
            format!("sui_id_session={s_admin}; sui_id_csrf={csrf2}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke attempt");
    // The handler redirects regardless (no leak), but bob's
    // session must still be good.
    assert!(resp.status().is_redirection());

    // Probe bob's session — must still be alive.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s_bob}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("bob probe");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "bob's session must NOT have been revoked by the admin's attempt"
    );
}

// ---------- helpers for /me/security tests ----------

fn extract_csrf_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    for v in headers.get_all(header::SET_COOKIE) {
        let s = v.to_str().ok()?;
        if let Some(rest) = s.strip_prefix("sui_id_csrf=") {
            let value = rest.split(';').next()?;
            return Some(value.to_owned());
        }
    }
    None
}

/// Issue a fresh session for the given username/password by hitting
/// `/admin/login` directly. The bootstrap helper already does the
/// setup wizard; we just want to add a parallel session.
async fn login_again_for_admin(state: &AppState, username: &str, password: &str) -> String {
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

fn urlencode(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

// ---------- self-service password change (v0.19.0) ----------

#[tokio::test]
async fn me_password_change_form_renders() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("password GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("パスワードを変更"));
    assert!(s.contains(r#"name="current_password""#));
    assert!(s.contains(r#"name="new_password""#));
    assert!(s.contains(r#"name="confirm_password""#));
    assert!(s.contains(r#"name="revoke_others""#));
}

#[tokio::test]
async fn me_password_change_happy_path_replaces_password() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
        // revoke_others omitted = unchecked for this test, so we can
        // inspect "old session still alive" cleanly afterwards
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(
        resp.status().is_redirection(),
        "expected redirect; got {}",
        resp.status()
    );

    // Old password no longer logs in; new one does.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}",
                    urlencode(USERNAME),
                    urlencode(PASSWORD)
                )))
                .expect("req"),
        )
        .await
        .expect("login old");
    let old_cookie = extract_set_cookie(resp.headers(), "sui_id_session");
    assert!(
        old_cookie.is_none(),
        "old password must no longer authenticate"
    );

    let new_cookie = login_again_for_admin(&state, USERNAME, new_pw).await;
    assert!(!new_cookie.is_empty());
}

#[tokio::test]
async fn me_password_change_wrong_current_is_refused() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode("wrong-current-tester-password"),
        urlencode("the-new-tester-password"),
        urlencode("the-new-tester-password")
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    // Refused — not redirected to /me/security?msg=password_changed.
    assert_ne!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "wrong current password must NOT yield a success redirect"
    );

    // Original password must still work.
    let cookie = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert!(!cookie.is_empty(), "original password must still authenticate");
}

#[tokio::test]
async fn me_password_change_mismatched_confirm_is_refused() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode("the-new-tester-password"),
        urlencode("typed-it-differently-the-second-time")
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert_ne!(resp.status(), StatusCode::SEE_OTHER);

    let cookie = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert!(!cookie.is_empty());
}

#[tokio::test]
async fn me_password_change_with_revoke_others_sweeps_other_sessions_and_refresh_tokens() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);

    // Mint a refresh token through the OAuth flow, so we have
    // something concrete to verify gets revoked.
    let (client_id, client_secret) = create_client(&state, &s1).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=z&code_challenge={challenge}&code_challenge_method=S256"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refresh = json["refresh_token"].as_str().expect("rt").to_owned();

    // Now do the password change with revoke_others=1.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-rotated-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}&revoke_others=1",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    // s1 (current session) is still alive.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("s1 probe");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "current session must remain alive across password change"
    );

    // s2 must be dead.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s2}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("s2 probe");
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "non-current session must be revoked"
    );

    // Refresh token must be dead.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rt redeem");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "refresh token must be revoked across password change"
    );
}

// ---------- dashboard sparkline (v0.20.2) ----------

#[tokio::test]
async fn dashboard_sparkline_renders_with_default_range() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("dashboard");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Sparkline-related markers we expect on the page.
    assert!(body.contains("サインイン活動"), "missing sparkline section");
    assert!(body.contains("過去 7 日間"), "missing default range tab label");
    // The SVG element with our aria-label must be there.
    assert!(
        body.contains(r#"aria-label="サインイン活動のスパークライン""#),
        "missing sparkline svg"
    );
    // The three range tabs must be present as anchors.
    assert!(body.contains("range=24h"));
    assert!(body.contains("range=7d"));
    assert!(body.contains("range=30d"));
    // 7d range = 7 buckets of audio-grid `<title>`. We can at
    // least verify the tooltip format made it into the HTML for
    // *some* bucket.
    assert!(
        body.contains("成功 0 / 失敗 0") || body.contains("成功 1 / 失敗 0"),
        "no bucket tooltip rendered"
    );
}

#[tokio::test]
async fn dashboard_sparkline_honours_explicit_range_query() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    for range in ["24h", "7d", "30d"] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/admin?range={range}"))
                    .header(header::COOKIE, format!("sui_id_session={session}"))
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("dashboard");
        assert_eq!(resp.status(), StatusCode::OK, "range={range}");
        let bytes = read_body(resp.into_body()).await;
        let body = String::from_utf8_lossy(&bytes);
        // The active range tab gets `aria-current="page"` on its
        // anchor. Detect that by string-search around the matching
        // href value.
        let needle = format!(r#"href="/admin?range={range}""#);
        assert!(
            body.contains(&needle),
            "expected anchor for range={range}"
        );
    }
}

#[tokio::test]
async fn dashboard_sparkline_falls_back_to_default_on_garbage_range() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin?range=banana")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("dashboard");
    // Should render normally — not 400 — and pick the default
    // (which is currently 7 days).
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("サインイン活動"));
}

// ---------- /admin/settings/* (v0.20.3) ----------

#[tokio::test]
async fn settings_index_redirects_to_basic() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/settings/basic");
}

#[tokio::test]
async fn settings_basic_renders_for_admin() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/basic")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("basic");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Tab strip is rendered with all five tabs.
    assert!(body.contains("/admin/settings/basic"));
    assert!(body.contains("/admin/settings/security"));
    assert!(body.contains("/admin/settings/authentication"));
    assert!(body.contains("/admin/settings/logs"));
    assert!(body.contains("/admin/settings/other"));
    // Active tab marker.
    assert!(
        body.contains(r#"href="/admin/settings/basic" aria-current="page""#),
        "basic tab should be aria-current"
    );
    // Body content.
    assert!(body.contains("Issuer"));
    assert!(body.contains("Listen address"));
    assert!(body.contains("Discovery"));
    assert!(body.contains("JWKS"));
}

#[tokio::test]
async fn settings_security_renders_lockout_and_headers() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("security");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("最大ロックアウト時間"));
    assert!(body.contains("HSTS"));
    assert!(body.contains("Content-Security-Policy"));
    assert!(body.contains("X-Frame-Options"));
    assert!(body.contains("CORS"));
}

#[tokio::test]
async fn settings_authentication_renders_lifetimes() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/authentication")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authentication");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("PKCE"));
    assert!(body.contains("Argon2id"));
    assert!(body.contains("Access token"));
    assert!(body.contains("Refresh"));
}

#[tokio::test]
async fn settings_logs_renders_with_24h_counts_and_chain_status() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/logs")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("logs");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("auth.login.success"));
    assert!(body.contains("auth.login.failure"));
    assert!(body.contains("auth.password.changed_self"));
    // Chain check report. With a fresh test_app there should be at
    // least one row (the setup), so chain status is "正常".
    assert!(body.contains("ハッシュチェーン"));
    assert!(body.contains("/admin/audit"));
}

#[tokio::test]
async fn settings_other_renders_versions_and_paths() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/other")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("other");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("sui-id バージョン"));
    assert!(body.contains("対応スキーマバージョン"));
    assert!(body.contains("DB ファイル"));
    assert!(body.contains("マスターキーファイル"));
    assert!(body.contains("/admin/users"));
    assert!(body.contains("/admin/clients"));
}

#[tokio::test]
async fn settings_pages_require_admin() {
    let state = test_app();
    let router = build_router(state);
    for path in [
        "/admin/settings/basic",
        "/admin/settings/security",
        "/admin/settings/authentication",
        "/admin/settings/logs",
        "/admin/settings/other",
    ] {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("settings");
        // Anonymous request must NOT see settings.
        assert_ne!(resp.status(), StatusCode::OK, "{path} leaked to anon");
    }
}

// ---------- v0.20.4: setup wizard 3-step ----------

#[tokio::test]
async fn setup_welcome_renders_when_uninitialized() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("sui-id へようこそ"));
    assert!(body.contains(r#"href="/setup/admin""#));
    // No form on the welcome page.
    assert!(!body.contains(r#"action="/setup/admin""#));
    // Step indicator shows step 1 active.
    assert!(body.contains("ようこそ"));
    assert!(body.contains("管理者作成"));
    assert!(body.contains("完了"));
}

#[tokio::test]
async fn setup_welcome_redirects_when_initialized() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("welcome after init");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login");
}

#[tokio::test]
async fn setup_admin_form_renders_with_email_and_confirm() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/admin")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"action="/setup/admin""#));
    assert!(body.contains(r#"name="setup_token""#));
    assert!(body.contains(r#"name="username""#));
    assert!(body.contains(r#"name="email""#));
    assert!(body.contains(r#"name="display_name""#));
    assert!(body.contains(r#"name="password""#));
    assert!(body.contains(r#"name="confirm_password""#));
}

#[tokio::test]
async fn setup_admin_post_creates_admin_with_email_and_redirects_to_done() {
    let state = test_app();
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &display_name=Alice\
         &email=alice%40example.test\
         &password={PASSWORD}\
         &confirm_password={PASSWORD}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("admin POST");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/setup/done");
    // Session cookie was set so the "次へ" button on /setup/done
    // lands on /admin already authenticated.
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());

    // The email was persisted on the user row.
    let row = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("user exists");
    assert_eq!(row.email.as_deref(), Some("alice@example.test"));
}

#[tokio::test]
async fn setup_admin_post_rejects_mismatched_confirm() {
    let state = test_app();
    let body = format!(
        "setup_token={SETUP_TOKEN}\
         &username={USERNAME}\
         &email=\
         &password={PASSWORD}\
         &confirm_password=different-password-here"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("admin POST");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("一致しません"));
    // No user was created.
    let result = sui_id_store::repos::users::find_by_username(&state.db, USERNAME);
    assert!(matches!(result, Err(sui_id_store::StoreError::NotFound)));
}

#[tokio::test]
async fn setup_done_renders_after_initialization() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/done")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("done");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("セットアップ完了"));
    assert!(body.contains(r#"href="/admin""#));
}

#[tokio::test]
async fn setup_done_says_not_yet_when_uninitialized() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup/done")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("done before init");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("完了していません"));
    assert!(body.contains(r#"href="/setup""#));
}

#[tokio::test]
async fn admin_users_create_form_accepts_email() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Get the users page to obtain a CSRF token.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("users GET");
    let csrf =
        extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie set on users GET");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body);
    // The create form must render an email field.
    assert!(html.contains(r#"name="email""#));

    // Submit the form with an email.
    let new_user_pw = "new-user-password-12345";
    let form = format!(
        "username=bob\
         &display_name=Bob\
         &email=bob%40example.test\
         &password={new_user_pw}\
         &_csrf={csrf}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/users")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(form))
                .expect("req"),
        )
        .await
        .expect("users POST");
    assert!(resp.status().is_redirection() || resp.status() == StatusCode::OK);

    // Verify the user has the email persisted.
    let row = sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .expect("bob exists");
    assert_eq!(row.email.as_deref(), Some("bob@example.test"));
}

// ---------- v0.21.0: step-up auth ----------

#[tokio::test]
async fn step_up_get_renders_form_with_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("再認証"));
    assert!(body.contains(r#"name="code""#));
    assert!(body.contains(r#"name="return_to""#));
    // The supplied return_to round-trips into the form.
    assert!(body.contains(r#"value="/admin/users""#));
}

#[tokio::test]
async fn step_up_get_sanitises_offsite_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=https://attacker.example/")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Off-site return_to gets collapsed to the safe default. The
    // attacker URL must NOT appear as a form value.
    assert!(!body.contains("attacker.example"));
    assert!(body.contains(r#"value="/me/security""#));
}

#[tokio::test]
async fn step_up_get_sanitises_protocol_relative_return_to() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=//attacker.example/path")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains("attacker"));
    assert!(body.contains(r#"value="/me/security""#));
}

#[tokio::test]
async fn step_up_post_with_correct_totp_marks_session_fresh_and_redirects() {
    use chrono::Utc;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol TOTP with a known secret directly via the store layer
    // so we can compute the expected code locally without going
    // through the enrolment flow (which would itself need the
    // running router).
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice exists");
    let secret = b"step-up-test-secret\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .expect("confirm totp");

    // Acquire CSRF cookie via the GET.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let now = Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(secret, step);

    let body = format!(
        "_csrf={csrf}\
         &code={code}\
         &return_to=/admin/users"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("step-up POST");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/users");

    // And the session row's last_step_up_at is now set.
    let session_id = sui_id_shared::ids::SessionId::from_uuid(
        session.parse::<uuid::Uuid>().expect("session id is uuid"),
    );
    let row =
        sui_id_store::repos::sessions::get(&state.db, session_id).expect("session row");
    assert!(
        row.last_step_up_at.is_some(),
        "session must be marked fresh after a successful step-up"
    );
}

#[tokio::test]
async fn step_up_post_with_wrong_code_returns_400_with_flash() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol TOTP so the verify path actually runs.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice exists");
    let secret = b"step-up-bad-code-secret\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .expect("upsert pending");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .expect("confirm");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!(
        "_csrf={csrf}\
         &code=000000\
         &return_to=/me/security"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("コードが正しくありません"));
}

#[tokio::test]
async fn step_up_redirects_when_mfa_admin_lacks_fresh() {
    // Verify the gate fires from a sensitive admin handler when
    // the admin has MFA enrolled but no recent step-up.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol MFA on the admin so the gate has something to gate
    // against.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice exists");
    let secret = b"gate-test-secret\x00\x00\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .expect("pending");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .expect("confirm");

    // The session predates the MFA enrolment, so its
    // last_step_up_at is still None and the gate should fire.

    // Get a CSRF cookie before posting to the gated endpoint.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/signing-keys")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("signing-keys GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/signing-keys/rotate")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rotate POST");
    // Step-up gate must redirect to the challenge page.
    assert!(
        resp.status().is_redirection(),
        "expected step-up redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location.starts_with("/me/security/step-up?"),
        "expected step-up URL, got {location}"
    );
    // The original target is preserved in `return_to`.
    assert!(
        location.contains("admin%2Fsigning-keys") || location.contains("/admin/signing-keys"),
        "return_to should preserve the original path, got {location}"
    );
}

#[tokio::test]
async fn admin_with_no_mfa_passes_step_up_gate_transparently() {
    // The default test admin has no MFA. The gate's
    // policy_for_session returns Allow for no-MFA users, so an
    // admin who never enrolled MFA can still rotate keys without
    // a re-prompt. This test pins that behaviour so future
    // refactors don't accidentally make MFA mandatory for admin
    // actions.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/signing-keys")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/signing-keys/rotate")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rotate POST");
    // Should redirect to /admin/signing-keys (success), not to
    // /me/security/step-up (gate firing).
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/signing-keys");
}

// ---------- v0.21.1: WebAuthn step-up ----------

#[tokio::test]
async fn step_up_form_shows_passkey_section_for_users_with_passkey() {
    use chrono::Utc;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice");

    // Insert a fake passkey row directly. The contents need not be
    // a real webauthn-rs Passkey for the GET form to render — only
    // the existence of *any* row matters for `has_credentials`.
    let cred_row = sui_id_store::models::UserWebauthnCredentialRow {
        id: sui_id_shared::ids::WebauthnCredentialId::new(),
        user_id: user.id,
        credential_id: vec![1, 2, 3, 4],
        passkey_enc: vec![], // create() seals our plaintext, this is overwritten
        nickname: "Test Key".into(),
        created_at: Utc::now(),
        last_used_at: None,
    };
    sui_id_store::repos::user_webauthn_credentials::create(&state.db, &cred_row, b"{}")
        .expect("create passkey");

    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=/admin/users")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"id="step-up-passkey-form""#), "passkey form should render");
    assert!(body.contains("/me/security/step-up/webauthn/start"));
    assert!(body.contains("/static/step-up-webauthn.js"));
}

#[tokio::test]
async fn step_up_form_omits_passkey_section_for_users_without_passkey() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains(r#"id="step-up-passkey-form""#));
    assert!(!body.contains("/me/security/step-up/webauthn"));
}

#[tokio::test]
async fn step_up_webauthn_start_requires_csrf() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // No CSRF cookie set: posting to /webauthn/start must fail.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::from("_csrf=&return_to=/me/security"))
                .expect("req"),
        )
        .await
        .expect("start POST");
    assert!(
        resp.status().is_client_error(),
        "missing CSRF must be rejected, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn step_up_webauthn_finish_without_pending_cookie_fails() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Get a CSRF cookie via the GET form so the finish call gets
    // past the CSRF guard and hits the pending-cookie check.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    // No pending-id cookie: finish must fail.
    let body = format!(
        "_csrf={csrf}\
         &credential={{}}\
         &return_to=/me/security"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/finish")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("finish POST");
    assert!(
        resp.status().is_client_error(),
        "missing pending cookie must reject, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn step_up_webauthn_start_for_user_without_passkey_returns_bad_request() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&return_to=/me/security");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up/webauthn/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("start POST");
    // The user has no passkey, so start_webauthn returns BadRequest.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------- v0.22.0: email features ----------

/// Insert a minimal SMTP configuration directly into the database
/// so the forgot-password endpoints stop returning 404 in tests.
/// We don't actually try to talk to a real SMTP relay — the
/// `InMemoryMailSender` injected via `test_app_with_mailer`
/// captures all sends.
fn enable_smtp_in_db(state: &AppState) {
    use chrono::Utc;
    use sui_id_store::models::{SmtpConfigRow, SmtpTlsMode};
    let now = Utc::now();
    let row = SmtpConfigRow {
        enabled: true,
        host: "smtp.test.invalid".into(),
        port: 587,
        tls_mode: SmtpTlsMode::StartTls,
        username: Some("test".into()),
        password_enc: None,
        from_address: "noreply@test.invalid".into(),
        from_name: Some("sui-id Test".into()),
        base_url: "https://idp.test.invalid".into(),
        created_at: now,
        updated_at: now,
    };
    sui_id_store::repos::smtp_config::upsert(&state.db, &row)
        .expect("upsert smtp_config");
}

#[tokio::test]
async fn forgot_password_get_404_when_smtp_disabled() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("forgot GET");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn forgot_password_get_renders_form_when_smtp_enabled() {
    let state = test_app();
    enable_smtp_in_db(&state);
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("forgot GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains(r#"action="/forgot-password""#));
    assert!(body.contains(r#"name="email""#));
}

#[tokio::test]
async fn forgot_password_post_neutral_response_for_unknown_email() {
    let (state, mailer) = test_app_with_mailer();
    enable_smtp_in_db(&state);

    // Get a CSRF cookie via the GET first.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&email=ghost%40nowhere.invalid");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::OK);

    // No mail was sent — the email did not match a user.
    assert_eq!(mailer.count().await, 0);
}

#[tokio::test]
async fn forgot_password_post_sends_mail_for_known_email() {
    let (state, mailer) = test_app_with_mailer();
    let _ = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state);

    // The default test admin doesn't have an email set; assign one
    // directly so the forgot-password lookup matches.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice");
    let mut updated = user.clone();
    updated.email = Some("alice@test.invalid".into());
    updated.updated_at = chrono::Utc::now();
    // No bulk update helper; round-trip a delete/create pair would
    // complicate things, so we use a raw SQL UPDATE via the DB
    // handle.
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE users SET email = ?1 WHERE id = ?2",
                rusqlite::params![updated.email, user.id.to_string()],
            )?;
            Ok(())
        })
        .expect("set email");

    // CSRF cookie via GET.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    let body = format!("_csrf={csrf}&email=alice%40test.invalid");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST");
    assert_eq!(resp.status(), StatusCode::OK);

    // One mail captured. Subject and body shape pinned so future
    // reword changes are intentional.
    assert_eq!(mailer.count().await, 1);
    let last = mailer.last().await.expect("at least one mail");
    assert_eq!(last.to, "alice@test.invalid");
    assert!(last.subject.contains("パスワードのリセット"));
    assert!(last.text_body.contains("/reset-password?token="));
    assert!(last.html_body.is_some());
}

#[tokio::test]
async fn reset_password_get_invalid_for_unknown_token() {
    let state = test_app();
    enable_smtp_in_db(&state);
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/reset-password?token=this-does-not-exist")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The "this link is invalid or expired" page renders, not the
    // new-password form.
    assert!(body.contains("無効"));
    assert!(!body.contains(r#"name="password""#));
}

#[tokio::test]
async fn reset_password_full_flow_changes_password_and_sends_notification() {
    let (state, mailer) = test_app_with_mailer();
    let _ = complete_setup_and_login(&state).await;
    enable_smtp_in_db(&state);

    // Set the admin's email so they can reset.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("alice");
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE users SET email = ?1 WHERE id = ?2",
                rusqlite::params!["alice@test.invalid", user.id.to_string()],
            )?;
            Ok(())
        })
        .expect("set email");

    // 1) POST /forgot-password to mint a token + capture mail
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let body = format!("_csrf={csrf}&email=alice%40test.invalid");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST forgot");
    assert_eq!(resp.status(), StatusCode::OK);

    // Extract the token from the captured mail.
    let mail = mailer.last().await.expect("reset mail");
    let prefix = "/reset-password?token=";
    let start = mail
        .text_body
        .find(prefix)
        .expect("link in mail")
        + prefix.len();
    let end = mail.text_body[start..]
        .find(|c: char| c == '\n' || c.is_whitespace())
        .map(|i| start + i)
        .unwrap_or(mail.text_body.len());
    let token = mail.text_body[start..end].to_owned();
    assert!(!token.is_empty());

    // 2) GET /reset-password?token=... renders the form
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/reset-password?token={token}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET reset");
    assert_eq!(resp.status(), StatusCode::OK);
    let csrf2 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf2");
    let body_bytes = read_body(resp.into_body()).await;
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains(r#"name="password""#));

    // 3) POST /reset-password with new password
    let new_pw = "brand-new-secure-pw-12345";
    let body = format!(
        "_csrf={csrf2}&token={token}&password={new_pw}&confirm_password={new_pw}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/reset-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf2}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST reset");
    assert!(resp.status().is_redirection(), "expected redirect, got {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(location.starts_with("/admin/login"));

    // 4) The captured mailer now has 2 mails: the reset link + a
    //    post-reset password-changed notification.
    assert_eq!(mailer.count().await, 2);
    let drained = mailer.drain().await;
    assert!(drained
        .iter()
        .any(|m| m.subject.contains("パスワードが変更されました")));

    // 5) Replay of the same token returns 400 + invalid page.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/reset-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET2");
    let csrf3 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf3");
    let body = format!(
        "_csrf={csrf3}&token={token}&password=different-second-password-99&confirm_password=different-second-password-99"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/reset-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf3}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST replay");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn settings_email_get_renders_for_admin() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/email")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("メール"));
    assert!(body.contains(r#"name="host""#));
    assert!(body.contains(r#"name="port""#));
    assert!(body.contains(r#"name="from_address""#));
}

#[tokio::test]
async fn settings_email_get_requires_admin() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/email")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings GET");
    // Anonymous request must not see the page.
    assert_ne!(resp.status(), StatusCode::OK);
}

// ---------- v0.22.0: password-change notification mail ----------

/// Helper: set the admin's email column directly in the DB. Setup
/// runs through `/setup/admin` which doesn't accept an email
/// post-setup; we bypass that by writing the column ourselves.
async fn set_admin_email(state: &AppState, email: &str) {
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .expect("user");
    state.db.with_conn(|conn| {
        conn.execute(
            "UPDATE users SET email = ?1 WHERE id = ?2",
            rusqlite::params![email, user.id.to_string()],
        )
        .expect("update");
        Ok(())
    })
    .expect("set email");
}

#[tokio::test]
async fn password_change_sends_notification_mail_when_email_is_set() {
    let (state, mailer) = test_app_with_mailer();
    let s1 = complete_setup_and_login(&state).await;
    set_admin_email(&state, "alice@example.test").await;

    // GET to obtain CSRF cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    // Drain captured emails. We expect exactly one — the
    // "your password has changed" notification.
    let sent = mailer.drain().await;
    assert_eq!(
        sent.len(),
        1,
        "expected one notification email, got {}",
        sent.len()
    );
    let mail = &sent[0];
    assert_eq!(mail.to, "alice@example.test");
    assert!(
        mail.subject.contains("パスワード"),
        "subject should mention password: {}",
        mail.subject
    );
    assert!(
        mail.text_body.contains("変更"),
        "body should mention 'changed': {}",
        mail.text_body
    );
}

#[tokio::test]
async fn password_change_sends_no_mail_when_email_is_unset() {
    let (state, mailer) = test_app_with_mailer();
    let s1 = complete_setup_and_login(&state).await;
    // Deliberately do NOT set the admin's email.

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    let sent = mailer.drain().await;
    assert!(
        sent.is_empty(),
        "expected no notification mail (no email on user), got {}",
        sent.len()
    );
}

// ---------- v0.23.0: Multilingual support ----------

/// `<html lang>` reflects whichever locale the resolution chain
/// picks. With no user, no cookie, no Accept-Language, the
/// migration's seeded default ('ja') wins.
#[tokio::test]
async fn login_page_html_lang_defaults_to_ja() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"ja\">"),
        "expected lang=ja, body starts: {}",
        &body[..body.len().min(200)]
    );
}

/// Accept-Language header pushes `<html lang>` to en.
#[tokio::test]
async fn login_page_html_lang_follows_accept_language() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"en\">"),
        "expected lang=en, head starts: {}",
        &body[..body.len().min(200)]
    );
}

/// `sui_id_lang` cookie overrides Accept-Language.
#[tokio::test]
async fn lang_cookie_overrides_accept_language() {
    let state = test_app();
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .header(header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .header(header::COOKIE, "sui_id_lang=ja")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("<html lang=\"ja\">"),
        "cookie should override Accept-Language; body: {}",
        &body[..body.len().min(200)]
    );
}

/// Posting to /admin/profile/lang persists the value, sets the
/// cookie, and is reflected on the next render.
#[tokio::test]
async fn profile_lang_post_persists_and_sets_cookie() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // GET /admin/profile to obtain a CSRF token.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile GET");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    // POST /admin/profile/lang with `lang=en`.
    let body = format!("_csrf={csrf}&lang=en");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_redirection(),
        "expected redirect, got {}",
        resp.status()
    );
    // Set-Cookie sui_id_lang=en should appear.
    let set_cookies: Vec<_> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        set_cookies.iter().any(|c| c.starts_with("sui_id_lang=en")),
        "expected sui_id_lang=en cookie; saw: {:?}",
        set_cookies
    );

    // DB has been updated.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    assert_eq!(user.preferred_lang.as_deref(), Some("en"));
}

/// Setting `lang=` (empty) clears the preference and the cookie.
#[tokio::test]
async fn profile_lang_clear_resets_to_browser_default() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Pre-set to "en" first.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    sui_id_store::repos::users::set_preferred_lang(
        &state.db,
        user.id,
        Some("en"),
        chrono::Utc::now(),
    )
    .expect("preset");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("profile GET");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&lang=");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    // Cookie cleared (Max-Age=0).
    let set_cookies: Vec<_> = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert!(
        set_cookies
            .iter()
            .any(|c| c.starts_with("sui_id_lang=") && c.contains("Max-Age=0")),
        "expected sui_id_lang to be cleared, saw: {:?}",
        set_cookies
    );

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    assert_eq!(user.preferred_lang, None);
}

/// Unknown language tag is rejected with 400.
#[tokio::test]
async fn profile_lang_post_rejects_unknown_tag() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/profile")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&lang=xyz");
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/profile/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

/// Admin can change the server default language at
/// /admin/settings/basic/lang.
#[tokio::test]
async fn admin_settings_basic_default_lang_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/basic")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings basic");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&default_lang=en");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/basic/lang")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db).expect("settings");
    assert_eq!(row.default_lang, "en");
}

/// Forgot-password mail is sent in the recipient's preferred
/// locale.
#[tokio::test]
async fn forgot_password_email_in_user_preferred_locale() {
    let (state, mailer) = test_app_with_mailer();
    complete_setup_and_login(&state).await;

    // Configure SMTP and set the user's email + preferred lang to en.
    enable_smtp_with_inmemory_mailer(&state).await;
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE users SET email = ?1, preferred_lang = ?2 WHERE id = ?3",
                rusqlite::params!["alice@example.test", "en", user.id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .expect("set fields");

    // GET /forgot-password to obtain a CSRF cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&email=alice%40example.test");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    // Forgot-password is user-enumeration neutral so it always
    // returns 200; we don't assert on the redirect/body shape.
    let _ = resp.status();

    let sent = mailer.drain().await;
    assert_eq!(
        sent.len(),
        1,
        "expected one email, got {}",
        sent.len()
    );
    let mail = &sent[0];
    // English subject contains the English string from STRINGS_EN.
    assert!(
        mail.subject.starts_with("Reset your password"),
        "expected English subject, got: {}",
        mail.subject
    );
    assert!(
        mail.text_body.contains("password-reset request"),
        "expected English body, got: {}",
        mail.text_body
    );
}

// Helper: configure the singleton smtp_config row to enabled with
// a dummy host. The InMemoryMailSender bypasses SMTP entirely, so
// the host value never needs to be reachable.
async fn enable_smtp_with_inmemory_mailer(state: &AppState) {
    use chrono::Utc;
    let now = Utc::now();
    sui_id_store::repos::smtp_config::upsert(
        &state.db,
        &sui_id_store::models::SmtpConfigRow {
            enabled: true,
            host: "smtp.test".into(),
            port: 587,
            tls_mode: sui_id_store::models::SmtpTlsMode::StartTls,
            username: None,
            password_enc: None,
            from_address: "sui-id@example.test".into(),
            from_name: Some("sui-id".into()),
            base_url: "https://sui-id.example.test".into(),
            created_at: now,
            updated_at: now,
        },
    )
    .expect("smtp upsert");
}

// ---------- v0.24.0: HIBP password breach check ----------

/// Helper: build an AppState with a pre-programmed
/// `InMemoryHibpClient`. The mailer is also captured so the test
/// suite gets the same `(state, mailer)` shape `test_app_with_mailer`
/// returns; HIBP is the third return so call sites can opt in.
fn test_app_with_hibp() -> (
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
        },
        security: sui_id::config::SecurityConfig::default(),
    };
    let mailer = std::sync::Arc::new(sui_id_core::mail::InMemoryMailSender::new());
    let mailer_dyn: std::sync::Arc<dyn sui_id_core::mail::MailSender> = mailer.clone();
    let hibp = std::sync::Arc::new(sui_id_core::hibp::test_support::InMemoryHibpClient::new());
    let hibp_dyn: std::sync::Arc<dyn sui_id_core::hibp::HibpClient> = hibp.clone();
    let state = AppState::new(db, cfg, SETUP_TOKEN.into(), mailer_dyn, hibp_dyn);
    (state, mailer, hibp)
}

/// Set the server-settings `hibp_mode` directly. Tests use this
/// to flip between modes without going through the (yet-to-be-
/// added in v0.24.0) admin settings page.
fn set_hibp_mode(state: &AppState, mode: sui_id_store::models::HibpMode) {
    sui_id_store::repos::server_settings::update_hibp_mode(
        &state.db,
        mode,
        chrono::Utc::now(),
    )
    .expect("update hibp_mode");
}

/// Default mode is `Warn`; setup wizard accepts a clean password.
/// The InMemoryHibpClient with no plan returns NotBreached, so
/// the setup completes normally — same shape as pre-v0.24.0.
#[tokio::test]
async fn setup_wizard_accepts_clean_password_in_warn_mode() {
    let (state, _mailer, _hibp) = test_app_with_hibp();
    // mode = warn (default).
    let _session = complete_setup_and_login(&state).await;
    // Got here = success.
}

/// In `Warn` mode, a breached password is accepted (the setup
/// completes) but a tracing warning is emitted at the call site.
/// We can't easily assert on tracing output from the e2e test, so
/// we just confirm the happy path still completes when the HIBP
/// stub flags the password.
#[tokio::test]
async fn setup_wizard_accepts_breached_password_in_warn_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    // Pre-program the HIBP stub: PASSWORD has been seen 9001 times.
    hibp.set_breached(PASSWORD, 9001);

    // Setup completes despite the breach hit.
    let _session = complete_setup_and_login(&state).await;
}

/// In `Block` mode, a breached password is refused with a 400.
/// The form re-renders with a flash message. The setup wizard
/// can be retried with a different (un-breached) password.
#[tokio::test]
async fn setup_wizard_rejects_breached_password_in_block_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block);
    // Pre-program: the test password is breached.
    hibp.set_breached(PASSWORD, 12345);

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "setup_token={SETUP_TOKEN}&username={USERNAME}&password={pw}\
                     &confirm_password={pw}&display_name=&email=",
                    pw = PASSWORD,
                )))
                .expect("req"),
        )
        .await
        .expect("setup admin");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let body = std::str::from_utf8(&bytes).expect("utf8");
    assert!(
        body.contains("過去のデータ漏洩"),
        "expected breach flash message, got: {}",
        &body[..body.len().min(500)]
    );
    // No admin row should have been created.
    let users_count: i64 = state
        .db
        .with_conn(|conn| {
            conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
                .map_err(Into::into)
        })
        .expect("count");
    assert_eq!(users_count, 0);
}

/// `Block` mode with an unbreached password proceeds normally.
#[tokio::test]
async fn setup_wizard_accepts_clean_password_in_block_mode() {
    let (state, _mailer, _hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block);
    let _session = complete_setup_and_login(&state).await;
}

/// `Off` mode skips the check entirely, so a "breached" password
/// in the stub still goes through. Verifies the short-circuit
/// path (no client call at all).
#[tokio::test]
async fn setup_wizard_off_mode_skips_check() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Off);
    hibp.set_breached(PASSWORD, 999);
    let _session = complete_setup_and_login(&state).await;
}

/// Fail-open: when the HIBP API is unavailable (the stub returns
/// `Unavailable`), `Block` mode lets the password through anyway.
/// This is the documented policy in migration 0017 — a flaky
/// external API must not lock an admin out of password setting.
#[tokio::test]
async fn setup_wizard_fails_open_when_hibp_unavailable_in_block_mode() {
    let (state, _mailer, hibp) = test_app_with_hibp();
    set_hibp_mode(&state, sui_id_store::models::HibpMode::Block);
    hibp.set_unavailable(PASSWORD);
    // Setup completes despite Block mode + the would-be-breached
    // password — fail-open.
    let _session = complete_setup_and_login(&state).await;
}

// ---------- v0.25.0: Idle session timeout + concurrent session cap ----------

/// Default mode: both knobs are 0, so an idle / over-cap session
/// behaves identically to pre-v0.25.0. Pin this so we don't break
/// the "no opt-in = no behaviour change" promise.
#[tokio::test]
async fn session_no_idle_timeout_when_disabled() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Reach back into the DB and make the session look like it
    // was last used 30 days ago. With idle_session_timeout_secs
    // = 0 (default), this should still be valid.
    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    let stale = chrono::Utc::now() - chrono::Duration::days(30);
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE user_id = ?2",
                rusqlite::params![stale, user.id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .expect("set stale");
    // Hitting an admin page should still 200 OK.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin");
    assert_eq!(resp.status(), StatusCode::OK);
}

/// With idle timeout enabled, an authenticated request whose
/// session has been idle past the window is rejected.
#[tokio::test]
async fn session_idle_timeout_revokes_after_window() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Configure a 60-second idle timeout.
    sui_id_store::repos::server_settings::update_idle_session_timeout(
        &state.db,
        60,
        chrono::Utc::now(),
    )
    .expect("set timeout");
    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    let stale = chrono::Utc::now() - chrono::Duration::seconds(120);
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE user_id = ?2",
                rusqlite::params![stale, user.id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .expect("set stale");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin");
    // The CurrentAdmin extractor should refuse — redirect to
    // /admin/login or 401, depending on the rejection mapping.
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED
            || resp.status().is_redirection(),
        "expected redirect or 401, got {}",
        resp.status()
    );
    // The session has been revoked in-place.
    let count: i64 = state
        .db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1 AND revoked_at IS NULL",
                rusqlite::params![user.id.to_string()],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .expect("count");
    assert_eq!(count, 0, "expected session to be revoked");
}

/// FIFO eviction: cap = 2, login 3 times → first session is
/// auto-revoked after the 3rd login.
#[tokio::test]
async fn session_cap_evicts_oldest_in_fifo() {
    let state = test_app();
    // First login also runs setup. After this, there is 1 active
    // session.
    let s1 = complete_setup_and_login(&state).await;
    // Set cap = 2 (only valid post-setup since the row needs to
    // exist).
    sui_id_store::repos::server_settings::update_max_concurrent_sessions(
        &state.db,
        2,
        chrono::Utc::now(),
    )
    .expect("set cap");

    // Login twice more; each via the regular login form.
    let login_once = || async {
        let body = format!("username={USERNAME}&password={pw}", pw = urlencode(PASSWORD));
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
        assert!(
            resp.status().is_redirection() || resp.status() == StatusCode::SEE_OTHER,
            "expected login redirect, got {}",
            resp.status()
        );
        extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie")
    };
    let s2 = login_once().await;
    let s3 = login_once().await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    // Active count is now 2 (cap respected).
    let active: i64 = sui_id_store::repos::sessions::count_active_for_user(
        &state.db,
        user.id,
        chrono::Utc::now(),
    )
    .expect("count");
    assert_eq!(active, 2, "expected 2 active after 3 logins with cap 2");

    // s1 should have been revoked; s2 and s3 should be live.
    let still_active = |sid_cookie: &str| {
        use std::str::FromStr;
        let sid =
            sui_id_shared::ids::SessionId::from_str(sid_cookie).expect("parse sid");
        let row =
            sui_id_store::repos::sessions::get(&state.db, sid).expect("get");
        row.revoked_at.is_none()
    };
    assert!(!still_active(&s1), "s1 should be revoked (FIFO)");
    assert!(still_active(&s2), "s2 should remain");
    assert!(still_active(&s3), "s3 should remain");
}

/// Cap = 0 (default) — cap disabled, login N times yields N
/// sessions.
#[tokio::test]
async fn session_cap_disabled_keeps_all_sessions() {
    let state = test_app();
    let _s1 = complete_setup_and_login(&state).await;
    // Default cap = 0 = disabled.

    // Login twice more without changing the cap.
    let login_once = || async {
        let body = format!("username={USERNAME}&password={pw}", pw = urlencode(PASSWORD));
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
        let _ = resp.status();
    };
    login_once().await;
    login_once().await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");
    let active: i64 = sui_id_store::repos::sessions::count_active_for_user(
        &state.db,
        user.id,
        chrono::Utc::now(),
    )
    .expect("count");
    assert_eq!(active, 3, "all 3 sessions active when cap disabled");
}

/// Admin POST /admin/settings/security/idle-timeout updates the
/// stored value.
#[tokio::test]
async fn admin_settings_security_idle_timeout_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&secs=900");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/idle-timeout")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db).expect("settings");
    assert_eq!(row.idle_session_timeout_secs, 900);
}

/// Admin POST /admin/settings/security/max-sessions updates the
/// stored cap. Also covers out-of-range rejection.
#[tokio::test]
async fn admin_settings_security_max_sessions_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&cap=5");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/max-sessions")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db).expect("settings");
    assert_eq!(row.max_concurrent_sessions, 5);

    // Out-of-range (>1000) is rejected.
    let body = format!("_csrf={csrf}&cap=99999");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/max-sessions")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx for out-of-range cap, got {}",
        resp.status()
    );
    // The cap stays at 5 (unchanged).
    let row = sui_id_store::repos::server_settings::get(&state.db).expect("settings");
    assert_eq!(row.max_concurrent_sessions, 5);
}

// ---------- v0.26.0: Master-key rotation ----------

/// End-to-end rotation: set up sui-id, enroll TOTP and a passkey,
/// configure SMTP password, then rotate the master key and assert
/// every sealed-row read still works under the new key.
#[tokio::test]
async fn rotation_reseal_succeeds_and_old_key_no_longer_decrypts() {
    use sui_id_core::key_rotation::rotate_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // The setup wizard creates a signing key (active) — that's
    // already one sealed row. Add an SMTP password to also exercise
    // the smtp_config path.
    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).expect("user");

    // Add an SMTP config row with a sealed password so rotation
    // has something to re-key in that table.
    {
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
                        b"my-smtp-password",
                        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
                    )
                    .expect("seal smtp pw"),
                ),
                from_address: "alice@example.test".into(),
                from_name: None,
                base_url: "https://idp.test".into(),
                created_at: now,
                updated_at: now,
            },
        )
        .expect("smtp upsert");
    }

    // Generate a brand-new master key.
    let new_key = MasterKey::generate();

    // The Database carries the old key. We open a SECOND handle
    // to the same underlying DB file? In-memory DBs cannot be
    // re-opened, so we test the rotation on the existing handle:
    // run rotation, then re-open under the new key and verify.
    //
    // The in-memory DB shares its connection across the
    // Database handle, so after rotation the same handle still
    // works (the key field is unchanged). Reading sealed columns
    // through that old handle would now FAIL — which is exactly
    // what we assert.
    let report = rotate_master_key(&state.db, &new_key).expect("rotate");

    assert!(
        report.signing_keys >= 1,
        "expected at least 1 signing key re-sealed, got {}",
        report.signing_keys
    );
    assert_eq!(report.smtp_config, 1, "smtp password should re-seal");
    assert!(report.total() >= 2);

    // The OLD key (still held by `state.db`) must no longer
    // decrypt the signing key column — it has been re-sealed
    // under `new_key`.
    let signing_row = sui_id_store::repos::signing_keys::active(&state.db).expect("active");
    let opened_old = sui_id_store::crypto::open(
        state.db.key(),
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_old.is_err(),
        "old key must no longer decrypt the re-sealed column"
    );
    // The NEW key decrypts it.
    let opened_new = sui_id_store::crypto::open(
        &new_key,
        &signing_row.private_key_enc,
        b"sui-id/signing_key/v1",
    );
    assert!(
        opened_new.is_ok(),
        "new key must decrypt the re-sealed column"
    );

    // SMTP password also re-sealed.
    let smtp_row = sui_id_store::repos::smtp_config::get(&state.db)
        .expect("smtp")
        .expect("smtp configured");
    let smtp_enc = smtp_row.password_enc.expect("password set");
    let opened_old_smtp = sui_id_store::crypto::open(
        state.db.key(),
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert!(opened_old_smtp.is_err());
    let opened_new_smtp = sui_id_store::crypto::open(
        &new_key,
        &smtp_enc,
        sui_id_store::repos::smtp_config::SMTP_PASSWORD_AAD,
    );
    assert_eq!(
        opened_new_smtp.expect("decrypt with new key"),
        b"my-smtp-password"
    );

    // Audit row was appended for the rotation event by the CLI
    // (we are not running through the CLI in this test, so we
    // don't check for the row here — that's covered by the CLI-
    // path test).

    // Avoid an unused-variable warning.
    let _ = user;
}

/// Sanity: rotation on a DB with no sealed rows runs successfully
/// and reports zeroes (other than the signing-key the setup
/// wizard always creates). Pin this so future migrations that
/// add new sealed columns have to update the rotation entry
/// list.
#[tokio::test]
async fn rotation_on_minimal_db_only_rekeys_signing_key() {
    use sui_id_core::key_rotation::rotate_master_key;
    use sui_id_store::crypto::MasterKey;

    let state = test_app();
    let _session = complete_setup_and_login(&state).await;
    // Brand-new install: only the active signing key exists as
    // a sealed row. (Refresh tokens are issued by /token; not
    // exercised here. TOTP / WebAuthn / SMTP all require admin
    // action.)
    let new_key = MasterKey::generate();
    let report = rotate_master_key(&state.db, &new_key).expect("rotate");
    assert_eq!(report.signing_keys, 1);
    assert_eq!(report.refresh_tokens, 0);
    assert_eq!(report.user_totp_secrets, 0);
    assert_eq!(report.user_totp_recovery_codes, 0);
    assert_eq!(report.user_webauthn_credentials, 0);
    assert_eq!(report.smtp_config, 0);
    assert_eq!(report.total(), 1);
}

// ---------- v0.28.0: Dev mode ----------

/// Default-seed dev mode: open an in-memory DB, run apply_seed
/// with hardcoded defaults, assert that the admin and the two
/// test users land in the DB, and the OIDC test client is
/// usable.
#[tokio::test]
async fn dev_mode_default_seed_creates_admin_users_and_client() {
    use sui_id::dev_mode::{apply_seed, open_dev_db, DevSeed};

    let db = open_dev_db(None).expect("open in-memory dev db");
    let setup_token = "test-dev-setup-token";
    let seed = DevSeed::default();
    let clock = sui_id_core::time::system_clock();
    let outcome = apply_seed(&db, &clock, setup_token, &seed).expect("apply_seed");

    // Admin lands.
    let admin =
        sui_id_store::repos::users::find_by_username(&db, "admin").expect("admin");
    assert!(admin.is_admin);
    assert_eq!(admin.id, outcome.admin_user_id);

    // Two default users land.
    let alice =
        sui_id_store::repos::users::find_by_username(&db, "alice").expect("alice");
    assert!(!alice.is_admin);
    let bob = sui_id_store::repos::users::find_by_username(&db, "bob").expect("bob");
    assert!(!bob.is_admin);

    // One client landed.
    assert_eq!(outcome.clients.len(), 1);
    let client = &outcome.clients[0];
    assert_eq!(client.name, "Dev test client");
    assert_eq!(client.client_secret.as_deref(), Some("test-secret"));
    assert!(client.redirect_uris.iter().any(|u| u.contains(":3000")));
}

/// Flag overrides: `--dev-admin-password` and
/// `--dev-client-secret` reach apply_seed.
#[tokio::test]
async fn dev_mode_flag_overrides_apply_to_seed() {
    use sui_id::dev_mode::{apply_seed, open_dev_db, DevFlagOverrides, DevSeed};

    let db = open_dev_db(None).expect("open");
    let setup_token = "test-dev-setup-token";
    let mut seed = DevSeed::default();
    seed.apply_overrides(DevFlagOverrides {
        admin_password: Some("hunter2-and-then-some".into()),
        client_secret: Some("custom-cs-value-xyz".into()),
    });
    let clock = sui_id_core::time::system_clock();
    let outcome = apply_seed(&db, &clock, setup_token, &seed).expect("apply");

    // Login as admin with the overridden password should succeed.
    let result = sui_id_core::session::login(
        &db,
        &clock,
        "admin",
        "hunter2-and-then-some",
        0,
    )
    .expect("admin login");
    let _ = result;

    // The first client's effective secret is the override.
    assert_eq!(
        outcome.clients[0].client_secret.as_deref(),
        Some("custom-cs-value-xyz")
    );
}

/// TOML seed: a custom user list and client list replace the
/// defaults, and `public = true` produces a PKCE-only client.
#[tokio::test]
async fn dev_mode_toml_seed_replaces_defaults() {
    use sui_id::dev_mode::{apply_seed, load_seed_from_toml, open_dev_db};

    let toml = r#"
[admin]
username = "ops"
password = "ops-pw-strong-enough"

[[user]]
username = "u1"
password = "u1-pw-strong-enough"

[[client]]
name = "spa"
redirect_uris = ["http://localhost:5173/cb"]
public = true

[[client]]
name = "api"
redirect_uris = ["http://localhost:8000/cb"]
client_secret = "api-secret-strong"
"#;

    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev-seed.toml");
    std::fs::write(&path, toml).expect("write toml");

    let seed = load_seed_from_toml(&path).expect("load seed");
    assert_eq!(seed.admin.username, "ops");
    assert_eq!(seed.users.len(), 1);
    assert_eq!(seed.clients.len(), 2);

    let db = open_dev_db(None).expect("open");
    let clock = sui_id_core::time::system_clock();
    let outcome =
        apply_seed(&db, &clock, "test-dev-setup-token", &seed).expect("apply");

    // Admin login works.
    let _ = sui_id_core::session::login(&db, &clock, "ops", "ops-pw-strong-enough", 0)
        .expect("admin login");

    // u1 exists, alice and bob do NOT.
    let _u1 =
        sui_id_store::repos::users::find_by_username(&db, "u1").expect("u1 exists");
    let alice = sui_id_store::repos::users::find_by_username(&db, "alice");
    assert!(alice.is_err(), "alice should not exist when TOML supplies users");

    // First client is public (PKCE-only): no secret.
    assert_eq!(outcome.clients.len(), 2);
    assert!(outcome.clients[0].client_secret.is_none());
    // Second client has the supplied secret.
    assert_eq!(
        outcome.clients[1].client_secret.as_deref(),
        Some("api-secret-strong")
    );
}

/// Pinning the dev DB to a path: the file is created, and a
/// pre-existing file is truncated each restart.
#[tokio::test]
async fn dev_mode_pinned_db_truncates_existing_file() {
    use sui_id::dev_mode::open_dev_db;

    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev.sqlite");
    // Pre-create the file with junk content.
    std::fs::write(&path, b"junk").expect("pre-create");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 4);

    // open_dev_db should remove and re-create.
    let db = open_dev_db(Some(&path)).expect("open with path");
    drop(db);
    // The file now exists and is a real SQLite DB (size > 4 bytes
    // due to the migrations applied at open time).
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 4, "expected SQLite file, size = {size}");
}

/// resolve_seed: TOML overrides defaults, flag overrides apply
/// on top.
#[tokio::test]
async fn dev_mode_resolve_seed_applies_priority() {
    use sui_id::dev_mode::{resolve_seed, DevFlagOverrides};

    let toml = r#"
[admin]
username = "admin-from-toml"
password = "toml-pw-strong"
"#;
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = dir.path().join("dev-seed.toml");
    std::fs::write(&path, toml).expect("write");

    let (seed, source) = resolve_seed(
        Some(&path),
        DevFlagOverrides {
            admin_password: Some("flag-overrides-toml".into()),
            client_secret: None,
        },
    )
    .expect("resolve");
    // TOML supplied the username; flag overrode the password.
    assert_eq!(seed.admin.username, "admin-from-toml");
    assert_eq!(seed.admin.password, "flag-overrides-toml");
    assert!(source.contains("TOML"));
}
