use super::*;
use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::routing::get;
use tower::ServiceExt;

fn app(enable_hsts: bool) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn_with_state(
            SecurityHeaderConfig { enable_hsts },
            middleware,
        ))
}

#[tokio::test]
async fn baseline_headers_are_set() {
    let resp = app(false)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let h = resp.headers();
    assert!(h.contains_key(header::CONTENT_SECURITY_POLICY));
    assert_eq!(
        h.get(&X_FRAME_OPTIONS).map(|v| v.to_str().unwrap()),
        Some("DENY")
    );
    assert_eq!(
        h.get(&X_CONTENT_TYPE_OPTIONS).map(|v| v.to_str().unwrap()),
        Some("nosniff")
    );
    assert!(
        h.get(&REFERRER_POLICY)
            .map(|v| v.to_str().unwrap())
            .unwrap_or("")
            .contains("strict-origin")
    );
    assert!(h.contains_key(&PERMISSIONS_POLICY_HDR));
    // HSTS is *not* set when enable_hsts=false.
    assert!(!h.contains_key(header::STRICT_TRANSPORT_SECURITY));
}

#[tokio::test]
async fn hsts_set_only_when_enabled() {
    let resp = app(true)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let h = resp.headers();
    let v = h
        .get(header::STRICT_TRANSPORT_SECURITY)
        .map(|v| v.to_str().unwrap())
        .unwrap_or("");
    assert!(v.contains("max-age="));
    assert!(v.contains("includeSubDomains"));
}

#[tokio::test]
async fn handler_set_headers_are_not_overwritten() {
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                let mut r = axum::response::Response::new(Body::from("ok"));
                r.headers_mut().insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static("custom-policy"),
                );
                r
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            SecurityHeaderConfig { enable_hsts: false },
            middleware,
        ));
    let resp = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .map(|v| v.to_str().unwrap()),
        Some("custom-policy")
    );
}
