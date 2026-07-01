//! Scope policy and `post_logout_redirect_uris` (v0.6.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- scope policy and post_logout_redirect_uris (v0.6.0) ----------

#[tokio::test]
async fn authorize_rejects_scope_outside_client_policy() {
    use sui_id_core::admin::CreateClientSpec;
    use sui_id_store::repos::clients;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
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
        &state.caches,
    )
    .await
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
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
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
    let _ = clients::get(&state.db, created.row.id)
        .await
        .expect("still there");
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
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
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
        .await
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
        &state.caches,
    )
    .await
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
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        loc.starts_with("https://rp.test/cb?code="),
        "expected code, got: {loc}"
    );
}

#[tokio::test]
async fn logout_uses_post_logout_redirect_uris_when_registered() {
    use sui_id_core::admin::CreateClientSpec;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice")
        .await
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
        &state.caches,
    )
    .await
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
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
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
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
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
        .await
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
        &state.caches,
    )
    .await
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
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.starts_with("https://rp.test/cb"));
}
