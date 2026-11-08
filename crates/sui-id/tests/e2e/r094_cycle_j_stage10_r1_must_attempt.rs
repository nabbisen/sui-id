//! RFC 094 M2b criterion 2 R1, cycle J stage 10: every best-effort audit
//! append that remains must report its own failure, rather than discard
//! it silently.
//!
//! `token.revoke` (T05) -- converted to `emit_must_attempt` this stage --
//! driven end to end the same way `r096_b1_stage6b.rs` proved A6: break
//! the audit log with a `BEFORE INSERT` trigger, run the real request,
//! and assert both halves of R1's own requirement -- the revocation
//! itself is unchanged, and the failure is logged by name.
//! `auth.login.failure`'s own conversion (one of the two sites that
//! collide with a sealed `EventDescriptor` of the same name, so it keeps
//! `audit::append` wrapped in its own `tracing::error!` instead) is
//! tested alongside `r11_login_failure.rs`'s existing audit-outage
//! fixtures, not duplicated here.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};

use super::common::*;
use tower::ServiceExt;
use url::Url;

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn break_audit_log(state: &AppState) {
    exec(
        state,
        "CREATE TRIGGER r094_j10_r1_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'r094 cycle J stage 10 R1 test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;
}

// ── Log capture -- same shape as `r096_b1_stage6b.rs`'s own `capture()`,
// thread-local so the handler runs on the capturing subscriber's thread. ──

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn capture() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn logged(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

/// Full setup -> authorize -> token, returning (client_id, client_secret,
/// access_token). Mirrors `introspection.rs`'s own `obtain_tokens`.
async fn obtain_access_token(state: &AppState) -> (String, String, String) {
    let session = complete_setup_and_login(state).await;
    let (client_id, client_secret) = create_client(state, &session).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=x&nonce=n&code_challenge={challenge}&code_challenge_method=S256"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri(&auth_url)
        .header(header::COOKIE, format!("sui_id_session={session}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("authorize");
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&loc)
        .expect("url")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("token");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    let access = json["access_token"].as_str().unwrap().to_owned();
    (client_id, client_secret, access)
}

async fn post_revoke(
    state: &AppState,
    client_id: &str,
    client_secret: &str,
    token: &str,
) -> StatusCode {
    let body = format!("token={token}&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    router.oneshot(req).await.expect("revoke").status()
}

async fn post_introspect(
    state: &AppState,
    client_id: &str,
    client_secret: &str,
    token: &str,
) -> bool {
    let body = format!("token={token}&client_id={client_id}&client_secret={client_secret}");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/introspect")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("req");
    let resp = router.oneshot(req).await.expect("introspect");
    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = read_body(resp.into_body()).await;
    let v: serde_json::Value = serde_json::from_slice(&body_bytes).expect("json");
    v["active"].as_bool().expect("active field")
}

/// T05 / R1: `token.revoke`'s own audit append now goes through
/// `emit_must_attempt`. With the audit log broken, the revocation must
/// still take effect -- RFC 094 invariant 5 forbids making a revocation
/// wait on its own best-effort record -- and the failure must be logged
/// by name.
#[tokio::test]
async fn revoke_still_revokes_and_logs_when_the_audit_append_fails() {
    let state = test_app();
    let (client_id, client_secret, access) = obtain_access_token(&state).await;
    break_audit_log(&state).await;
    let (captured, _guard) = capture();

    let status = post_revoke(&state, &client_id, &client_secret, &access).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "RFC 7009 section 2.2: revoke returns 200 even when its own audit record fails"
    );

    let still_active = post_introspect(&state, &client_id, &client_secret, &access).await;
    assert!(
        !still_active,
        "the revocation itself must take effect even though its audit row did not"
    );

    let log = logged(&captured);
    assert!(
        log.contains("must-attempt emission: audit append failed"),
        "no telemetry line in captured log:\n{log}"
    );
    assert!(
        log.contains("token.revoke"),
        "telemetry line present but doesn't name the event:\n{log}"
    );
}

/// The ordinary path, for contrast: nothing broken, the revocation still
/// happens, and the event genuinely lands in the audit log.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoke_records_the_event_when_nothing_is_broken() {
    let state = test_app();
    let (client_id, client_secret, access) = obtain_access_token(&state).await;

    let status = post_revoke(&state, &client_id, &client_secret, &access).await;
    assert_eq!(status, StatusCode::OK);

    let rows: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'token.revoke'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count");
    assert_eq!(
        rows, 1,
        "exactly one revoke happened, so exactly one row records it"
    );
}
