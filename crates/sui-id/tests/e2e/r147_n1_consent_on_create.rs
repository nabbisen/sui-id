//! RFC 147 §5: the create handler accepts `consent_policy`, and applies
//! it exactly as the edit handler already does -- not just rendered in
//! the form.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

#[tokio::test]
async fn create_handler_applies_the_submitted_consent_policy() {
    let state = test_app();
    let session_cookie = complete_setup_and_login(&state).await;

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/clients")
        .header(header::COOKIE, format!("sui_id_session={session_cookie}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("clients GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie set on GET");

    let router = build_router(state.clone());
    let body = format!(
        "name=consent-on-create&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb&confidential=true\
         &consent_policy=first_time&save=1&_csrf={csrf}"
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
    assert_eq!(resp.status(), StatusCode::OK);

    let rows = sui_id_store::repos::clients::list(&state.db)
        .await
        .expect("list clients");
    let created = rows
        .iter()
        .find(|r| r.name == "consent-on-create")
        .expect("the just-created client");
    assert_eq!(
        created.consent_policy,
        sui_id_store::models::ConsentPolicy::FirstTime,
        "the submitted consent_policy must be applied, not defaulted away"
    );
}

/// Behaviour unchanged for every existing caller: an absent
/// `consent_policy` field still defaults to `None` (no consent screen),
/// the administrator-created-client default RFC 136 D1 established.
#[tokio::test]
async fn create_handler_without_the_field_keeps_todays_default() {
    let state = test_app();
    let session_cookie = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session_cookie).await;

    let rows = sui_id_store::repos::clients::list(&state.db)
        .await
        .expect("list clients");
    let created = rows
        .iter()
        .find(|r| r.id.to_string() == client_id)
        .expect("the just-created client");
    assert_eq!(
        created.consent_policy,
        sui_id_store::models::ConsentPolicy::None,
        "a create request with no consent_policy field keeps today's default"
    );
}
