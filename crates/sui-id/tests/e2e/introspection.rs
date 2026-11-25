//! RFC 7662 introspection + RFC 7009 revocation (v0.11.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};

use super::common::*;
use super::r102_stage1::sign_in;
use tower::ServiceExt;
use url::Url;

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
    let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
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
    let body =
        format!("token=this.is.not.a.token&client_id={client_id}&client_secret={client_secret}");
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
    let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
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
        let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
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
    let body = format!("token=garbage&client_id={client_id}&client_secret={client_secret}");
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

/// RFC 094 cycle J stage 10, R2, T05's own named test: `oauth_token::
/// revoke` reaches both token kinds (T06's refresh revocation, T07's
/// access-token deny-list insert) and is idempotent under RFC 7009
/// §2.2 -- the three properties `revoke_then_introspect_shows_inactive_
/// for_access_token`, `revoke_refresh_token_invalidates_subsequent_
/// refresh_grant` and `revoke_is_idempotent` already prove individually,
/// combined here under T05's own `test_id` so the write-commands row
/// names a real test.
#[tokio::test]
async fn t05_revoke_calls_both_token_kinds_and_is_idempotent() {
    let state = test_app();
    let (client_id, client_secret, access, refresh) = obtain_tokens(&state).await;

    // T07: revoking the access token reaches the deny list.
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
    assert_eq!(
        router.oneshot(req).await.expect("revoke access").status(),
        StatusCode::OK
    );
    let body = format!("token={access}&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect access");
    let v: serde_json::Value = serde_json::from_slice(&read_body(resp.into_body()).await).unwrap();
    assert_eq!(
        v["active"].as_bool(),
        Some(false),
        "T07: the access token must be inactive after revoke"
    );

    // T06: revoking the refresh token reaches refresh_tokens::revoke.
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
    assert_eq!(
        router.oneshot(req).await.expect("revoke refresh").status(),
        StatusCode::OK
    );
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
    let resp = router.oneshot(req).await.expect("refresh after revoke");
    assert!(
        resp.status().is_client_error(),
        "T06: a refresh grant on a revoked refresh token must fail; got {}",
        resp.status()
    );

    // RFC 7009 §2.2: revoking either, again, or a bogus token, is still 200.
    for body in [
        format!("token={access}&client_id={client_id}&client_secret={client_secret}"),
        format!("token={refresh}&client_id={client_id}&client_secret={client_secret}"),
        format!("token=garbage&client_id={client_id}&client_secret={client_secret}"),
    ] {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/oauth2/revoke")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .expect("req");
        assert_eq!(
            router.oneshot(req).await.expect("re-revoke").status(),
            StatusCode::OK,
            "revoke must stay idempotent"
        );
    }
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
    let body = format!("token={access}&client_id={client_b}&client_secret={secret_b}");
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
    assert!(
        v["introspection_endpoint"]
            .as_str()
            .unwrap()
            .ends_with("/oauth2/introspect")
    );
    assert!(
        v["revocation_endpoint"]
            .as_str()
            .unwrap()
            .ends_with("/oauth2/revoke")
    );
    let methods = v["introspection_endpoint_auth_methods_supported"]
        .as_array()
        .unwrap();
    let methods_set: Vec<_> = methods.iter().filter_map(|x| x.as_str()).collect();
    assert!(methods_set.contains(&"client_secret_basic"));
    // Public clients (auth method "none") must NOT be listed.
    assert!(!methods_set.contains(&"none"));
}

// ---------- introspection must agree with userinfo about who may be served ----------
//
// An administrator can't disable or delete themselves, so these need a
// second, ordinary user as the resource owner. `obtain_tokens` above is
// kept as-is (it's used by tests that don't care who the owner is); this
// is the same shape, parameterized over the owner's own session.

const TARGET_PASSWORD: &str = "a-very-strong-target-password";

