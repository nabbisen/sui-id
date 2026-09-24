//! RFC 103 stage 1 — a password reset never gives a directory or federated
//! account a local password (D13), completion consumes the token exactly
//! once and re-reads the user in the same transaction, and the token never
//! travels in a URL the server sees (D10).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{PasswordResetTokenId, UserId};
use tower::ServiceExt;

pub(super) const NEW_PASSWORD: &str = "brand-new-secure-pw-12345";

pub(super) struct Resp {
    pub(super) status: StatusCode,
    pub(super) location: Option<String>,
    pub(super) headers: axum::http::HeaderMap,
    pub(super) body: String,
}

pub(super) async fn send(state: &AppState, req: Request<Body>) -> Resp {
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("send");
    let status = resp.status();
    let headers = resp.headers().clone();
    let location = headers
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    Resp {
        status,
        location,
        headers,
        body,
    }
}

pub(super) async fn get(state: &AppState, uri: &str) -> Resp {
    send(
        state,
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("req"),
    )
    .await
}

pub(super) async fn csrf_from(state: &AppState, uri: &str) -> String {
    let r = get(state, uri).await;
    extract_set_cookie(&r.headers, "sui_id_csrf").expect("csrf cookie")
}

async fn request_reset(state: &AppState, email: &str) -> Resp {
    let csrf = csrf_from(state, "/forgot-password").await;
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/forgot-password")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
            .body(Body::from(format!(
                "_csrf={csrf}&email={}",
                urlencode(email)
            )))
            .expect("req"),
    )
    .await
}

pub(super) fn complete_request(csrf: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/reset-password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
        .body(Body::from(format!(
            "_csrf={csrf}&token={}&password={NEW_PASSWORD}&confirm_password={NEW_PASSWORD}",
            urlencode(token)
        )))
        .expect("req")
}

pub(super) async fn complete(state: &AppState, token: &str) -> Resp {
    let csrf = csrf_from(state, "/reset-password").await;
    send(state, complete_request(&csrf, token)).await
}

fn token_from_mail(mail: &sui_id_core::mail::OutgoingMail) -> String {
    let prefix = "/reset-password#t=";
    let start = mail.text_body.find(prefix).expect("link in mail") + prefix.len();
    let end = mail.text_body[start..]
        .find(char::is_whitespace)
        .map(|i| start + i)
        .unwrap_or(mail.text_body.len());
    mail.text_body[start..end].to_owned()
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

pub(super) async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

pub(super) async fn events(state: &AppState, action: &str) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM audit_log WHERE action = '{action}'"),
    )
    .await
}

async fn password_hash(state: &AppState, user: UserId) -> Option<String> {
    let sql = format!("SELECT password_hash FROM credentials WHERE user_id = '{user}'");
    state
        .db
        .with_conn(move |c| {
            use rusqlite::OptionalExtension;
            Ok(c.query_row(&sql, [], |r| r.get(0)).optional()?)
        })
        .await
        .expect("credential read")
}

async fn token_consumed(state: &AppState, id: PasswordResetTokenId) -> bool {
    scalar(
        state,
        format!("SELECT consumed_at IS NOT NULL FROM password_reset_tokens WHERE id = '{id}'"),
    )
    .await
        == 1
}

/// Insert a live token for `user` directly, bypassing `request_reset`.
async fn mint_token(state: &AppState, user: UserId, plaintext: &str) -> PasswordResetTokenId {
    let now = chrono::Utc::now();
    let row = sui_id_store::models::PasswordResetTokenRow {
        id: PasswordResetTokenId::new(),
        user_id: user,
        token_hash: Sha256::digest(plaintext.as_bytes()).to_vec(),
        issued_at: now,
        expires_at: now + chrono::Duration::minutes(30),
        consumed_at: None,
        requester_ip: None,
        issued_via: sui_id_store::models::ResetTokenOrigin::Email,
        issued_by: None,
        revoked_at: None,
    };
    sui_id_store::repos::password_reset_tokens::insert(&state.db, &row)
        .await
        .expect("insert token");
    row.id
}

