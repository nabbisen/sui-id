//! RFC 144 N2e: a chosen radio option you can see, and a password match
//! you can see. §2's part — the static script, served and wired to each
//! of the three confirmation forms. §1 (the chosen-option CSS) is
//! covered at the unit level in `sui-id-web`'s `tokens::tests` and
//! `pages::repair_checks`; this file covers what only a real server can
//! answer: the script's own route, and each page's rendered markup.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

use super::common::*;

async fn get(state: &AppState, uri: &str, session: Option<&str>) -> axum::response::Response {
    let mut req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::ACCEPT_LANGUAGE, "en");
    if let Some(s) = session {
        req = req.header(header::COOKIE, format!("sui_id_session={s}"));
    }
    build_router(state.clone())
        .oneshot(req.body(Body::empty()).expect("req"))
        .await
        .expect("GET")
}

async fn body_of(resp: axum::response::Response) -> String {
    let bytes = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Every `<script` tag on a page with a confirmation form loads a
/// same-origin file — no inline script, the CSP's `script-src 'self'`
/// would block one anyway.
fn assert_no_inline_script(body: &str, page: &str) {
    for tag in body.split("<script").skip(1) {
        assert!(
            tag.trim_start().starts_with("src=\"/static/"),
            "{page}: inline script: {tag}"
        );
    }
}

#[tokio::test]
async fn password_match_js_is_served_as_javascript() {
    let state = test_app();
    let resp = get(&state, "/static/password-match.js", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    assert!(
        content_type.starts_with("application/javascript"),
        "content-type: {content_type}"
    );
    let body = body_of(resp).await;
    assert!(body.contains("data-confirms"));
    assert!(body.contains("aria-describedby"));
}

/// The scan the dispatch names: none of these calls may appear in the
/// script, because the comparison must never leave the page. Written
/// against the live `/static/password-match.js` response, not the
/// source file on disk, so a future build step that bundles or
/// transforms it is still caught.
#[tokio::test]
async fn password_match_js_contains_none_of_the_banned_calls() {
    let state = test_app();
    let body = body_of(get(&state, "/static/password-match.js", None).await).await;
    const BANNED: &[&str] = &[
        "fetch",
        "XMLHttpRequest",
        "sendBeacon",
        "localStorage",
        "sessionStorage",
        "console.",
        "innerHTML",
        ".value =",
        // Running new code, or reaching beyond the two fields (review
        // fold-in, 0.92.0 N2e): none of these has a use in comparing two
        // inputs.
        "eval(",
        "Function(",
        "import(",
        "document.cookie",
        "document.write",
        "postMessage",
        "WebSocket",
        "indexedDB",
        "outerHTML",
        "insertAdjacentHTML",
    ];
    let found: Vec<&str> = BANNED
        .iter()
        .copied()
        .filter(|needle| body.contains(needle))
        .collect();
    assert!(
        found.is_empty(),
        "password-match.js contains banned call(s): {found:?}"
    );
}

#[tokio::test]
async fn setup_admin_page_wires_its_confirmation_field() {
    let state = test_app();
    let body = body_of(get(&state, "/setup/admin", None).await).await;
    assert_no_inline_script(&body, "/setup/admin");
    assert!(body.contains(r#"<script src="/static/password-match.js"></script>"#));
    assert!(body.contains(r#"data-confirms="password""#));
    assert!(body.contains(r#"id="confirm-password-match-status""#));
    assert!(body.contains(r#"id="confirm-password-match-status" hidden"#));
    assert!(body.contains("The passwords match."));
    assert!(body.contains("The passwords do not match yet."));
}

#[tokio::test]
async fn change_password_page_wires_its_confirmation_field() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let body = body_of(get(&state, "/me/security/password", Some(&session)).await).await;
    assert_no_inline_script(&body, "/me/security/password");
    assert!(body.contains(r#"<script src="/static/password-match.js"></script>"#));
    assert!(body.contains(r#"data-confirms="new_password""#));
    assert!(body.contains(r#"id="confirm-password-match-status""#));
    assert!(body.contains("The passwords match."));
    assert!(body.contains("The passwords do not match yet."));
}

#[tokio::test]
async fn reset_password_page_wires_its_confirmation_field() {
    let state = test_app();
    let resp = get(&state, "/reset-password", None).await;
    let csp = resp
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = body_of(resp).await;
    assert_no_inline_script(&body, "/reset-password");
    // Both scripts load: reset-password.js (RFC 103 D10's fragment token)
    // and password-match.js (N2e §2) — different concerns, same page.
    assert!(body.contains(r#"<script src="/static/reset-password.js"></script>"#));
    assert!(body.contains(r#"<script src="/static/password-match.js"></script>"#));
    assert!(body.contains(r#"data-confirms="password""#));
    assert!(body.contains(r#"id="confirm-password-match-status""#));
    assert!(body.contains("The passwords match."));
    assert!(body.contains("The passwords do not match yet."));
    assert!(csp.contains("script-src 'self'"), "CSP: {csp}");
}
