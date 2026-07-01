//! Self-service password change (v0.19.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;
use url::Url;

// ---------- self-service password change (v0.19.0) ----------

#[tokio::test]
async fn me_password_change_form_renders() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("password GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("パスワードを変更"));
    assert!(s.contains(r#"name="current_password""#));
    assert!(s.contains(r#"name="new_password""#));
    assert!(s.contains(r#"name="confirm_password""#));
    assert!(s.contains(r#"name="revoke_others""#));
}

#[tokio::test]
async fn me_password_change_happy_path_replaces_password() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-new-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw) // revoke_others omitted = unchecked for this test, so we can
                          // inspect "old session still alive" cleanly afterwards
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(
        resp.status().is_redirection(),
        "expected redirect; got {}",
        resp.status()
    );

    // Old password no longer logs in; new one does.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}",
                    urlencode(USERNAME),
                    urlencode(PASSWORD)
                )))
                .expect("req"),
        )
        .await
        .expect("login old");
    let old_cookie = extract_set_cookie(resp.headers(), "sui_id_session");
    assert!(
        old_cookie.is_none(),
        "old password must no longer authenticate"
    );

    let new_cookie = login_again_for_admin(&state, USERNAME, new_pw).await;
    assert!(!new_cookie.is_empty());
}

#[tokio::test]
async fn me_password_change_wrong_current_is_refused() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode("wrong-current-tester-password"),
        urlencode("the-new-tester-password"),
        urlencode("the-new-tester-password")
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    // Refused — not redirected to /me/security?msg=password_changed.
    assert_ne!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "wrong current password must NOT yield a success redirect"
    );

    // Original password must still work.
    let cookie = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert!(
        !cookie.is_empty(),
        "original password must still authenticate"
    );
}

#[tokio::test]
async fn me_password_change_mismatched_confirm_is_refused() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}",
        csrf,
        urlencode(PASSWORD),
        urlencode("the-new-tester-password"),
        urlencode("typed-it-differently-the-second-time")
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert_ne!(resp.status(), StatusCode::SEE_OTHER);

    let cookie = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert!(!cookie.is_empty());
}

#[tokio::test]
async fn me_password_change_with_revoke_others_sweeps_other_sessions_and_refresh_tokens() {
    let state = test_app();
    let s1 = complete_setup_and_login(&state).await;
    let s2 = login_again_for_admin(&state, USERNAME, PASSWORD).await;
    assert_ne!(s1, s2);

    // Mint a refresh token through the OAuth flow, so we have
    // something concrete to verify gets revoked.
    let (client_id, client_secret) = create_client(&state, &s1).await;
    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=z&code_challenge={challenge}&code_challenge_method=S256"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location")
        .to_owned();
    let code = Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code");
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("token");
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let refresh = json["refresh_token"].as_str().expect("rt").to_owned();

    // Now do the password change with revoke_others=1.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let new_pw = "the-rotated-tester-password";
    let body = format!(
        "_csrf={}&current_password={}&new_password={}&confirm_password={}&revoke_others=1",
        csrf,
        urlencode(PASSWORD),
        urlencode(new_pw),
        urlencode(new_pw)
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/password")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={s1}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("change");
    assert!(resp.status().is_redirection());

    // s1 (current session) is still alive.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s1}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("s1 probe");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "current session must remain alive across password change"
    );

    // s2 must be dead.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security")
                .header(header::COOKIE, format!("sui_id_session={s2}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("s2 probe");
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "non-current session must be revoked"
    );

    // Refresh token must be dead.
    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}\
         &client_id={client_id}&client_secret={client_secret}"
    );
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("rt redeem");
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "refresh token must be revoked across password change"
    );
}
