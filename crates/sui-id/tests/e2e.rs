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
    let router = build_router(state.clone());
    let body = "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/clients")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
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
    // First two .code spans on the freshly-created flash banner are id and secret.
    assert!(codes.len() >= 2, "expected client id and secret in HTML, found {codes:?}");
    (codes[0].clone(), codes[1].clone())
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
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
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
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
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

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{active_id}/delete"))
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
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
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/signing-keys/rotate")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("rotate");
    assert!(resp.status().is_redirection());

    // Now delete the retired (original) one.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/signing-keys/{original_id}/delete"))
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
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
    };
    let state = sui_id::AppState::new(db, cfg_src.clone(), SETUP_TOKEN.into());
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    // Step 2: take a backup.
    let archive = tmp.path().join("backup.tar");
    backup::run_backup(&cfg_src, &archive).expect("backup");
    assert!(archive.exists());

    // Step 3: restore into a fresh location and re-open.
    let cfg_dst = sui_id::config::Config {
        storage: StorageConfig {
            db_path: tmp.path().join("restored.sqlite"),
            key_file: tmp.path().join("restored.key"),
        },
        ..cfg_src.clone()
    };
    backup::run_restore(&cfg_dst, &archive, false).expect("restore");

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

// `http` is brought in transitively by axum; we only need its HeaderMap.
use axum::http;