/// An app with SMTP enabled and the setup admin carrying an address.
pub(super) async fn reset_app() -> (AppState, Arc<sui_id_core::mail::InMemoryMailSender>, UserId) {
    let (state, mailer) = test_app_with_mailer();
    complete_setup_and_login(&state).await;
    enable_smtp(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin");
    sui_id_store::repos::users::update_email(
        &state.db,
        admin.id,
        Some("alice@test.invalid"),
        chrono::Utc::now(),
    )
    .await
    .expect("email");
    (state, mailer, admin.id)
}

pub(super) async fn issue_token(
    state: &AppState,
    mailer: &sui_id_core::mail::InMemoryMailSender,
) -> (String, PasswordResetTokenId) {
    assert_eq!(
        request_reset(state, "alice@test.invalid").await.status,
        StatusCode::OK
    );
    let token = token_from_mail(&mailer.last().await.expect("reset mail"));
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let row = sui_id_store::repos::password_reset_tokens::find_by_hash(&state.db, &hash)
        .await
        .expect("lookup")
        .expect("token row");
    (token, row.id)
}

pub(super) fn is_invalid_link_page(r: &Resp) -> bool {
    r.body.contains(r#"href="/forgot-password""#) && !r.body.contains(r#"name="password""#)
}

// ── D13: directory accounts ──────────────────────────────────────────

#[tokio::test]
async fn r103_directory_user_gets_no_token_and_a_minted_one_is_refused() {
    use sui_id_store::user_source::InMemoryUserSource;
    let (mut state, mailer) = test_app_with_mailer();
    complete_setup_and_login(&state).await;
    enable_smtp(&state).await;
    let mut users = std::collections::HashMap::new();
    users.insert(
        "bob".to_string(),
        (
            "bob-directory-password".to_string(),
            "uuid-bob".to_string(),
            Some("bob@test.invalid".to_string()),
            None,
        ),
    );
    state.user_sources = vec![Arc::new(InMemoryUserSource {
        slug: "corp".into(),
        users,
    })];
    // Sign bob in once so the directory shadow row exists.
    let login = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(
                "username=bob&password=bob-directory-password&next=/me/security",
            ))
            .expect("req"),
    )
    .await;
    assert!(
        login.status.is_redirection(),
        "bob signs in: {}",
        login.status
    );
    let bob = sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .await
        .expect("bob shadow row");
    assert_ne!(bob.source, sui_id_store::models::UserSource::Local);

    // The request looks exactly like one for an unknown address.
    let before_requested = events(&state, "auth.password.reset_requested").await;
    let unknown = request_reset(&state, "nobody@test.invalid").await;
    let directory = request_reset(&state, "bob@test.invalid").await;
    assert_eq!(unknown.status, directory.status);
    assert_eq!(unknown.body, directory.body);
    assert_eq!(mailer.count().await, 0, "no mail for a directory account");
    assert_eq!(
        scalar(
            &state,
            format!(
                "SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = '{}'",
                bob.id
            )
        )
        .await,
        0,
        "no token issued for a directory account"
    );
    assert_eq!(
        scalar(
            &state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_requested' \
             AND target IS NOT NULL"
                .into()
        )
        .await,
        0,
        "the directory request names no user, like an unknown address"
    );
    assert_eq!(
        events(&state, "auth.password.reset_requested").await,
        before_requested + 2
    );

    // A token minted directly (as if issued before this fix) is refused.
    let token_id = mint_token(&state, bob.id, "directly-minted-token").await;
    let r = complete(&state, "directly-minted-token").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&r), "invalid-link page expected");
    assert_eq!(
        password_hash(&state, bob.id).await,
        None,
        "no local credential"
    );
    assert!(
        !token_consumed(&state, token_id).await,
        "the refusal rolled back"
    );
    assert_eq!(events(&state, "auth.password.reset_completed").await, 0);
}

