//! Idle session timeout and concurrent-session cap (v0.25.0).
//!
//! Part of the integration test binary; helpers come from
//! [`super::common`].

#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;

use super::common::*;
use tower::ServiceExt;

// ---------- v0.25.0: Idle session timeout + concurrent session cap ----------

/// Default mode: both knobs are 0, so an idle / over-cap session
/// behaves identically to pre-v0.25.0. Pin this so we don't break
/// the "no opt-in = no behaviour change" promise.
#[tokio::test]
async fn session_no_idle_timeout_when_disabled() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Reach back into the DB and make the session look like it
    // was last used 30 days ago. With idle_session_timeout_secs
    // = 0 (default), this should still be valid.
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");
    let stale = chrono::Utc::now() - chrono::Duration::days(30);
    let user_id_owned = user.id;
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE user_id = ?2",
                rusqlite::params![stale, user.id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .await
        .expect("set stale");
    // Hitting an admin page should still 200 OK.
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin");
    assert_eq!(resp.status(), StatusCode::OK);
}

/// With idle timeout enabled, an authenticated request whose
/// session has been idle past the window is rejected.
#[tokio::test]
async fn session_idle_timeout_revokes_after_window() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    // Configure a 60-second idle timeout.
    sui_id_store::repos::server_settings::update_idle_session_timeout(
        &state.db,
        60,
        chrono::Utc::now(),
    )
    .await
    .expect("set timeout");
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");
    let stale = chrono::Utc::now() - chrono::Duration::seconds(120);
    let user_id_owned = user.id;
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE sessions SET last_used_at = ?1 WHERE user_id = ?2",
                rusqlite::params![stale, user.id.to_string()],
            )
            .expect("update");
            Ok(())
        })
        .await
        .expect("set stale");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("admin");
    // The CurrentAdmin extractor should refuse — redirect to
    // /admin/login or 401, depending on the rejection mapping.
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status().is_redirection(),
        "expected redirect or 401, got {}",
        resp.status()
    );
    // The session has been revoked in-place.
    let uid_for_count = user.id;
    let count: i64 = state
        .db
        .with_conn(move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1 AND revoked_at IS NULL",
                rusqlite::params![uid_for_count.to_string()],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .expect("count");
    assert_eq!(count, 0, "expected session to be revoked");
}

/// FIFO eviction: cap = 2, login 3 times → first session is
/// auto-revoked after the 3rd login.
#[tokio::test]
async fn session_cap_evicts_oldest_in_fifo() {
    let state = test_app();
    // First login also runs setup. After this, there is 1 active
    // session.
    let s1 = complete_setup_and_login(&state).await;
    // Set cap = 2 (only valid post-setup since the row needs to
    // exist).
    sui_id_store::repos::server_settings::update_max_concurrent_sessions(
        &state.db,
        2,
        chrono::Utc::now(),
    )
    .await
    .expect("set cap");

    // Login twice more; each via the regular login form.
    let login_once = || async {
        let body = format!(
            "username={USERNAME}&password={pw}",
            pw = urlencode(PASSWORD)
        );
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/admin/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .expect("req"),
            )
            .await
            .expect("login");
        assert!(
            resp.status().is_redirection() || resp.status() == StatusCode::SEE_OTHER,
            "expected login redirect, got {}",
            resp.status()
        );
        extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie")
    };
    let s2 = login_once().await;
    let s3 = login_once().await;

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");
    // Active count is now 2 (cap respected).
    let active: i64 = sui_id_store::repos::sessions::count_active_for_user(
        &state.db,
        user.id,
        chrono::Utc::now(),
    )
    .await
    .expect("count");
    assert_eq!(active, 2, "expected 2 active after 3 logins with cap 2");

    // s1 should have been revoked; s2 and s3 should be live.
    // Inline checks instead of closure because sessions::get is async.
    use std::str::FromStr;
    let sid1 = sui_id_shared::ids::SessionId::from_str(&s1).expect("parse s1");
    let sid2 = sui_id_shared::ids::SessionId::from_str(&s2).expect("parse s2");
    let sid3 = sui_id_shared::ids::SessionId::from_str(&s3).expect("parse s3");
    assert!(
        sui_id_store::repos::sessions::get(&state.db, sid1)
            .await
            .expect("get s1")
            .revoked_at
            .is_some(),
        "s1 should be revoked (FIFO)"
    );
    assert!(
        sui_id_store::repos::sessions::get(&state.db, sid2)
            .await
            .expect("get s2")
            .revoked_at
            .is_none(),
        "s2 should remain"
    );
    assert!(
        sui_id_store::repos::sessions::get(&state.db, sid3)
            .await
            .expect("get s3")
            .revoked_at
            .is_none(),
        "s3 should remain"
    );
}

/// Cap = 0 (default) — cap disabled, login N times yields N
/// sessions.
#[tokio::test]
async fn session_cap_disabled_keeps_all_sessions() {
    let state = test_app();
    let _s1 = complete_setup_and_login(&state).await;
    // Default cap = 0 = disabled.

    // Login twice more without changing the cap.
    let login_once = || async {
        let body = format!(
            "username={USERNAME}&password={pw}",
            pw = urlencode(PASSWORD)
        );
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/admin/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .expect("req"),
            )
            .await
            .expect("login");
        let _ = resp.status();
    };
    login_once().await;
    login_once().await;

    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("user");
    let active: i64 = sui_id_store::repos::sessions::count_active_for_user(
        &state.db,
        user.id,
        chrono::Utc::now(),
    )
    .await
    .expect("count");
    assert_eq!(active, 3, "all 3 sessions active when cap disabled");
}

/// Admin POST /admin/settings/security/idle-timeout updates the
/// stored value.
#[tokio::test]
async fn admin_settings_security_idle_timeout_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&secs=900");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/idle-timeout")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(row.idle_session_timeout_secs, 900);
}

/// Admin POST /admin/settings/security/max-sessions updates the
/// stored cap. Also covers out-of-range rejection.
#[tokio::test]
async fn admin_settings_security_max_sessions_change() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/security")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let body = format!("_csrf={csrf}&cap=5");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/max-sessions")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(resp.status().is_redirection());

    let row = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(row.max_concurrent_sessions, 5);

    // Out-of-range (>1000) is rejected.
    let body = format!("_csrf={csrf}&cap=99999");
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/security/max-sessions")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("post");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx for out-of-range cap, got {}",
        resp.status()
    );
    // The cap stays at 5 (unchanged).
    let row = sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings");
    assert_eq!(row.max_concurrent_sessions, 5);
}
