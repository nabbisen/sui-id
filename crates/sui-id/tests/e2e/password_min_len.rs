//! RFC 151 — the password minimum can be raised, never lowered.
//!
//! Through the real HTTP router: the *Authentication* settings tab's
//! own save, a password reset's completion, and the self-service
//! change, all reading the one live setting
//! (`server_settings.password_min_len`).

use super::common::*;
use super::r103_stage1::{csrf_from, mint_token, send};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use tower::ServiceExt;

/// `resolve_admin_locale`'s own order is the admin's `preferred_lang`
/// first, then `server_settings.default_lang`, then a hardcoded
/// `Locale::Ja` fallback -- never `Accept-Language` for an admin page.
/// A freshly set-up admin has no preference set, so setting the
/// server default here is what makes admin pages render English.
async fn app() -> (AppState, UserId, String) {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice")
        .id;
    sui_id_store::commands::change_default_lang(&state.db, id, "en".into(), chrono::Utc::now())
        .await
        .expect("set server default language to English");
    (state, id, session)
}

/// Raises the minimum directly through the sealed command -- the same
/// seam the *Authentication* tab's own save calls. Setup, not the
/// feature under test in most of these.
async fn raise_minimum_to(state: &AppState, admin: UserId, n: i64) {
    sui_id_store::commands::change_password_min_len(&state.db, admin, n, chrono::Utc::now())
        .await
        .expect("raise the minimum");
}

async fn stored_minimum(state: &AppState) -> i64 {
    sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings")
        .password_min_len
}

/// `UPDATE users SET role = 'auditor'` directly -- no HTTP path creates
/// an auditor in this suite (the same shape
/// `r096_b2_stage5_admin_surface.rs::demote_to_auditor` already uses).
async fn demote_to_auditor(state: &AppState, user_id: UserId) {
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE users SET role = 'auditor' WHERE id = ?1",
                [user_id.to_string()],
            )?)
        })
        .await
        .expect("demote to auditor");
}

fn reset_complete_request(csrf: &str, token: &str, password: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/reset-password")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
        .body(Body::from(format!(
            "_csrf={csrf}&token={}&password={}&confirm_password={}",
            urlencode(token),
            urlencode(password),
            urlencode(password)
        )))
        .expect("req")
}

// ── the reset-completion path ────────────────────────────────────────