// ── D13: in-transaction re-read ──────────────────────────────────────

#[tokio::test]
async fn r103_user_disabled_after_issuance_is_refused_and_rolled_back() {
    let (state, mailer, admin) = reset_app().await;
    let (token, token_id) = issue_token(&state, &mailer).await;
    let hash_before = password_hash(&state, admin).await;
    exec(
        &state,
        format!("UPDATE users SET is_disabled = 1 WHERE id = '{admin}'"),
    )
    .await;

    let r = complete(&state, &token).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&r));
    assert_eq!(password_hash(&state, admin).await, hash_before);
    assert!(!token_consumed(&state, token_id).await, "rolled back");
    assert_eq!(events(&state, "auth.password.reset_completed").await, 0);
}

#[tokio::test]
async fn r103_user_deleted_after_issuance_is_refused() {
    let (state, mailer, admin) = reset_app().await;
    let (token, token_id) = issue_token(&state, &mailer).await;
    let hash_before = password_hash(&state, admin).await;
    exec(
        &state,
        format!("UPDATE users SET is_deleted = 1 WHERE id = '{admin}'"),
    )
    .await;

    let r = complete(&state, &token).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&r));
    assert_eq!(password_hash(&state, admin).await, hash_before);
    assert!(!token_consumed(&state, token_id).await);
}

// ── D13: guarded consume ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn r103_two_concurrent_completions_change_the_password_once() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;
    let csrf = csrf_from(&state, "/reset-password").await;

    let a = tokio::spawn({
        let (state, csrf, token) = (state.clone(), csrf.clone(), token.clone());
        async move { send(&state, complete_request(&csrf, &token)).await.status }
    });
    let b = tokio::spawn({
        let (state, csrf, token) = (state.clone(), csrf.clone(), token.clone());
        async move { send(&state, complete_request(&csrf, &token)).await.status }
    });
    let statuses = [a.await.expect("a"), b.await.expect("b")];
    assert_eq!(
        statuses.iter().filter(|s| s.is_redirection()).count(),
        1,
        "exactly one completion succeeds: {statuses:?}"
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::BAD_REQUEST)
            .count(),
        1
    );
    assert_eq!(events(&state, "auth.password.reset_completed").await, 1);
}

/// The store command itself refuses a second consume of the same token,
/// even when the caller's earlier read saw it unconsumed.
#[tokio::test]
async fn r103_the_command_consumes_a_token_only_once() {
    let (state, mailer, admin) = reset_app().await;
    let (_token, token_id) = issue_token(&state, &mailer).await;
    let credential = |hash: &str| sui_id_store::models::CredentialRow {
        user_id: admin,
        password_hash: hash.into(),
        must_change: false,
        updated_at: chrono::Utc::now(),
    };
    sui_id_store::commands::consume_and_reset_password(
        &state.db,
        admin,
        token_id,
        credential("first"),
        chrono::Utc::now(),
        false,
    )
    .await
    .expect("first consume");
    let second = sui_id_store::commands::consume_and_reset_password(
        &state.db,
        admin,
        token_id,
        credential("second"),
        chrono::Utc::now(),
        false,
    )
    .await;
    assert!(
        matches!(second, Err(sui_id_store::StoreError::NotFound)),
        "second consume must fail"
    );
    assert_eq!(password_hash(&state, admin).await.as_deref(), Some("first"));
    assert_eq!(events(&state, "auth.password.reset_completed").await, 1);
}

// ── D10: the token stays out of URLs and logs ────────────────────────

