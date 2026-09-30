//! RFC 122 — a one-time secret does not travel where it persists.
//!
//! One test per surface: the response that carries the secret is never a
//! redirect (so the secret is never a URL component, D1), and carries
//! `Cache-Control: no-store` (D3). A separate test pins D2 directly: the
//! client-edit page must not render a secret a caller merely claims to
//! have, even after the query parameter that once carried it is gone.
//!
//! `/admin/users/{id}/recovery-link` (surface F) is out of scope here —
//! it predates this RFC and already has its own no-store test
//! (`r103_stage4`).

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

fn assert_no_store(label: &str, headers: &axum::http::HeaderMap) {
    let cc = headers
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok());
    assert_eq!(cc, Some("no-store"), "{label}: Cache-Control");
}

// ---------- surface A: client secret at creation ----------

#[tokio::test]
async fn client_creation_response_is_no_store() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;
    let body = "name=test-rp&redirect_uris=https%3A%2F%2Frp.test%2Fcb&confidential=true&_csrf="
        .to_owned()
        + &csrf;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/clients")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("create client");
    assert_eq!(resp.status(), StatusCode::OK, "no redirect: never a URL");
    assert_no_store("client creation", resp.headers());
}

// ---------- surface B: rotated client secret ----------

#[tokio::test]
async fn rotated_secret_is_rendered_directly_never_a_url_and_response_is_no_store() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _initial_secret) = create_client(&state, &session).await;

    let csrf = fetch_csrf(&state, &session).await;
    let body = format!("_csrf={csrf}&_confirmed=1&reason=rotation+test");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/clients/{client_id}/rotate-secret"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rotate secret");
    // D1: rendered directly, not a redirect — so the new secret is never a
    // URL component (no Location header to carry it in).
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "rotation must render the edit page directly, not redirect"
    );
    assert!(
        resp.headers().get(header::LOCATION).is_none(),
        "no Location header: the secret must never travel in one"
    );
    assert_no_store("secret rotation", resp.headers());
    let html = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        html.contains("class=\"code ml-2\""),
        "the new secret should be rendered inline: {html}"
    );
}

// ---------- D2: the edit page trusts nothing a caller hands it ----------

#[tokio::test]
async fn edit_page_ignores_a_caller_supplied_rotated_secret_query_parameter() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/admin/clients/{client_id}/edit?rotated_secret=FORGED-BY-ATTACKER"
                ))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("edit GET with forged query param");
    assert_eq!(resp.status(), StatusCode::OK);
    let html = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        !html.contains("FORGED-BY-ATTACKER"),
        "the edit page must never render a secret it was merely handed: {html}"
    );
}

// ---------- surface C: TOTP secret + QR ----------

#[tokio::test]
async fn totp_enrollment_start_response_is_no_store() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!("_csrf={csrf}&current_password={}", urlencode(PASSWORD));
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/enroll/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("enroll start");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_no_store("TOTP enrollment start", resp.headers());
}

// ---------- surfaces D & E: recovery codes ----------

#[tokio::test]
async fn recovery_codes_at_enrollment_response_is_no_store() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // enroll_mfa_for drives both /enroll/start and /enroll/confirm; the
    // response headers it returns to the caller aren't exposed, so this
    // test drives /enroll/confirm directly to inspect them, following the
    // same start-then-confirm sequence.
    let csrf = fetch_csrf(&state, &session).await;
    let start_body = format!("_csrf={csrf}&current_password={}", urlencode(PASSWORD));
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/enroll/start")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(start_body))
                .expect("req"),
        )
        .await
        .expect("enroll start");
    let html = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    let secret_b32 = {
        let label_at = html
            .find("秘密鍵:")
            .or_else(|| html.find("Secret:"))
            .expect("secret label rendered");
        let rest = &html[label_at..];
        let span_at = rest.find("<span class=\"code ml-1\"").expect("secret span");
        let after_open = &rest[span_at..];
        let gt = after_open.find('>').expect("span open close");
        let inner = &after_open[gt + 1..];
        let end = inner.find("</span>").expect("secret end");
        inner[..end].to_owned()
    };
    let secret = decode_b32(&secret_b32);
    let now = chrono::Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(&secret, step).await;

    let csrf = fetch_csrf(&state, &session).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/enroll/confirm")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("enroll confirm");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_no_store("recovery codes at enrollment", resp.headers());
}

#[tokio::test]
async fn recovery_codes_regenerated_response_is_no_store() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (secret_b32, _codes) = enroll_mfa_for(&state, &session).await;
    let secret = decode_b32(&secret_b32);

    // Regenerating recovery codes requires a fresh step-up once a factor
    // exists (RFC 102 B7) — freshen it via /me/security/step-up first.
    let csrf = fetch_csrf(&state, &session).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/step-up?return_to=/me/security/mfa")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("step-up GET");
    let step_up_csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").unwrap_or(csrf);
    let now = chrono::Utc::now().timestamp();
    let step = now / 30 + 1;
    let code = sui_id_core::totp::code_for_step(&secret, step).await;
    let body = format!("_csrf={step_up_csrf}&code={code:06}&return_to=%2Fme%2Fsecurity%2Fmfa");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={step_up_csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("step-up POST");
    assert!(
        resp.status().is_redirection(),
        "expected fresh step-up to succeed, got {}",
        resp.status()
    );

    let csrf = fetch_csrf(&state, &session).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/mfa/recovery-codes/regenerate")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(format!("_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("regenerate");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_no_store("recovery codes regenerated", resp.headers());
}

// ---------- surface G: dynamic client registration ----------

#[tokio::test]
async fn dynamic_registration_response_is_no_store_and_never_a_url() {
    use sha2::{Digest, Sha256};
    use sui_id_shared::ids::RegistrationTokenId;
    use sui_id_store::repos::client_registration_token::{self, RegistrationTokenRow};

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    let raw_token = "test-registration-token-r122";
    let hash: String = Sha256::digest(raw_token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let now = state.clock.now();
    client_registration_token::create(
        &state.db,
        &RegistrationTokenRow {
            id: RegistrationTokenId::new(),
            token_hash: hash,
            max_uses: 1,
            used_count: 0,
            expires_at: None,
            revoked_at: None,
            note: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .expect("store registration token");

    let payload = r#"{"redirect_uris":["https://rp.test/cb"],"client_name":"dyn-rp"}"#;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {raw_token}"))
                .body(Body::from(payload))
                .expect("req"),
        )
        .await
        .expect("dynamic register");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "a JSON POST response, never a redirect: never a URL"
    );
    assert!(resp.headers().get(header::LOCATION).is_none());
    assert_no_store("dynamic client registration", resp.headers());
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        body.contains("\"client_secret\""),
        "expected a generated client_secret in the response: {body}"
    );
}
