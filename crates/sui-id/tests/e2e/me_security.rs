//! /me/security self-service security page (v0.18.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- /me/security (v0.18.0) ----------

#[tokio::test]
async fn me_security_page_renders_for_authenticated_user() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("me/security GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The page must mention what we expect to be there: the
    // section headings, the username, and the "current session"
    // marker for the row that matches the cookie.
    assert!(body.contains("アカウントセキュリティ"), "missing heading");
    assert!(
        body.contains("サインイン中の場所"),
        "missing sessions section"
    );
    assert!(
        body.contains("最近のアクティビティ"),
        "missing audit section"
    );
    assert!(
        body.contains("current session"),
        "current session not marked"
    );
}

#[tokio::test]
async fn me_security_redirects_when_not_signed_in() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("me/security GET");
    // The CurrentUser extractor maps a missing cookie to
    // Unauthenticated, which the HTML error path renders as a
    // login redirect or an HTML error — either way, *not* OK.
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "unauthenticated request must not see /me/security"
    );
}

#[tokio::test]
async fn me_security_revoke_one_signs_target_session_out() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);

    // Sanity: GET /me/security with s1 should list both rows and
    // have a Revoke button for s2.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={s1}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("page");
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("Revoke"));

    // Re-fetch the page to get a CSRF token (the cookie is set on
    // the response, so we extract it from there).
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf cookie");

    // Revoke s2.
    let body = format!("_csrf={csrf}");
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/me/security/sessions/{s2}/revoke"))
        .header(
            header::COOKIE,
            format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke");
    assert!(
        resp.status().is_redirection(),
        "expected redirect got {}",
        resp.status()
    );

    // s2 must no longer authenticate.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/me/security")
        .header(header::COOKIE, format!("sui_id_session={s2}"))
        .body(Body::empty())
        .expect("req");
    let resp = build_router(state)
        .oneshot(req)
        .await
        .expect("post-revoke s2");
    assert_ne!(resp.status(), StatusCode::OK, "s2 must be dead now");
}

#[tokio::test]
async fn me_security_revoke_all_others_keeps_current_session() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    let s3 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);
    assert_ne!(s2, s3);

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf cookie");

    let body = format!("_csrf={csrf}&current_session={s1}");
    let req = Request::builder()
        .method(Method::POST)
        .uri("/me/security/sessions/revoke-all-others")
        .header(
            header::COOKIE,
            format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke-all-others");
    assert!(resp.status().is_redirection());

    // s1 must still work; s2 and s3 must not.
    for (sid, alive) in [(&s1, true), (&s2, false), (&s3, false)] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/me/security")
                    .header(header::COOKIE, format!("sui_id_session={sid}"))
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("post-revoke probe");
        if alive {
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "s1 (current) must remain alive"
            );
        } else {
            assert_ne!(resp.status(), StatusCode::OK, "{sid} should be revoked");
        }
    }
}

#[tokio::test]
async fn me_security_cannot_revoke_someone_elses_session() {
    let state = test_app();
    // First user (the bootstrap admin = "alice").
    let s_admin = complete_setup_and_login(&state).await;

    // Create a second user "bob" via the admin UI.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/users")
                .header(header::COOKIE, format!("sui_id_session={s_admin}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("users page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");
    let body =
        format!("_csrf={csrf}&username=bob&display_name=Bob&password=bob-the-tester-password");
    let _ = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/users")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s_admin}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("create bob");
    let s_bob = login_again_for_admin(&state, "bob", "bob-the-tester-password").await;

    // The admin tries to revoke bob's session through /me/security.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s_admin}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf2 = extract_csrf_cookie(resp.headers()).expect("csrf");
    let body = format!("_csrf={csrf2}");
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/me/security/sessions/{s_bob}/revoke"))
        .header(
            header::COOKIE,
            format!("sui_id_session={s_admin}; sui_id_csrf={csrf2}"),
        )
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("revoke attempt");
    // The handler redirects regardless (no leak), but bob's
    // session must still be good.
    assert!(resp.status().is_redirection());

    // Probe bob's session — must still be alive.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s_bob}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("bob probe");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "bob's session must NOT have been revoked by the admin's attempt"
    );
}
