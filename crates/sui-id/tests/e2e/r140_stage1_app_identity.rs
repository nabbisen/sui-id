//! RFC 140 stage 1 — C10: an e2e assertion through the real HTTP admin
//! edit route, not the sealed command directly, same precedent as
//! `r094_m2b_wave4_stage2_client_update_form.rs`. Plus the dispatch's
//! own required regression: a stage-1 edit must not blank `logo_uri`,
//! which this form does not manage.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

async fn get_edit_csrf(
    state: &sui_id::AppState,
    session: &str,
    client_id: &str,
) -> (String, axum::response::Response) {
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
    (csrf, resp)
}

#[tokio::test]
async fn clients_edit_post_updates_app_identity_and_is_audited() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;
    let (csrf, _) = get_edit_csrf(&state, &session, &client_id).await;

    let before: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.app_identity_changed'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");

    let body = format!(
        "name=rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb\
         &homepage_uri=https%3A%2F%2Frp.test%2F\
         &privacy_policy_uri=https%3A%2F%2Frp.test%2Fprivacy\
         &tos_uri=https%3A%2F%2Frp.test%2Ftos\
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
    assert_eq!(row.homepage_uri.as_deref(), Some("https://rp.test/"));
    assert_eq!(
        row.privacy_policy_uri.as_deref(),
        Some("https://rp.test/privacy")
    );
    assert_eq!(row.tos_uri.as_deref(), Some("https://rp.test/tos"));

    let note: Option<String> = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'client.app_identity_changed' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "homepage_uri_changed").as_deref(),
        Some("true"),
        "the audit record must report the form's fields, not just the DB row"
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "privacy_policy_uri_changed").as_deref(),
        Some("true")
    );
    assert_eq!(
        sui_id_store::registry::note_field(&note, "tos_uri_changed").as_deref(),
        Some("true")
    );

    let after: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'client.app_identity_changed'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(
        after,
        before + 1,
        "exactly one client.app_identity_changed event"
    );
}

/// RFC 140 stage 1 §3b's own explicit requirement: the stage-1 edit
/// form does not manage `logo_uri`, and must not blank it. Seed a
/// client with a `logo_uri`, submit the edit form (the three fields
/// this stage does manage), and assert the logo survives through the
/// real HTTP route -- not only through the sealed command directly
/// (already covered in `commands/tests/runner/client_app_identity.rs`).
#[tokio::test]
async fn clients_edit_post_does_not_blank_logo_uri() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    use sui_id_shared::ids::ClientId;
    let id = client_id.parse::<ClientId>().expect("parse");
    sui_id_store::repos::clients::update_app_identity(
        &state.db,
        id,
        Some("https://rp.test/logo.png".into()),
        None,
        None,
        None,
        chrono::Utc::now(),
    )
    .await
    .expect("seed logo_uri");

    let (csrf, _) = get_edit_csrf(&state, &session, &client_id).await;
    let body = format!(
        "name=rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb\
         &homepage_uri=https%3A%2F%2Frp.test%2F\
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

    let row = sui_id_store::repos::clients::get(&state.db, id)
        .await
        .expect("get");
    assert_eq!(
        row.logo_uri.as_deref(),
        Some("https://rp.test/logo.png"),
        "a stage-1 edit must not blank logo_uri -- this form does not manage it"
    );
    assert_eq!(
        row.homepage_uri.as_deref(),
        Some("https://rp.test/"),
        "the field this form does manage must still have been written"
    );
}

/// RFC 140 stage 1 §2's resolution: administrator input is validated at
/// the HTTP layer with `validate_application_uri` (HTTPS only) — not
/// `is_valid_app_uri`'s laxer `http://localhost` allowance. A
/// `localhost` URL submitted through this form must be rejected, and
/// must not have been written, confirming the stricter rule is what
/// actually runs here, not merely declared in a doc comment.
#[tokio::test]
async fn clients_edit_post_rejects_a_non_https_homepage_uri() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;
    let (csrf, _) = get_edit_csrf(&state, &session, &client_id).await;

    let body = format!(
        "name=rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb\
         &homepage_uri=http%3A%2F%2Flocalhost%3A8080%2F\
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
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a localhost homepage_uri must be rejected by the stricter HTTP-layer rule"
    );

    use sui_id_shared::ids::ClientId;
    let id = client_id.parse::<ClientId>().expect("parse");
    let row = sui_id_store::repos::clients::get(&state.db, id)
        .await
        .expect("get");
    assert_eq!(
        row.homepage_uri, None,
        "the rejected value must not have been written"
    );
}
