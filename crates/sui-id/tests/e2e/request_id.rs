//! Request-id middleware (v0.12.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- request-id middleware (v0.12.0) ----------

#[tokio::test]
async fn response_carries_a_generated_x_request_id_when_caller_omits_one() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    // UUIDv4 is 36 characters with dashes.
    assert_eq!(id.len(), 36, "got: {id}");
    assert!(id.matches('-').count() == 4);
}

#[tokio::test]
async fn caller_supplied_x_request_id_is_echoed_back() {
    let state = test_app();
    let router = build_router(state);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", "client-trace-abc123")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    assert_eq!(
        resp.headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok()),
        Some("client-trace-abc123")
    );
}

#[tokio::test]
async fn caller_supplied_x_request_id_thats_too_long_is_replaced() {
    let state = test_app();
    let router = build_router(state);
    // 100 bytes — well over our MAX_LEN of 64. Middleware should
    // discard and generate a UUID instead. (We use only safe chars
    // here to isolate the length check from the alphabet check;
    // some unsafe bytes are pre-rejected by the http crate before
    // they reach our code, which is the right kind of defence in
    // depth but not what this test is exercising.)
    let long_id = "a".repeat(100);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", long_id.clone())
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    assert_ne!(id, long_id, "long id should have been replaced");
    assert_eq!(id.len(), 36, "should be a UUID; got: {id}");
}

#[tokio::test]
async fn caller_supplied_x_request_id_with_unsafe_chars_is_replaced() {
    let state = test_app();
    let router = build_router(state);
    // Space is in the http-permitted range but in our reject set.
    let req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .header("X-Request-Id", "has spaces in it")
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("healthz");
    let id = resp
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id missing");
    assert!(!id.contains(' '));
    assert_eq!(id.len(), 36);
}