/// One client, pre-authorized for `username` (an ordinary user, created
/// and signed in for real, never by writing a row directly), with
/// everything [`token_as`] needs to mint another token for that same
/// user later -- the point of a second mint being that it postdates a
/// state change the first token was issued before.
struct OwnerClient {
    user_id: sui_id_shared::ids::UserId,
    username: String,
    client_id: String,
    client_secret: String,
}

async fn ordinary_user_with_client(
    state: &AppState,
    admin_session: &str,
    username: &str,
) -> OwnerClient {
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user must already exist (complete_setup_and_login)")
        .id;
    let user = create_user_with_password(
        &state.db,
        &state.clock,
        &admin_actor_for(admin_id),
        sui_id_core::admin::CreateUserSpec {
            username,
            display_name: None,
            email: None,
            is_admin: false,
        },
        TARGET_PASSWORD,
    )
    .await
    .expect("create ordinary user");
    let (client_id, client_secret) = create_client(state, admin_session).await;
    OwnerClient {
        user_id: user.id,
        username: username.to_owned(),
        client_id,
        client_secret,
    }
}

/// Sign this user in and run the authorize + token exchange against
/// its own client, returning a fresh access token issued *now*.
async fn token_as(state: &AppState, owner: &OwnerClient) -> String {
    let owner_session = sign_in(state, &owner.username, TARGET_PASSWORD).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=x&nonce=n&code_challenge={challenge}&code_challenge_method=S256",
        owner.client_id
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={owner_session}"))
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
         &client_id={}&client_secret={}&code_verifier={verifier}",
        owner.client_id, owner.client_secret
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
    let json: serde_json::Value =
        serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned()
}

async fn introspect_active(state: &AppState, owner: &OwnerClient, access: &str) -> Option<bool> {
    let body = format!(
        "token={access}&client_id={}&client_secret={}",
        owner.client_id, owner.client_secret
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    let v: serde_json::Value =
        serde_json::from_slice(&read_body(resp.into_body()).await).expect("json");
    v["active"].as_bool()
}

async fn userinfo_ok(state: &AppState, access: &str) -> bool {
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/oauth2/userinfo")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .body(Body::empty())
        .expect("req");
    router.oneshot(req).await.expect("userinfo").status() == StatusCode::OK
}

#[tokio::test]
async fn introspection_agrees_with_userinfo_about_a_disabled_user() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let owner = ordinary_user_with_client(&state, &admin_session, "bob").await;
    let access = token_as(&state, &owner).await;

    // Before: active everywhere -- proves the test, not a broken token.
    assert_eq!(introspect_active(&state, &owner, &access).await, Some(true));
    assert!(userinfo_ok(&state, &access).await);

    sui_id_core::admin::set_user_disabled(
        &state.db,
        &state.clock,
        &admin_actor_on_session(admin_id, &admin_session),
        owner.user_id,
        true,
        None,
    )
    .await
    .expect("disable bob");

    // After: inactive everywhere, same token.
    assert_eq!(
        introspect_active(&state, &owner, &access).await,
        Some(false),
        "introspection must refuse a disabled user's unexpired access token"
    );
    assert!(
        !userinfo_ok(&state, &access).await,
        "userinfo already refused this; introspection must agree"
    );
}

#[tokio::test]
async fn introspection_agrees_with_userinfo_about_a_deleted_user() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let owner = ordinary_user_with_client(&state, &admin_session, "bob").await;
    let access = token_as(&state, &owner).await;

    assert_eq!(introspect_active(&state, &owner, &access).await, Some(true));
    assert!(userinfo_ok(&state, &access).await);

    sui_id_core::admin::delete_user(
        &state.db,
        &state.clock,
        &admin_actor_on_session(admin_id, &admin_session),
        owner.user_id,
        None,
    )
    .await
    .expect("delete bob");

    assert_eq!(
        introspect_active(&state, &owner, &access).await,
        Some(false),
        "introspection must refuse a deleted user's unexpired access token"
    );
    assert!(!userinfo_ok(&state, &access).await);
}

