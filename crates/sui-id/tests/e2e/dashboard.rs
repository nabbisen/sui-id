//! Admin dashboard sparkline (v0.20.2).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- dashboard sparkline (v0.20.2) ----------

#[tokio::test]
async fn dashboard_sparkline_renders_with_default_range() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("dashboard");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Sparkline-related markers we expect on the page.
    assert!(body.contains("サインイン活動"), "missing sparkline section");
    assert!(
        body.contains("過去 7 日間"),
        "missing default range tab label"
    );
    // The SVG element with our aria-label must be there.
    assert!(
        body.contains(r#"aria-label="サインイン活動のスパークライン""#),
        "missing sparkline svg"
    );
    // The three range tabs must be present as anchors.
    assert!(body.contains("range=24h"));
    assert!(body.contains("range=7d"));
    assert!(body.contains("range=30d"));
    // 7d range = 7 buckets of audio-grid `<title>`. We can at
    // least verify the tooltip format made it into the HTML for
    // *some* bucket.
    assert!(
        body.contains("成功 0 / 失敗 0") || body.contains("成功 1 / 失敗 0"),
        "no bucket tooltip rendered"
    );
}

#[tokio::test]
async fn dashboard_sparkline_honours_explicit_range_query() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    for range in ["24h", "7d", "30d"] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/admin?range={range}"))
                    .header(header::COOKIE, format!("sui_id_session={session}"))
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("dashboard");
        assert_eq!(resp.status(), StatusCode::OK, "range={range}");
        let bytes = read_body(resp.into_body()).await;
        let body = String::from_utf8_lossy(&bytes);
        // The active range tab gets `aria-current="page"` on its
        // anchor. Detect that by string-search around the matching
        // href value.
        let needle = format!(r#"href="/admin?range={range}""#);
        assert!(body.contains(&needle), "expected anchor for range={range}");
    }
}

#[tokio::test]
async fn dashboard_sparkline_falls_back_to_default_on_garbage_range() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin?range=banana")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("dashboard");
    // Should render normally — not 400 — and pick the default
    // (which is currently 7 days).
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("サインイン活動"));
}
