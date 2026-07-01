//! Account lockout after repeated failed login (v0.16.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- account lockout (v0.16.0) ----------

#[tokio::test]
async fn three_consecutive_wrong_passwords_lock_the_account() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // First two failures: counter bumps but no lock.
    for attempt in 0..2 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let resp = router.oneshot(req).await.expect("login");
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {attempt}: expected 401"
        );
    }

    // Third failure: still 401 — the lock is now stamped, but the
    // response shape is identical to the previous failures (timing
    // and status code both indistinguishable to a remote observer).
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("username=alice&password=wrong-password-here"))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Fourth attempt with the *correct* password must still be
    // refused: the account is locked.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "username=alice&password=alice-the-tester-password",
        ))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "locked account must refuse even the correct password"
    );

    // Confirm the audit log records `auth.login.locked`.
    let recent = sui_id_store::repos::audit::recent(&state.db, 50)
        .await
        .expect("audit list");
    let count = recent
        .iter()
        .filter(|r| r.action == "auth.login.locked")
        .count();
    assert!(
        count >= 1,
        "expected at least one auth.login.locked audit row; got {count}"
    );
}

#[tokio::test]
async fn admin_unlock_clears_an_active_lock() {
    use sui_id_store::repos::users;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // Drive the account to a locked state.
    for _ in 0..3 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let _ = router.oneshot(req).await.expect("login");
    }
    let alice = users::find_by_username(&state.db, "alice")
        .await
        .expect("alice");
    assert!(alice.locked_until.is_some(), "expected locked");
    assert!(alice.failed_login_count >= 3);

    // Direct admin_unlock — same call the CLI subcommand makes.
    users::admin_unlock(&state.db, alice.id)
        .await
        .expect("unlock");

    let alice2 = users::find_by_username(&state.db, "alice")
        .await
        .expect("alice");
    assert!(alice2.locked_until.is_none(), "lock should be cleared");
    assert_eq!(alice2.failed_login_count, 0);

    // After the unlock, the correct password must succeed.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "username=alice&password=alice-the-tester-password",
        ))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "post-unlock login should succeed"
    );
}

#[tokio::test]
async fn successful_login_clears_partial_failure_count() {
    use sui_id_store::repos::users;

    let state = test_app();
    let _ = complete_setup_and_login(&state).await;

    // Two failures (under the threshold; no lock yet).
    for _ in 0..2 {
        let router = build_router(state.clone());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("username=alice&password=wrong-password-here"))
            .expect("req");
        let _ = router.oneshot(req).await.expect("login");
    }
    let alice = users::find_by_username(&state.db, "alice")
        .await
        .expect("alice");
    assert_eq!(alice.failed_login_count, 2);
    assert!(alice.locked_until.is_none());

    // Then a successful login. The counter must reset to 0.
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "username=alice&password=alice-the-tester-password",
        ))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let alice2 = users::find_by_username(&state.db, "alice")
        .await
        .expect("alice");
    assert_eq!(
        alice2.failed_login_count, 0,
        "successful login must reset counter"
    );
}
