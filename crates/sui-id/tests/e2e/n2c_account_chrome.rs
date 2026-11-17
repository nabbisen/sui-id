//! RFC 144 N2c: a signed-in person outside the console — `role` cannot
//! read the admin console, neither admin nor auditor — sees the
//! account menu alone on their own `/me` pages, and no console link to
//! a page the server would then refuse. The same pages, for an admin,
//! still carry the full console navigation. Sign-out still works for a
//! person outside the console.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

async fn get(state: &AppState, uri: &str, session: &str) -> axum::response::Response {
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
        .expect("GET")
}

/// Signs a plain, non-admin user in through the same `/admin/login`
/// form the console uses. `login_post` establishes a session for any
/// role once the destination is under `/me/` (`SessionAudience::Any`),
/// so this needs no OIDC round-trip through the dev client — the same
/// shortcut the preparation review's F3 measurement used.
async fn login_as(state: &AppState, username: &str, password: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={username}&password={password}&next=/me/security/overview"
                )))
                .expect("req"),
        )
        .await
        .expect("login POST");
    assert!(
        resp.status().is_redirection(),
        "expected the non-admin login to redirect, got {}",
        resp.status()
    );
    extract_set_cookie(resp.headers(), "sui_id_session").expect("session cookie set")
}

async fn fetch_csrf_for_me(state: &AppState, session: &str) -> String {
    let resp = get(state, "/me/security/overview", session).await;
    extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie on /me GET")
}

/// The `/me` routes that render a page on a plain GET with no further
/// precondition — RFC 144 N2c's "Where", built from the router
/// (`crates/sui-id/src/http/router.rs`'s `/me/*` GETs).
const ME_PAGE_ROUTES: &[&str] = &[
    "/me/security/overview",
    "/me/security/password",
    "/me/security/mfa",
    "/me/security/passkeys",
    "/me/security/sessions",
    "/me/security/language",
    "/me/apps",
];

#[tokio::test]
async fn a_person_outside_the_console_sees_the_account_menu_and_no_console_link() {
    let state = test_app();
    let _admin_session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");

    let target_username = "n2c-outside-the-console";
    let target_password = "n2c-outside-the-console-pw-1";
    create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: target_username,
            display_name: None,
            email: None,
            is_admin: false,
        },
        target_password,
    )
    .await
    .expect("create non-admin user");

    let session = login_as(&state, target_username, target_password).await;

    assert_eq!(
        ME_PAGE_ROUTES.len(),
        7,
        "RFC 144 N2c's `/me` page-rendering GET route count"
    );
    for uri in ME_PAGE_ROUTES {
        let resp = get(&state, uri, &session).await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri}: expected 200");
        let body = read_body(resp.into_body()).await;
        let html = String::from_utf8_lossy(&body).to_string();
        assert!(
            html.contains(r#"class="user-menu""#),
            "{uri}: no account menu rendered for a person outside the console"
        );
        assert!(
            html.contains(target_username),
            "{uri}: account menu does not show the signed-in name"
        );
        // The sign-out form's own `action="/admin/logout"` is the one
        // sanctioned `/admin` reference; no console *link* may appear.
        assert!(
            !html.contains(r#"href="/admin"#),
            "{uri}: rendered a console link to a page this person cannot open"
        );
    }

    // Sign-out still works for a person outside the console (§1).
    let csrf = fetch_csrf_for_me(&state, &session).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/logout")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::from(format!("_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("logout POST");
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert!(
        html.contains("You are signed out."),
        "sign-out for a person outside the console must say so plainly"
    );

    let resp = get(&state, "/me/security/overview", &session).await;
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "the old session cookie must no longer open a /me page after sign-out"
    );
}

#[tokio::test]
async fn an_admin_on_the_same_routes_still_sees_the_console_navigation() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for uri in ME_PAGE_ROUTES {
        let resp = get(&state, uri, &session).await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri}: expected 200");
        let body = read_body(resp.into_body()).await;
        let html = String::from_utf8_lossy(&body).to_string();
        assert!(
            html.contains(r#"href="/admin/users""#),
            "{uri}: an admin must still see the console navigation"
        );
    }
}