#[tokio::test]
async fn a_token_issued_after_re_enabling_is_active_again() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let owner = ordinary_user_with_client(&state, &admin_session, "bob").await;
    let actor = admin_actor_on_session(admin_id, &admin_session);

    sui_id_core::admin::set_user_disabled(
        &state.db,
        &state.clock,
        &actor,
        owner.user_id,
        true,
        None,
    )
    .await
    .expect("disable");
    sui_id_core::admin::set_user_disabled(
        &state.db,
        &state.clock,
        &actor,
        owner.user_id,
        false,
        None,
    )
    .await
    .expect("re-enable");

    // A fresh token, issued after re-enabling.
    let access = token_as(&state, &owner).await;
    assert_eq!(
        introspect_active(&state, &owner, &access).await,
        Some(true),
        "a token issued after re-enabling must be active"
    );
    assert!(userinfo_ok(&state, &access).await);
}

/// Walks a single token through enabled, disabled and deleted, asserting
/// userinfo and introspection agree at every step. `userinfo_ok` already
/// holds (it's the reference behaviour this stage matches introspection
/// to); the point is that `introspect_active` tracks it exactly, not
/// just that both happen to refuse a disabled user for their own
/// separate reasons.
#[tokio::test]
async fn introspection_and_userinfo_agree_on_whether_a_user_can_be_served() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    let owner = ordinary_user_with_client(&state, &admin_session, "bob").await;
    let actor = admin_actor_on_session(admin_id, &admin_session);

    async fn assert_agree_as(state: &AppState, owner: &OwnerClient, access: &str, expected: bool) {
        let introspection = introspect_active(state, owner, access).await;
        let userinfo = userinfo_ok(state, access).await;
        assert_eq!(
            introspection,
            Some(userinfo),
            "introspection and userinfo disagree"
        );
        assert_eq!(introspection, Some(expected));
    }

    // Enabled.
    let access = token_as(&state, &owner).await;
    assert_agree_as(&state, &owner, &access, true).await;

    // Disabled.
    sui_id_core::admin::set_user_disabled(
        &state.db,
        &state.clock,
        &actor,
        owner.user_id,
        true,
        None,
    )
    .await
    .expect("disable");
    assert_agree_as(&state, &owner, &access, false).await;

    // Deleted (from disabled -- deletion doesn't require a prior enable).
    sui_id_core::admin::delete_user(&state.db, &state.clock, &actor, owner.user_id, None)
        .await
        .expect("delete");
    assert_agree_as(&state, &owner, &access, false).await;
}

/// `delete_user`'s real soft-delete sets `is_disabled` alongside
/// `is_deleted` (`users::soft_delete_within_tx`), so a test that only
/// goes through that command can't tell a fix that checks both flags
/// apart from one that checks `is_disabled` alone -- the exact
/// mutation dispatch §3 names. This flips `is_deleted` on its own, by
/// direct SQL (the real command can't produce this state; a test
/// helper reaching around it is the established way to isolate a
/// case production code can't, per `common::create_user_with_password`'s
/// own precedent), to prove the check reads `is_deleted` too, not only
/// as a side effect of what `delete_user` happens to also set.
#[tokio::test]
async fn introspection_refuses_a_user_marked_deleted_without_being_marked_disabled() {
    let state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    let owner = ordinary_user_with_client(&state, &admin_session, "bob").await;
    let access = token_as(&state, &owner).await;
    assert_eq!(introspect_active(&state, &owner, &access).await, Some(true));

    let uid = owner.user_id;
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE users SET is_deleted = 1 WHERE id = ?1",
                [uid.to_string()],
            )?)
        })
        .await
        .expect("mark deleted without disabling");
    assert!(
        !sui_id_store::repos::users::get(&state.db, uid)
            .await
            .expect("user")
            .is_disabled,
        "is_disabled must still be false -- otherwise this isn't isolating is_deleted"
    );

    assert_eq!(
        introspect_active(&state, &owner, &access).await,
        Some(false),
        "a user marked deleted, even with is_disabled still false, must be inactive"
    );
}