#[derive(Clone, Default)]
pub(super) struct Captured(pub(super) Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn r103_token_appears_in_no_log_line_and_no_request_uri() {
    let (mut state, mailer, _admin) = reset_app().await;
    let mut config = (*state.config).clone();
    config.log.access_log = true;
    state.config = Arc::new(config);

    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    // Other tests run in parallel with no subscriber; without a rebuild a
    // callsite they reached first can stay cached as disabled.
    tracing::callsite::rebuild_interest_cache();

    let (token, _) = issue_token(&state, &mailer).await;
    let page = get(&state, "/reset-password").await;
    assert!(page.body.contains(r#"name="token""#));
    assert!(!page.body.contains(&token));
    let ok = complete(&state, &token).await;
    assert!(ok.status.is_redirection(), "reset succeeds: {}", ok.status);
    assert_eq!(ok.location.as_deref(), Some("/admin/login?reset=ok"));
    // A used link is an ordinary refusal: logged at info (RFC 103 stage
    // 2), without the token. `r103_stage2` covers the error level.
    tracing::callsite::rebuild_interest_cache();
    let replay = complete(&state, &token).await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST);
    assert!(is_invalid_link_page(&replay));

    let logged = String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned();
    assert!(
        logged.contains("uri=/reset-password"),
        "the access log is active:\n{logged}"
    );
    let line = logged
        .lines()
        .find(|l| l.contains("password reset completion refused"))
        .unwrap_or_else(|| panic!("no log line for the refused completion:\n{logged}"));
    assert!(line.contains(" INFO "), "wrong level: {line}");
    assert!(
        !logged.contains(" ERROR "),
        "no error line for a used link:\n{logged}"
    );
    assert!(line.contains("request_id="), "no request id: {line}");
    assert!(!logged.contains(&token), "the token leaked into the log");
    for uri in logged.split_whitespace().filter(|w| w.starts_with("uri=")) {
        assert!(
            !uri.contains("token"),
            "a request URI carries a token: {uri}"
        );
    }
}

#[tokio::test]
async fn r103_query_string_token_is_not_processed() {
    let (state, mailer, admin) = reset_app().await;
    let (token, token_id) = issue_token(&state, &mailer).await;
    let hash_before = password_hash(&state, admin).await;

    let r = get(&state, &format!("/reset-password?token={token}")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(is_invalid_link_page(&r), "request-a-new-link page expected");
    assert!(!r.body.contains(&token), "the token is not echoed");
    assert!(!token_consumed(&state, token_id).await);
    assert_eq!(password_hash(&state, admin).await, hash_before);
}

#[tokio::test]
async fn r103_mail_link_carries_the_token_in_the_fragment_from_the_issuer() {
    let (state, mailer, _admin) = reset_app().await;
    let (token, _) = issue_token(&state, &mailer).await;
    let mail = mailer.last().await.expect("mail");
    assert!(
        mail.text_body
            .contains(&format!("https://idp.test/reset-password#t={token}"))
    );
    assert!(!mail.text_body.contains("?token="));
    // The token is also given as text for the no-JavaScript paste field.
    assert!(mail.text_body.lines().any(|l| l.trim() == token.as_str()));
}

#[tokio::test]
async fn r103_fragment_script_is_served_and_allowed_by_the_csp() {
    let (state, _mailer, _admin) = reset_app().await;
    let page = get(&state, "/reset-password").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body
            .contains(r#"<script src="/static/reset-password.js"></script>"#)
    );
    // No inline script: every <script> tag on the page loads a same-origin file.
    for tag in page.body.split("<script").skip(1) {
        assert!(
            tag.trim_start().starts_with("src=\"/static/"),
            "inline script: {tag}"
        );
    }
    let csp = page
        .headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .expect("CSP header");
    assert!(csp.contains("script-src 'self'"), "CSP: {csp}");
    // Without JavaScript the paste field is visible.
    assert!(page.body.contains(r#"id="reset-token-field""#));
    assert!(!page.body.contains(r#"id="reset-token-field" hidden"#));

    let script = get(&state, "/static/reset-password.js").await;
    assert_eq!(script.status, StatusCode::OK);
    assert!(
        script
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/javascript"))
    );
    assert!(script.body.contains("history.replaceState"));
    assert!(script.body.contains("reset-token"));
}
