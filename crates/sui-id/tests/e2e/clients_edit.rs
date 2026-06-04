//! Admin client edit (v0.8.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::build_router;

use tower::ServiceExt;
use super::common::*;

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
    let row = sui_id_store::repos::clients::get(&state.db, id).await.expect("get");
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
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice").await
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
        &state.caches,
    ).await
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