#[tokio::test]
async fn a_raised_minimum_refuses_a_short_reset_then_accepts_the_new_length() {
    let (state, admin, _session) = app().await;
    raise_minimum_to(&state, admin, 16).await;
    let token = "reset-token-for-rfc151-aaaaaaaa";
    mint_token(&state, admin, token).await;

    // Policy and breach refusals happen before the token is looked
    // up, so a refused attempt leaves it live for the next one.
    let csrf = csrf_from(&state, "/reset-password").await;
    let resp = send(
        &state,
        reset_complete_request(&csrf, token, "fifteen-chars!!"),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    assert!(resp.body.contains("16"), "{}", resp.body);
    assert!(!resp.body.contains(r#"id="reset-done""#), "{}", resp.body);

    let csrf2 = csrf_from(&state, "/reset-password").await;
    let resp2 = send(
        &state,
        reset_complete_request(&csrf2, token, "exactly-16-chars"),
    )
    .await;
    assert_eq!(resp2.status, StatusCode::OK, "{}", resp2.body);
    assert!(resp2.body.contains(r#"id="reset-done""#));
}

#[tokio::test]
async fn the_reset_page_hint_states_the_raised_minimum() {
    let (state, admin, _session) = app().await;
    raise_minimum_to(&state, admin, 16).await;

    let resp = send(
        &state,
        Request::builder()
            .method(Method::GET)
            .uri("/reset-password")
            .header(header::ACCEPT_LANGUAGE, "en")
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(
        resp.body.contains("At least 16 characters"),
        "{}",
        resp.body
    );
    assert!(resp.body.contains("minlength=\"16\""), "{}", resp.body);
}

#[tokio::test]
async fn the_reset_refusal_on_a_japanese_page_is_in_japanese_and_names_16() {
    let (state, admin, _session) = app().await;
    raise_minimum_to(&state, admin, 16).await;
    let token = "reset-token-for-rfc151-japanese0";
    mint_token(&state, admin, token).await;

    let csrf = csrf_from(&state, "/reset-password").await;
    let resp = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/reset-password")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
            .header(header::ACCEPT_LANGUAGE, "ja")
            .body(Body::from(format!(
                "_csrf={csrf}&token={}&password=short&confirm_password=short",
                urlencode(token)
            )))
            .expect("req"),
    )
    .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    assert!(resp.body.contains("16 文字以上"), "{}", resp.body);
}

// ── the self-service change path ─────────────────────────────────────

#[tokio::test]
async fn a_raised_minimum_refuses_a_short_self_service_change_then_accepts_the_new_length() {
    let (state, admin, session) = app().await;
    raise_minimum_to(&state, admin, 16).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/me/security/password")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");
    let body_bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("At least 16 characters"), "{body}");

    let post = |password: &str, csrf: &str| {
        Request::builder()
            .method(Method::POST)
            .uri("/me/security/password")
            .header(
                header::COOKIE,
                format!("sui_id_session={session}; sui_id_csrf={csrf}"),
            )
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "_csrf={csrf}&current_password={}&new_password={}&confirm_password={}",
                urlencode(PASSWORD),
                urlencode(password),
                urlencode(password)
            )))
            .expect("req")
    };

    let resp = build_router(state.clone())
        .oneshot(post("fifteen-chars!!", &csrf))
        .await
        .expect("refused change");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = read_body(resp.into_body()).await;
    let refused_body = String::from_utf8_lossy(&bytes);
    assert!(refused_body.contains("16"), "{refused_body}");

    let resp = build_router(state.clone())
        .oneshot(post("exactly-16-chars", &csrf))
        .await
        .expect("accepted change");
    assert!(resp.status().is_redirection(), "{}", resp.status());
}

// ── the *Authentication* settings tab ────────────────────────────────

#[tokio::test]
async fn an_administrator_raises_the_minimum_from_the_settings_tab() {
    let (state, _admin, session) = app().await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/authentication")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/authentication/password-min-len")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!("_csrf={csrf}&password_min_len=16")))
                .expect("req"),
        )
        .await
        .expect("save");
    assert!(resp.status().is_redirection(), "{}", resp.status());
    assert_eq!(stored_minimum(&state).await, 16);

    assert_eq!(
        sui_id_store::repos::server_settings::get(&state.db)
            .await
            .expect("settings")
            .password_min_len,
        16
    );
}

#[tokio::test]
async fn posting_11_names_the_floor_and_changes_nothing() {
    for bad in ["11", "abc", ""] {
        let (state, _admin, session) = app().await;
        let before = stored_minimum(&state).await;

        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/admin/settings/authentication")
                    .header(header::COOKIE, format!("sui_id_session={session}"))
                    .header(header::ACCEPT_LANGUAGE, "en")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("page");
        let csrf = extract_csrf_cookie(resp.headers()).expect("csrf");

        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/admin/settings/authentication/password-min-len")
                    .header(
                        header::COOKIE,
                        format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                    )
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!("_csrf={csrf}&password_min_len={bad}")))
                    .expect("req"),
            )
            .await
            .expect("refused save");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "input {bad:?}");
        let bytes = read_body(resp.into_body()).await;
        let body = String::from_utf8_lossy(&bytes);
        assert!(body.contains("between 12 and 128"), "input {bad:?}: {body}");
        assert_eq!(
            stored_minimum(&state).await,
            before,
            "input {bad:?}: nothing changed"
        );
    }
}

