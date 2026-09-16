//! RFC 103 stage 2a — follow-ups to stage 1: a refused completion is
//! logged at info when the link is merely unusable and at error when
//! storage failed, and no `/reset-password` response may be cached.

use super::common::*;
use super::r103_stage1::{
    Captured, Resp, complete, csrf_from, exec, get, is_invalid_link_page, issue_token, reset_app,
    send,
};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};

fn capture_logs() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn text(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

#[tokio::test]
async fn r103_s2_storage_failure_on_completion_is_logged_at_error() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;
    // U10's audit append fails, so its transaction rolls back with a
    // storage error rather than the ordinary invalid-link outcome.
    exec(
        &state,
        "CREATE TRIGGER r103_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r103 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;

    let (captured, _guard) = capture_logs();
    tracing::callsite::rebuild_interest_cache();
    let r = complete(&state, &token).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&r), "the uniform invalid-link page");

    let logged = text(&captured);
    let line = logged
        .lines()
        .find(|l| l.contains("password reset completion refused"))
        .unwrap_or_else(|| panic!("no log line for the storage failure:\n{logged}"));
    assert!(line.contains(" ERROR "), "wrong level: {line}");
    assert!(line.contains("request_id="), "no request id: {line}");
    assert!(
        line.contains("r103 test: audit_log insert rejected"),
        "cause missing: {line}"
    );
    assert!(
        !logged.contains("unknown, used or expired"),
        "a storage failure is not reported as an unusable link"
    );
    assert!(!logged.contains(&token), "the token leaked into the log");
}

#[tokio::test]
async fn r103_s2_unknown_token_is_logged_at_info() {
    let (state, _mailer, _admin) = reset_app().await;
    let (captured, _guard) = capture_logs();
    tracing::callsite::rebuild_interest_cache();
    let r = complete(&state, "no-such-token-anywhere").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&r));

    let logged = text(&captured);
    let line = logged
        .lines()
        .find(|l| l.contains("password reset completion refused"))
        .unwrap_or_else(|| panic!("no log line for the unknown token:\n{logged}"));
    assert!(line.contains(" INFO "), "wrong level: {line}");
    assert!(line.contains("request_id="), "no request id: {line}");
    assert!(!logged.contains(" ERROR "), "no error line:\n{logged}");
    assert!(
        !logged.contains("no-such-token-anywhere"),
        "the token leaked"
    );
}

fn assert_no_store(label: &str, r: &Resp) {
    let value = r
        .headers
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok());
    assert_eq!(value, Some("no-store"), "{label}: Cache-Control");
}

async fn post_reset(state: &sui_id::AppState, token: &str, password: &str, confirm: &str) -> Resp {
    let csrf = csrf_from(state, "/reset-password").await;
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/reset-password")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
            .body(Body::from(format!(
                "_csrf={csrf}&token={}&password={}&confirm_password={}",
                urlencode(token),
                urlencode(password),
                urlencode(confirm)
            )))
            .expect("req"),
    )
    .await
}

#[tokio::test]
async fn r103_s2_every_reset_password_response_is_no_store() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;

    assert_no_store("GET form", &get(&state, "/reset-password").await);
    assert_no_store(
        "GET ?token=",
        &get(&state, "/reset-password?token=anything").await,
    );

    // The re-shown forms hold the token in a field value.
    let mismatch = post_reset(&state, &token, "brand-new-secure-pw-1", "something-else-2").await;
    assert_eq!(mismatch.status, StatusCode::BAD_REQUEST);
    assert!(mismatch.body.contains(&token), "the form holds the token");
    assert_no_store("POST password mismatch", &mismatch);
    let policy = post_reset(&state, &token, "short", "short").await;
    assert_eq!(policy.status, StatusCode::BAD_REQUEST);
    assert!(policy.body.contains(&token), "the form holds the token");
    assert_no_store("POST password policy", &policy);

    let invalid = post_reset(
        &state,
        "no-such-token",
        "brand-new-secure-pw-1",
        "brand-new-secure-pw-1",
    )
    .await;
    assert!(is_invalid_link_page(&invalid));
    assert_no_store("POST invalid link", &invalid);

    let ok = post_reset(
        &state,
        &token,
        "brand-new-secure-pw-1",
        "brand-new-secure-pw-1",
    )
    .await;
    assert!(ok.status.is_redirection(), "reset succeeds: {}", ok.status);
    assert_no_store("POST success", &ok);
}
