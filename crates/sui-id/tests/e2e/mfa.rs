//! MFA / TOTP enrolment, login challenge, recovery codes, disable, admin-initiated reset.
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use sui_id::build_router;

use tower::ServiceExt;
use super::common::*;

#[tokio::test]
async fn mfa_enroll_then_login_with_totp_succeeds() {
    use sui_id_core::totp;
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // Enrol.
    let (secret_b32, _codes) = enroll_mfa_for(&state, &session).await;
    let secret = decode_b32(&secret_b32);

    // Now: a fresh password login should *NOT* yield a session cookie
    // — instead it should set sui_id_pending_mfa and redirect.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "expected redirect to MFA");
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(loc.ends_with("/admin/login/mfa"), "got: {loc}");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa")
        .expect("pending_mfa cookie set");
    let session_cookie = extract_set_cookie(resp.headers(), "sui_id_session");
    assert!(session_cookie.is_none(), "session cookie must not be set before MFA");

    // Submit a fresh TOTP code.
    let now = chrono::Utc::now().timestamp();
    // Use step+1 to avoid the replay-defence cursor we set at enrolment time.
    let step = now / 30 + 1;
    let code = totp::code_for_step(&secret, step).await;

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code={code:06}&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "expected redirect to /admin");
    let session_cookie = extract_set_cookie(resp.headers(), "sui_id_session")
        .expect("session cookie issued after MFA success");
    assert!(!session_cookie.is_empty());
}

#[tokio::test]
async fn mfa_login_with_wrong_code_returns_401() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let _ = enroll_mfa_for(&state, &session).await;

    // Password login → pending.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending");

    // Wrong code.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("code=000000&_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_none());
}

#[tokio::test]
async fn mfa_login_with_recovery_code_succeeds_and_consumes_code() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (_secret_b32, codes) = enroll_mfa_for(&state, &session).await;
    let one_code = codes[0].clone();

    // Password login → pending.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending");

    // Submit recovery code.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!(
            "code={}&_csrf={csrf}",
            utf8_encode(&one_code)
        )))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "recovery code should accept");
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());

    // Reusing the same recovery code must fail. We need a new pending row.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login 2");
    let pending2 = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending2");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending2}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET 2");
    let csrf2 = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf2");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login/mfa")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_pending_mfa={pending2}; sui_id_csrf={csrf2}"),
        )
        .body(Body::from(format!(
            "code={}&_csrf={csrf2}",
            utf8_encode(&one_code)
        )))
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa POST replay");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "recovery code must be single-use"
    );
}

#[tokio::test]
async fn mfa_disable_lets_user_log_in_with_password_only() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let _ = enroll_mfa_for(&state, &session).await;

    // Disable MFA.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/me/security/mfa/disable")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("disable");
    assert!(resp.status().is_redirection());

    // Password login should now go straight to a session.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=alice-the-tester-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login post-disable");
    assert!(resp.status().is_redirection());
    let loc = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    assert!(!loc.ends_with("/admin/login/mfa"));
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());
}

// ---------- admin-initiated MFA reset (v0.10.0) ----------

#[tokio::test]
async fn admin_can_reset_users_mfa_factors() {
    use sui_id_core::admin::{admin_reset_mfa, CreateUserSpec};
    use sui_id_core::mfa;
    use sui_id_core::time::system_clock;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice").await
        .expect("admin")
        .id;
    let clock = system_clock();

    // Create a second user (the target) and enrol TOTP for them.
    sui_id_core::admin::create_user(
        &state.db,
        &state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        CreateUserSpec {
            username: "bob",
            password: "bob-very-strong-password",
            display_name: None,
            email: None,
            is_admin: false,
        },
    ).await
    .expect("create");
    let bob = sui_id_store::repos::users::find_by_username(&state.db, "bob").await
        .expect("bob")
        .id;
    let ticket = mfa::start_enrollment(&state.db, "sui-id", bob, "bob").await.expect("start");
    let step = clock.now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&ticket.secret, step).await;
    let _ = mfa::confirm_enrollment(&state.db, &clock, bob, code).await.expect("confirm");
    assert!(mfa::is_mfa_enabled(&state.db, bob).await.unwrap());

    // Admin resets it.
    let report = admin_reset_mfa(&state.db, admin_id, bob).await.expect("reset");
    assert!(report.totp_removed);
    assert_eq!(report.passkeys_removed, 0);

    // MFA is now off for bob, and the audit log captured the reset.
    assert!(!mfa::is_mfa_enabled(&state.db, bob).await.unwrap());
    let audit = sui_id_store::repos::audit::recent(&state.db, 50).await.expect("audit");
    let reset_entries: Vec<_> = audit
        .iter()
        .filter(|e| e.action == "mfa.admin_reset")
        .collect();
    assert_eq!(reset_entries.len(), 1, "exactly one reset row expected");
    assert_eq!(reset_entries[0].actor, Some(admin_id));
    assert_eq!(reset_entries[0].target, Some(bob.to_string()));
    let note = reset_entries[0].note.as_deref().unwrap_or("");
    assert!(
        note.contains("totp=removed") && note.contains("passkeys=0"),
        "audit note should describe what was removed; got {note:?}"
    );
}

#[tokio::test]
async fn admin_mfa_reset_via_http_redirects_and_disables_mfa_requirement() {
    use sui_id_core::admin::CreateUserSpec;
    use sui_id_core::mfa;
    use sui_id_core::time::system_clock;

    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin_id = sui_id_store::repos::users::find_by_username(&state.db, "alice").await
        .expect("admin")
        .id;
    let clock = system_clock();

    sui_id_core::admin::create_user(
        &state.db,
        &state.clock,
        None,
        sui_id_store::models::HibpMode::Off,
        admin_id,
        CreateUserSpec {
            username: "carol",
            password: "carol-very-strong-password",
            display_name: None,
            email: None,
            is_admin: false,
        },
    ).await
    .expect("create");
    let carol = sui_id_store::repos::users::find_by_username(&state.db, "carol").await
        .expect("carol")
        .id;
    let ticket = mfa::start_enrollment(&state.db, "sui-id", carol, "carol").await.expect("start");
    let step = clock.now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&ticket.secret, step).await;
    let _ = mfa::confirm_enrollment(&state.db, &clock, carol, code).await.expect("confirm");

    // Sanity: a fresh password login for carol now goes to MFA challenge.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=carol&password=carol-very-strong-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(loc.ends_with("/admin/login/mfa"), "got: {loc}");

    // Admin issues the reset via the HTTP endpoint.
    let csrf = fetch_csrf(&state, &session).await;
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("/admin/users/{carol}/mfa-reset"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(
            header::COOKIE,
            format!("sui_id_session={session}; sui_id_csrf={csrf}"),
        )
        .body(Body::from(format!("_csrf={csrf}")))
        .expect("req");
    let resp = router.oneshot(req).await.expect("reset");
    assert!(resp.status().is_redirection(), "got {}", resp.status());
    assert_eq!(
        resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/admin/users")
    );

    // Now carol's password login goes straight to a session.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=carol&password=carol-very-strong-password"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login post-reset");
    assert!(resp.status().is_redirection());
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(!loc.ends_with("/admin/login/mfa"), "should not require MFA; got: {loc}");
    assert!(extract_set_cookie(resp.headers(), "sui_id_session").is_some());
}