#[tokio::test]
async fn an_auditor_sees_a_static_row_and_their_post_is_refused() {
    let (state, admin, session) = app().await;
    demote_to_auditor(&state, admin).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/authentication")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("page");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(!body.contains("name=\"password_min_len\""), "{body}");
    assert!(body.contains("12 characters"), "{body}");

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/authentication/password-min-len")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("_csrf=x&password_min_len=16"))
                .expect("req"),
        )
        .await
        .expect("refused");
    assert!(
        resp.status() == StatusCode::FORBIDDEN
            || resp.status() == StatusCode::UNAUTHORIZED
            || resp.status().is_redirection(),
        "an auditor's POST must not succeed: {}",
        resp.status()
    );
    assert_eq!(stored_minimum(&state).await, 12, "nothing changed");
}

// ── no path enforces a fixed minimum (RFC 151 §4) ────────────────────

/// Every production `.rs` file under `crates/`, except a test file or
/// module (`tests.rs`, a `tests/` directory, or a `#[cfg(test)]`-only
/// fixture file) and this `e2e` test binary itself. Walked, not
/// listed, so a new source file is covered without this test changing.
fn production_sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR has at least two ancestors")
        .join("crates");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name == "tests" || name == "target" {
                continue;
            }
            walk(&path, out);
        } else if name.ends_with(".rs") && name != "tests.rs" {
            out.push(path);
        }
    }
}

/// RFC 151 §4: `SecurityLevel::password_min_len()` is a method call on
/// a *level*, not a setting -- a call anywhere outside the three named
/// exceptions would enforce (or display) a fixed number instead of the
/// one an administrator can raise. Each allowed file's own call count
/// is pinned exactly, not just "present" -- a *second*, stray call
/// inside an already-allowed file is exactly as much a defect as a
/// first one in a new file, and "present" would miss it.
///
/// **Mutation**: add a `.password_min_len()` call in, for example,
/// `crates/sui-id/src/http/handlers/settings.rs` (a real handler, not
/// one of the three exceptions) -- this test fails, naming the file.
#[test]
fn no_path_enforces_a_fixed_minimum() {
    let allowed: &[(&str, usize)] = &[
        ("sui-id-core/src/setup.rs", 1),
        ("sui-id/src/runtime/dev_mode.rs", 1),
        ("sui-id/src/http/handlers.rs", 1),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ancestors")
        .join("crates");

    for path in production_sources() {
        let rel = path
            .strip_prefix(&root)
            .expect("under crates/")
            .to_string_lossy()
            .replace('\\', "/");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // Doc comments (`///`/`//!`) are prose, not a call path --
        // `security.rs`'s own worked example documents the method
        // being scanned for, and a mention elsewhere describing it is
        // the same case, not a caller enforcing anything.
        let found = src
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                !t.starts_with("///") && !t.starts_with("//!")
            })
            .filter(|line| line.contains(".password_min_len()"))
            .count();
        let expected = allowed
            .iter()
            .find(|(name, _)| *name == rel)
            .map(|(_, n)| *n)
            .unwrap_or(0);
        assert_eq!(
            found, expected,
            "{rel}: {found} call(s) to `.password_min_len()`, expected {expected} \
             (allowed only at setup, dev mode's seeding, and \
             `handlers::password_min_len`'s own dev branch)"
        );
    }
}

/// P5 review: setup's first administrator is refused a short password
/// with a sentence that says so, naming the floor, in the page's
/// language. Before this, the typed length refusal fell through to
/// "Setup failed. Please review the form and try again." and told the
/// operator nothing about why.
#[tokio::test]
async fn setup_refuses_a_short_first_password_and_says_why() {
    let state = test_app();
    let short = "a".repeat(11);
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/setup/admin")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "setup_token={SETUP_TOKEN}&username={USERNAME}&password={short}\
                     &confirm_password={short}&display_name=&email="
                )))
                .expect("req"),
        )
        .await
        .expect("setup admin");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = std::str::from_utf8(&bytes).expect("utf8");
    let t = sui_id_i18n::Locale::Ja.strings();
    let expected = (t.password_too_short_flash)(12);
    assert!(
        body.contains(&expected),
        "expected {expected:?}, got: {body}"
    );
    assert!(!body.contains(t.setup_generic_failure), "{body}");
}
