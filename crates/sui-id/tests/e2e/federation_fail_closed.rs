//! Federated sign-in fails closed (roadmap: federation-signin-fail-closed).
//!
//! The callback runs against a mock upstream IdP on 127.0.0.1: discovery,
//! then a token endpoint returning an unsigned ID token for a fixed `sub`.
//! A federation link ties that `sub` to a local user.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use base64ct::{Base64UrlUnpadded, Encoding};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{Role, UserRow, UserSource};
use tower::ServiceExt;

const SUB: &str = "fed-sub-1";
const SIGNIN_FAILED: &str = "/admin/login?fed_error=signin_failed";

/// Start a mock upstream IdP and return its issuer URL.
async fn mock_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let issuer = format!("http://{}", listener.local_addr().expect("addr"));
    let discovery = serde_json::json!({
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
    });
    let claims = serde_json::json!({ "sub": SUB });
    let id_token = format!(
        "{}.{}.sig",
        Base64UrlUnpadded::encode_string(br#"{"alg":"none"}"#),
        Base64UrlUnpadded::encode_string(claims.to_string().as_bytes())
    );
    let app = axum::Router::new()
        .route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || async move { axum::Json(discovery) }),
        )
        .route(
            "/token",
            axum::routing::post(move || async move {
                axum::Json(serde_json::json!({ "access_token": "at", "id_token": id_token }))
            }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock upstream");
    });
    issuer
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

/// A local user linked to the mock upstream's `SUB`.
async fn linked_user(state: &AppState, issuer: &str) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    sui_id_store::repos::users::create(
        &state.db,
        &UserRow {
            id: uid,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    let provider = uuid::Uuid::new_v4();
    exec(
        state,
        format!(
            "INSERT INTO federation_provider (id, slug, display_name, issuer, client_id, \
             enabled, created_at, updated_at) VALUES ('{provider}', 'up', 'Up', '{issuer}', \
             'client', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'); \
             INSERT INTO federation_link (user_id, provider_id, upstream_sub, linked_at, \
             last_seen_at) VALUES ('{uid}', '{provider}', '{SUB}', '2026-01-01T00:00:00Z', \
             '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
    uid
}

struct Outcome {
    status: StatusCode,
    location: String,
    session_cookie: Option<String>,
    pending_mfa_cookie: Option<String>,
}

/// Run `/auth/federated/up/start` and then the callback.
async fn federated_signin(state: &AppState) -> Outcome {
    let start = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");
    assert!(start.status().is_redirection(), "start: {}", start.status());
    let fed_state = extract_set_cookie(start.headers(), "sui_id_fed_state").expect("state cookie");
    let location = start
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("upstream redirect");
    let state_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("state="))
        .expect("state parameter");

    let cb = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={state_param}"
                ))
                .header(header::COOKIE, format!("sui_id_fed_state={fed_state}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    Outcome {
        status: cb.status(),
        location: cb
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned(),
        session_cookie: extract_set_cookie(cb.headers(), "sui_id_session")
            .filter(|v| !v.is_empty()),
        pending_mfa_cookie: extract_set_cookie(cb.headers(), "sui_id_pending_mfa")
            .filter(|v| !v.is_empty()),
    }
}

async fn sessions_of(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'"),
    )
    .await
}

fn assert_refused(o: &Outcome) {
    assert!(o.status.is_redirection(), "status {}", o.status);
    assert_eq!(o.location, SIGNIN_FAILED);
    assert!(o.session_cookie.is_none(), "no session cookie");
    assert!(o.pending_mfa_cookie.is_none(), "no pending-MFA cookie");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_active_linked_user_signs_in() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;

    let o = federated_signin(&state).await;
    assert_eq!(o.location, "/admin");
    assert!(o.session_cookie.is_some(), "the harness reaches a session");
    assert_eq!(sessions_of(&state, user).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_user_with_mfa_gets_the_mfa_step_not_a_session() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{user}', X'0102', X'00', 'test', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;

    let o = federated_signin(&state).await;
    assert_eq!(o.location, "/admin/login/mfa");
    assert!(o.session_cookie.is_none());
    assert!(o.pending_mfa_cookie.is_some());
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_mfa_read_error_gives_no_session() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;
    // Inject a storage failure into the MFA state read only.
    exec(
        &state,
        "ALTER TABLE user_totp RENAME TO user_totp_gone".into(),
    )
    .await;

    let o = federated_signin(&state).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_disabled_user_gets_no_session() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!("UPDATE users SET is_disabled = 1 WHERE id = '{user}'"),
    )
    .await;

    let o = federated_signin(&state).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_deleted_user_gets_no_session() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!("UPDATE users SET is_deleted = 1 WHERE id = '{user}'"),
    )
    .await;

    let o = federated_signin(&state).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

async fn get_with_session(state: &AppState, uri: &str, session: &str) -> StatusCode {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get")
        .status()
}

async fn refused_after(state: &AppState, session: &str, uri: &str, column: &str) {
    let id: SessionId = session.parse().expect("session id");
    let user = sui_id_store::repos::sessions::get(&state.db, id)
        .await
        .expect("session")
        .user_id;
    exec(
        state,
        format!("UPDATE users SET {column} = 1 WHERE id = '{user}'"),
    )
    .await;
    let status = get_with_session(state, uri, session).await;
    assert!(
        status.is_redirection() || status == StatusCode::UNAUTHORIZED,
        "{uri} after {column}: expected refusal, got {status}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_existing_session_of_a_user_disabled_afterwards_is_refused_on_me_security() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    linked_user(&state, &issuer).await;
    let session = federated_signin(&state)
        .await
        .session_cookie
        .expect("session");
    // Both extractors: `CurrentUser` (the MFA tab) and `SessionContext`
    // (the step-up page) accept the session while the user is active.
    assert_eq!(
        get_with_session(&state, "/me/security/mfa", &session).await,
        StatusCode::OK
    );
    assert_eq!(
        get_with_session(&state, "/me/security/step-up", &session).await,
        StatusCode::OK
    );
    refused_after(&state, &session, "/me/security/mfa", "is_disabled").await;
    assert!(
        !get_with_session(&state, "/me/security/step-up", &session)
            .await
            .is_success()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_existing_session_of_a_user_deleted_afterwards_is_refused_on_me_security() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    let issuer = mock_upstream().await;
    linked_user(&state, &issuer).await;
    let session = federated_signin(&state)
        .await
        .session_cookie
        .expect("session");
    refused_after(&state, &session, "/me/security/step-up", "is_deleted").await;
}
