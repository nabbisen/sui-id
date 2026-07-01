//! /admin/settings/* tabs (v0.20.3).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- /admin/settings/* (v0.20.3) ----------

#[tokio::test]
async fn settings_index_redirects_to_basic() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("settings");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/settings/basic");
}

#[tokio::test]
async fn settings_basic_renders_for_admin() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/basic")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("basic");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // Tab strip is rendered with all five tabs.
    assert!(body.contains("/admin/settings/basic"));
    assert!(body.contains("/admin/settings/security"));
    assert!(body.contains("/admin/settings/authentication"));
    assert!(body.contains("/admin/settings/logs"));
    assert!(body.contains("/admin/settings/other"));
    // Active tab marker.
    assert!(
        body.contains(r#"href="/admin/settings/basic" aria-current="page""#),
        "basic tab should be aria-current"
    );
    // Body content.
    assert!(body.contains("Issuer"));
    assert!(body.contains("Listen address"));
    assert!(body.contains("Discovery"));
    assert!(body.contains("JWKS"));
}

#[tokio::test]
async fn settings_security_renders_lockout_and_headers() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("security");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("最大ロックアウト時間"));
    assert!(body.contains("HSTS"));
    assert!(body.contains("Content-Security-Policy"));
    assert!(body.contains("X-Frame-Options"));
    assert!(body.contains("CORS"));
}

#[tokio::test]
async fn settings_authentication_renders_lifetimes() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/authentication")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authentication");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("PKCE"));
    assert!(body.contains("Argon2id"));
    assert!(body.contains("Access token"));
    assert!(body.contains("Refresh"));
}

#[tokio::test]
async fn settings_logs_renders_with_24h_counts_and_chain_status() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/logs")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("logs");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("auth.login.success"));
    assert!(body.contains("auth.login.failure"));
    assert!(body.contains("auth.password.changed_self"));
    // Chain check report. With a fresh test_app there should be at
    // least one row (the setup), so chain status is "正常".
    assert!(body.contains("ハッシュチェーン"));
    assert!(body.contains("/admin/audit"));
}

#[tokio::test]
async fn settings_other_renders_versions_and_paths() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/other")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("other");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("sui-id バージョン"));
    assert!(body.contains("対応スキーマバージョン"));
    assert!(body.contains("DB ファイル"));
    assert!(body.contains("マスターキーファイル"));
    assert!(body.contains("/admin/users"));
    assert!(body.contains("/admin/clients"));
}

#[tokio::test]
async fn settings_pages_require_admin() {
    let state = test_app();
    let router = build_router(state);
    for path in [
        "/admin/settings/basic",
        "/admin/settings/security",
        "/admin/settings/authentication",
        "/admin/settings/logs",
        "/admin/settings/other",
    ] {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("settings");
        // Anonymous request must NOT see settings.
        assert_ne!(resp.status(), StatusCode::OK, "{path} leaked to anon");
    }
}
