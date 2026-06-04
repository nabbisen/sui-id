//! RFC 020 — User identity invariants and OIDC claim consistency.
//!
//! Covers:
//! 1. Email case-fold round-trip: a user registered with a mixed-case
//!    email can request a password reset using the lowercase form.
//! 2. Email uniqueness across cases: creating two users differing only
//!    in email case is rejected.
//! 3. UserInfo with `email` scope returns `email` + `email_verified`.
//! 4. UserInfo without `email` scope omits both claims.
//! 5. UserInfo with no user email omits both claims.

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use tower::ServiceExt;
use url::Url;

use super::common::*;
use sui_id::build_router;

// ── helpers ───────────────────────────────────────────────────────────

async fn enable_smtp_row(state: &sui_id::AppState) {
    state
        .db
        .with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO smtp_config(id, host, port, username, \
                 password_enc, from_address, from_name, base_url, \
                 tls_mode, created_at, updated_at, enabled) \
                 VALUES('singleton', 'smtp.test', 587, 'user', X'', 'noreply@test.invalid', \
                 'sui-id Test', 'https://idp.test.invalid', \
                 'starttls', datetime('now'), datetime('now'), 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("enable smtp row");
}

async fn post_forgot_password(
    state: &sui_id::AppState,
    email: &str,
) -> axum::response::Response {
    let get_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/forgot-password")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET forgot-password");
    let csrf = extract_set_cookie(get_resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let encoded_email = urlencode(email);
    let body = format!("_csrf={csrf}&email={encoded_email}");
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/forgot-password")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST forgot-password")
}

async fn get_access_token_with_scope(
    state: &sui_id::AppState,
    session: &str,
    client_id: &str,
    client_secret: &str,
    scope: &str,
) -> String {
    let (verifier, challenge) = pkce_pair();
    let scope_enc = urlencode(scope);
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}\
         &redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope={scope_enc}&state=xyz&nonce=n0\
         &code_challenge={challenge}&code_challenge_method=S256"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .expect("url")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");

    let token_body = format!(
        "grant_type=authorization_code&code={code}\
         &redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}\
         &code_verifier={verifier}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(token_body))
                .expect("req"),
        )
        .await
        .expect("token");
    let body = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    json["access_token"]
        .as_str()
        .expect("access_token")
        .to_owned()
}

// ── tests ─────────────────────────────────────────────────────────────

/// RFC 020 § 1: a user registered with a mixed-case email can request a
/// password reset using the lowercase form.
#[tokio::test]
async fn forgot_password_case_insensitive() {
    let (state, mailer) = test_app_with_mailer();
    let _ = complete_setup_and_login(&state).await;
    enable_smtp_row(&state).await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("find user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("Alice@Example.com"),
        chrono::Utc::now(),
    ).await
    .expect("set mixed-case email");

    let resp = post_forgot_password(&state, "alice@example.com").await;
    assert_eq!(resp.status(), StatusCode::OK);

    assert_eq!(
        mailer.count().await,
        1,
        "expected one reset email for case-folded lookup"
    );
    let mail = mailer.last().await.expect("mail");
    assert_eq!(mail.to, "Alice@Example.com");
}

/// RFC 020 § 2: creating two users whose emails differ only in case must fail.
#[tokio::test]
async fn email_uniqueness_is_case_insensitive() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("find user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("admin@example.com"),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "username=bob2\
         &password=bob-password-secure-12\
         &confirm_password=bob-password-secure-12\
         &email=Admin%40EXAMPLE.COM\
         &_csrf={csrf}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/users")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("POST create user");

    assert_ne!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "duplicate email (different case) must not create a user"
    );
}

/// RFC 020 § 3: userinfo returns email claims when scope includes email.
#[tokio::test]
async fn userinfo_returns_email_when_scope_email() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client_with_scopes(&state, &session, "openid email").await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("find user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("alice@idp.test"),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    let access = get_access_token_with_scope(
        &state, &session, &client_id, &client_secret, "openid email",
    )
    .await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/oauth2/userinfo")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("userinfo");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let info: serde_json::Value = serde_json::from_slice(&body).expect("json");

    assert_eq!(info["email"].as_str(), Some("alice@idp.test"));
    assert_eq!(
        info["email_verified"].as_bool(),
        Some(false),
        "email_verified is false until a verification flow ships"
    );
}

/// RFC 020 § 3: email claims are absent when scope is openid-only.
#[tokio::test]
async fn userinfo_omits_email_without_email_scope() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client_with_scopes(&state, &session, "openid email").await;

    let user =
        sui_id_store::repos::users::find_by_username(&state.db, USERNAME).await.expect("find user");
    sui_id_store::repos::users::update_email(
        &state.db,
        user.id,
        Some("alice@idp.test"),
        chrono::Utc::now(),
    ).await
    .expect("set email");

    let access =
        get_access_token_with_scope(&state, &session, &client_id, &client_secret, "openid").await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/oauth2/userinfo")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("userinfo");
    let body = read_body(resp.into_body()).await;
    let info: serde_json::Value = serde_json::from_slice(&body).expect("json");

    assert!(info.get("email").is_none(), "email must be absent with openid scope only");
    assert!(info.get("email_verified").is_none());
}

/// RFC 020 § 3: email claims are absent when the user has no email, even
/// with scope=email.
#[tokio::test]
async fn userinfo_omits_email_when_user_has_no_email() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client_with_scopes(&state, &session, "openid email").await;

    // The default test user has no email after complete_setup_and_login.
    let access = get_access_token_with_scope(
        &state, &session, &client_id, &client_secret, "openid email",
    )
    .await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/oauth2/userinfo")
                .header(header::AUTHORIZATION, format!("Bearer {access}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("userinfo");
    let body = read_body(resp.into_body()).await;
    let info: serde_json::Value = serde_json::from_slice(&body).expect("json");

    assert!(
        info.get("email").is_none(),
        "email absent when user has no email on record"
    );
    assert!(info.get("email_verified").is_none());
}
