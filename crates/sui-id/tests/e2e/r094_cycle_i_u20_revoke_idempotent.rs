//! RFC 094 cycle I stage 3, U20 — revoking an application's access is
//! idempotent at the HTTP layer.
//!
//! The sealed command returns `NotFound` when there was no grant to
//! revoke, and writes no audit row for a revocation that did not happen.
//! That part is covered by the store's own tests. This one covers what
//! the *user* sees: the only ways to reach `NotFound` are a double-submit
//! or a stale page, and in both the grant they wanted gone is gone, so
//! the page redirects rather than showing an error.
//!
//! `me_security/sessions.rs`'s `revoke_one` already treats an unknown
//! session id this way, in the same `/me` area. Added in review, with
//! the handler change it protects.

use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::build_router;
use sui_id_shared::ids::ClientId;

use super::common::*;
use tower::ServiceExt;

async fn post_revoke(
    state: &sui_id::AppState,
    session: &str,
    csrf: &str,
    client_id: &ClientId,
) -> axum::http::StatusCode {
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/me/apps/{client_id}/revoke"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    router.oneshot(req).await.expect("revoke").status()
}

#[tokio::test]
async fn revoking_an_already_revoked_grant_redirects_and_records_nothing_twice() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id_str, _) = create_client(&state, &session).await;
    let client_id: ClientId = client_id_str.parse().expect("client id");
    let csrf = fetch_csrf(&state, &session).await;

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("find user");

    // Seed the grant directly: this test is about the revoke, not about
    // driving the consent screen, which r094_m2a_consent_policy_persistence
    // already covers.
    sui_id_store::commands::grant_consent(
        &state.db,
        user.id,
        client_id,
        "openid".to_owned(),
        state.clock.now(),
    )
    .await
    .expect("seed the grant");

    let first = post_revoke(&state, &session, &csrf, &client_id).await;
    assert!(
        first.is_redirection(),
        "the first revoke redirects: got {first}"
    );

    // The double-submit. Before this handler change it was an error page.
    let second = post_revoke(&state, &session, &csrf, &client_id).await;
    assert!(
        second.is_redirection(),
        "a second revoke of the same grant must redirect, not error: got {second}"
    );

    let revocations: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.consent.revoked'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(
        revocations, 1,
        "exactly one revocation happened, so exactly one row records it"
    );
}
