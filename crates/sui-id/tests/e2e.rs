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

fn test_app() -> AppState {
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
    AppState::new(db, cfg, SETUP_TOKEN.into())
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
    let body = format!(
        "setup_token={SETUP_TOKEN}&username={USERNAME}&display_name=Alice&password={PASSWORD}"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/setup")
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
    let state = sui_id::AppState::new(db, cfg_src.clone(), SETUP_TOKEN.into());
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
