//! RFC 094 M2b wave 4 stage 3 — C02 (update client basic info): an e2e
//! assertion through the real HTTP admin edit route, asserting the audit
//! record and not only the field values, the same distinction stage 1's
//! review asked every stage since to carry. C01's creation and C07's
//! secret rotation already go through this same admin area (`create_client`
//! e2e-tested in `clients_edit.rs`'s own suite and `scope_logout.rs`'s;
//! `rotate_client_secret` has its own confirm-screen route); this test
//! targets C02 specifically because it is the one of the four sharing the
//! client-update form with stage 2's C03, and stage 2's own e2e test
//! covers C03's three fields but asserted nothing about C02's two.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

#[tokio::test]
async fn client_edit_form_updates_basic_info_and_is_audited() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

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

    let before: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.update'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");

    let body = "name=renamed-via-c02&redirect_uris=https%3A%2F%2Frp.test%2Fnew-cb\
         &allowed_scopes=openid&consent_policy=none&_csrf=CSRF_PLACEHOLDER"
        .replace("CSRF_PLACEHOLDER", &csrf);
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

    use sui_id_shared::ids::ClientId;
    let id = client_id.parse::<ClientId>().expect("parse");
    let row = sui_id_store::repos::clients::get(&state.db, id)
        .await
        .expect("get");
    assert_eq!(row.name, "renamed-via-c02");
    assert_eq!(
        row.redirect_uris,
        vec!["https://rp.test/new-cb".to_string()]
    );

    let after: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.update'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(after, before + 1, "exactly one client.update event");
}
