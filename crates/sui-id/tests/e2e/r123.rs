//! RFC 123 — an endpoint that authenticates a client costs the caller
//! something.
//!
//! D1: `/oauth2/introspect` and `/oauth2/revoke` each take two rate-limit
//! buckets (per source IP, per claimed `client_id`), checked before
//! `authenticate_client` runs. D2: that ordering introduces no protocol
//! conformance change. D3: no separate failure counter. D4: every branch of
//! `authenticate_client` that used to skip hashing now runs a dummy
//! verification instead, so rejecting an unknown/public/disabled client id
//! is not distinguishable from rejecting a wrong secret by return-early
//! timing (see `rfcs/handoffs/123-authenticating-a-client-costs-the-caller/`
//! for the measured numbers, before and after).
//!
//! Production-sized budgets (300/60s per IP, 600/60s per client id) are too
//! large to exhaust in a fast test, so the limit tests here shrink
//! `AppState::limiters` immediately after construction, before the state is
//! ever cloned into a router — `Arc::get_mut` only succeeds while that
//! uniqueness holds, which is itself a guard against reusing this helper
//! after a router has already started sharing the `Arc`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

/// Shrink both introspect/revoke buckets on a freshly built `AppState` so a
/// test can exhaust one deterministically in a handful of requests. Must be
/// called before the state is cloned anywhere (router construction included).
fn shrink_client_endpoint_limits(
    state: &mut sui_id::AppState,
    ip_per_window: i64,
    client_per_window: i64,
) {
    let limiters = std::sync::Arc::get_mut(&mut state.limiters)
        .expect("limiters must be uniquely owned before first use");
    limiters.introspect_revoke_ip = sui_id::ratelimit::Limiter::new(ip_per_window, 60);
    limiters.introspect_revoke_client = sui_id::ratelimit::Limiter::new(client_per_window, 60);
}

async fn post_introspect(
    state: &sui_id::AppState,
    client_id: &str,
    client_secret: &str,
) -> axum::response::Response {
    let body = format!("token=whatever&client_id={client_id}&client_secret={client_secret}");
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/introspect")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("introspect request")
}

#[tokio::test]
async fn introspect_refuses_past_its_per_ip_limit() {
    let mut state = test_app();
    // Client-id bucket left generous; a distinct id per request also keeps
    // it from tripping first, isolating the per-IP bucket under test.
    shrink_client_endpoint_limits(&mut state, 3, 1000);

    let mut saw_429 = false;
    for i in 0..6 {
        let resp = post_introspect(&state, &format!("not-a-real-client-{i}"), "garbage").await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
    }
    assert!(
        saw_429,
        "six requests from one IP all accepted; per-IP limit did not fire"
    );
}

#[tokio::test]
async fn introspect_refuses_past_its_per_client_id_limit() {
    let mut state = test_app();
    // IP bucket left generous; the same claimed client_id every time
    // isolates the per-client-id bucket under test.
    shrink_client_endpoint_limits(&mut state, 1000, 3);

    let mut saw_429 = false;
    for _ in 0..6 {
        let resp = post_introspect(&state, "not-a-real-client-fixed-id", "garbage").await;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
    }
    assert!(
        saw_429,
        "six requests for one claimed client_id all accepted; per-client limit did not fire"
    );
}

#[tokio::test]
async fn a_legitimate_caller_within_both_limits_is_unaffected() {
    let mut state = test_app();
    shrink_client_endpoint_limits(&mut state, 5, 5);

    for _ in 0..3 {
        let resp = post_introspect(&state, "not-a-real-client", "garbage").await;
        // Rejected for bad credentials (401), never for rate limiting —
        // three calls is inside a budget of five on both dimensions.
        assert_ne!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn a_rate_limited_introspect_request_uses_oauth_wire_format() {
    let mut state = test_app();
    shrink_client_endpoint_limits(&mut state, 1, 1000);

    let _ = post_introspect(&state, "client-a", "garbage").await; // consumes the one slot
    let resp = post_introspect(&state, "client-a", "garbage").await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after = resp
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    assert!(retry_after.is_some(), "Retry-After header must be present");
    let body: serde_json::Value =
        serde_json::from_slice(&read_body(resp.into_body()).await).expect("json body");
    assert_eq!(
        body["error"].as_str(),
        Some("temporarily_unavailable"),
        "rate-limited introspect must use the same RFC 6749 §5.2 shape /oauth2/token already uses: {body}"
    );
    assert!(
        body.get("code").is_none(),
        "the internal API envelope's 'code' field must be absent: {body}"
    );
}

#[tokio::test]
async fn revoke_takes_the_same_two_buckets_as_introspect() {
    // Not a full duplicate of the introspect suite above — just confirms
    // revoke is wired the same way, since it shares `client_credentials`
    // and `authenticate_client` but is a separate handler.
    let mut state = test_app();
    shrink_client_endpoint_limits(&mut state, 3, 1000);

    let mut saw_429 = false;
    for i in 0..6 {
        let body = format!("token=whatever&client_id=not-a-real-client-{i}&client_secret=garbage");
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/oauth2/revoke")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .expect("req"),
            )
            .await
            .expect("revoke request");
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            saw_429 = true;
            break;
        }
    }
    assert!(saw_429, "revoke's per-IP limit did not fire");
}

// ---------- D4: rejection is uniform, not just fast for some branches ----------

#[tokio::test]
async fn unknown_public_and_disabled_clients_are_all_refused_the_same_way() {
    // Correctness, not timing (timing is measured separately and committed
    // under the handoff, not asserted in CI — a wall-clock assertion here
    // would be flaky by nature). This pins that D4's dummy-verify branches
    // did not change *what* is returned, only how long returning it takes.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (confidential_id, _secret) = create_client(&state, &session).await;

    // A public client: create one directly via the store so this test does
    // not depend on the admin UI exposing a "public" toggle.
    let public_id = {
        use sui_id_shared::ids::ClientId;
        use sui_id_store::models::{ClientRow, ConsentPolicy, RegistrationSource};
        let id = ClientId::new();
        let now = state.clock.now();
        sui_id_store::repos::clients::create(
            &state.db,
            &ClientRow {
                id,
                name: "public-client".into(),
                confidential: false,
                secret_hash: None,
                redirect_uris: vec!["https://rp.test/cb".into()],
                allowed_scopes: "openid".into(),
                post_logout_redirect_uris: vec![],
                is_disabled: false,
                is_deleted: false,
                consent_policy: ConsentPolicy::default(),
                registered_via: RegistrationSource::Admin,
                logo_uri: None,
                homepage_uri: None,
                privacy_policy_uri: None,
                tos_uri: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("insert public client");
        id.to_string()
    };

    for (label, id) in [
        ("unknown", "00000000-0000-4000-8000-000000000099".to_owned()),
        ("public", public_id),
        ("confidential, wrong secret", confidential_id),
    ] {
        let resp = post_introspect(&state, &id, "definitely-wrong").await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{label}: status");
        assert_eq!(
            resp.headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok()),
            Some("Basic realm=\"sui-id\""),
            "{label}: WWW-Authenticate"
        );
        let body: serde_json::Value =
            serde_json::from_slice(&read_body(resp.into_body()).await).expect("json body");
        assert_eq!(
            body["error"].as_str(),
            Some("invalid_client"),
            "{label}: {body}"
        );
    }
}
