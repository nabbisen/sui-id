//! RFC 094 M2b wave 4 stage 2 — C03 (the client-update form): an e2e
//! assertion through the real HTTP admin edit route, not the sealed
//! command directly. C09's own defect (`client.consent_policy_changed`
//! emitted nowhere in `crates/`) escaped a reader checking
//! `identity/admin/clients.rs`'s call sites because the handler reached
//! into `sui_id_store::repos` directly; this test goes through the
//! route so a regression of that kind shows up here too.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

#[tokio::test]
async fn client_edit_form_updates_scopes_uris_and_consent_policy_and_is_audited() {
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
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.update_form'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");

    let body = format!(
        "name=rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb\
         &allowed_scopes=openid+profile+email\
         &post_logout_redirect_uris=https%3A%2F%2Frp.test%2Fbye\
         &consent_policy=always\
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

    use sui_id_shared::ids::ClientId;
    let id = client_id.parse::<ClientId>().expect("parse");
    let row = sui_id_store::repos::clients::get(&state.db, id)
        .await
        .expect("get");
    assert_eq!(row.allowed_scopes, "openid profile email");
    assert_eq!(
        row.post_logout_redirect_uris,
        vec!["https://rp.test/bye".to_string()]
    );
    assert_eq!(
        row.consent_policy,
        sui_id_store::models::ConsentPolicy::Always
    );

    let note: Option<String> = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'client.update_form' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "allowed_scopes_count").as_deref(),
        Some("3"),
        "the audit record must report the form's fields, not just the DB row"
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "post_logout_redirect_uris_count").as_deref(),
        Some("1")
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "consent_policy").as_deref(),
        Some("always")
    );

    let after: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.update_form'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(
        after,
        before + 1,
        "exactly one client.update_form event for the three-field group"
    );
}
