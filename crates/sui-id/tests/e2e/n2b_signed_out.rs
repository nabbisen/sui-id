//! RFC 144 (Refresh, §6): sign-out says so plainly, and the session
//! cookie it clears no longer opens an admin page.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

#[tokio::test]
async fn signing_out_says_so_and_the_old_cookie_stops_working() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/logout")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::from(format!("_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("logout POST");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert!(
        html.contains("You are signed out."),
        "the sign-in page must say plainly that the person is signed out: {html}"
    );

    // The old session cookie must no longer open an admin page.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET /admin after logout");
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "the session cookie from before sign-out must no longer open /admin"
    );
}
