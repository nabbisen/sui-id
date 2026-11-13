//! A `?next=`/`?return_to=`-shaped value must be a same-site path before
//! any sign-in flow redirects to it. One matrix case per entry point
//! named in the inventory (admin/auth.rs's already-signed-in GET, its
//! form POST, its local-MFA pending-cookie write and read, admin/
//! webauthn.rs's pending-cookie read, and federation.rs's `next_path`
//! write read back through the federated-MFA continuation): the response
//! is the site's own default destination, never the supplied value, and
//! never a `500`. A same-site control (`/admin/users`) proves a good
//! value still survives, so a check this broad is not mistaken for one
//! that rejects everything.

use super::common::*;
use super::r096_b1_stage12b::{drive_to_federated_mfa_with_next, get_admin_csrf, post_mfa_code};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

/// The three payloads the dispatch names, plus a tab after the leading
/// slash: a URL parser deletes every ASCII tab and newline before it
/// parses (WHATWG URL Standard, basic URL parser), and a header value may
/// carry a tab, so `/<TAB>/x` is followed as `//x`. Only the
/// `char::is_control` rule refuses it.
const BAD: [&str; 4] = ["//x", "/\\x", "/a\nb", "/\t/x"];
const CONTROL: &str = "/admin/users";

async fn send(state: &AppState, req: Request<Body>) -> axum::http::Response<Body> {
    let resp = build_router(state.clone())
        .oneshot(req)
        .await
        .expect("send");
    assert_ne!(
        resp.status(),
        StatusCode::INTERNAL_SERVER_ERROR,
        "a return-path value must never produce a 500"
    );
    resp
}

fn location(resp: &axum::http::Response<Body>) -> Option<String> {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

// ── site 1: admin/auth.rs -- already signed in, GET /admin/login?next= ──

#[tokio::test]
async fn already_signed_in_get_next_must_be_same_site() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    for bad in BAD {
        let resp = send(
            &state,
            Request::builder()
                .method(Method::GET)
                .uri(format!("/admin/login?next={}", urlencode(bad)))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await;
        assert_eq!(
            location(&resp).as_deref(),
            Some("/admin"),
            "bad next {bad:?} must not be followed"
        );
    }
    let resp = send(
        &state,
        Request::builder()
            .method(Method::GET)
            .uri(format!("/admin/login?next={}", urlencode(CONTROL)))
            .header(header::COOKIE, format!("sui_id_session={session}"))
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    assert_eq!(
        location(&resp).as_deref(),
        Some(CONTROL),
        "a same-site next must still be followed"
    );
}

// ── site 2: admin/auth.rs -- POST /admin/login, form `next` ─────────────

#[tokio::test]
async fn login_post_form_next_must_be_same_site() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    for bad in BAD {
        let resp = send(
            &state,
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={}&password={}&next={}",
                    urlencode(USERNAME),
                    urlencode(PASSWORD),
                    urlencode(bad)
                )))
                .expect("req"),
        )
        .await;
        assert_eq!(
            location(&resp).as_deref(),
            Some("/admin"),
            "bad form next {bad:?} must not be followed"
        );
    }
    let resp = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password={}&next={}",
                urlencode(USERNAME),
                urlencode(PASSWORD),
                urlencode(CONTROL)
            )))
            .expect("req"),
    )
    .await;
    assert_eq!(location(&resp).as_deref(), Some(CONTROL));
}

// ── site 3: admin/auth.rs -- the local-MFA pending-`next` cookie, write ──
// then read, through a genuine password step and TOTP completion ───────

async fn totp_code(secret_b32: &str) -> String {
    let secret = decode_b32(secret_b32);
    let step = chrono::Utc::now().timestamp() / 30;
    format!(
        "{:06}",
        sui_id_core::totp::code_for_step(&secret, step).await
    )
}

/// Enrolment confirms using the step at the moment it runs; signing in
/// again immediately after, in the same 30-second window, would replay
/// that exact step. Rewind it first, the same margin
/// `tests/e2e/r102_stage3.rs`'s own `rewind_step` uses.
async fn rewind_totp_step(state: &AppState) {
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id
        .to_string();
    let step = chrono::Utc::now().timestamp() / 30 - 2;
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE user_totp SET last_used_step = ?1 WHERE user_id = ?2",
                rusqlite::params![step, user],
            )?)
        })
        .await
        .expect("rewind last_used_step");
}

/// Drive a password step with `next`, requiring MFA, and return the
/// pending-MFA cookie plus whatever `sui_id_pending_mfa_next` cookie (if
/// any) the response set.
async fn password_step_to_mfa(state: &AppState, next: &str) -> (String, Option<String>) {
    let resp = send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password={}&next={}",
                urlencode(USERNAME),
                urlencode(PASSWORD),
                urlencode(next)
            )))
            .expect("req"),
    )
    .await;
    assert_eq!(location(&resp).as_deref(), Some("/admin/login/mfa"));
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending cookie");
    let next_cookie = extract_set_cookie(resp.headers(), "sui_id_pending_mfa_next");
    (pending, next_cookie)
}

async fn complete_totp_and_get_location(
    state: &AppState,
    pending: &str,
    next_cookie: Option<&str>,
    secret_b32: &str,
) -> Option<String> {
    let page = send(
        state,
        Request::builder()
            .method(Method::GET)
            .uri("/admin/login/mfa")
            .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
    rewind_totp_step(state).await;
    let code = totp_code(secret_b32).await;
    let mut cookie = format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}");
    if let Some(n) = next_cookie {
        cookie.push_str(&format!("; sui_id_pending_mfa_next={n}"));
    }
    let resp = send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login/mfa")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, cookie)
            .body(Body::from(format!("_csrf={csrf}&code={code}")))
            .expect("req"),
    )
    .await;
    location(&resp)
}

#[tokio::test]
async fn local_mfa_pending_next_cookie_write_rejects_a_bad_value() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (secret_b32, _codes) = enroll_mfa_for(&state, &session).await;
    for bad in BAD {
        let (pending, next_cookie) = password_step_to_mfa(&state, bad).await;
        assert_eq!(
            next_cookie, None,
            "a bad next {bad:?} must never reach the pending-next cookie at write time"
        );
        let loc = complete_totp_and_get_location(&state, &pending, None, &secret_b32).await;
        assert_eq!(
            loc.as_deref(),
            Some("/admin"),
            "with no stored next, completion lands on the default"
        );
    }
    let (pending, next_cookie) = password_step_to_mfa(&state, CONTROL).await;
    assert!(
        next_cookie.is_some(),
        "a good next must still be stored (not rejected)"
    );
    let loc =
        complete_totp_and_get_location(&state, &pending, next_cookie.as_deref(), &secret_b32).await;
    assert_eq!(
        loc.as_deref(),
        Some(CONTROL),
        "and decoded back to the original path at read time"
    );
}

/// Defence in depth, per the dispatch: a value already stored before this
/// fix (or stored some other way) must still be refused when it is read
/// back, not only when it is written. Sets the pending-next cookie
/// directly, bypassing the write-site check entirely.
#[tokio::test]
async fn local_mfa_pending_next_cookie_read_rejects_a_bad_value_even_if_already_stored() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (secret_b32, _codes) = enroll_mfa_for(&state, &session).await;
    for bad in BAD {
        let (pending, _) = password_step_to_mfa(&state, "").await;
        let loc =
            complete_totp_and_get_location(&state, &pending, Some(&urlencode(bad)), &secret_b32)
                .await;
        assert_eq!(
            loc.as_deref(),
            Some("/admin"),
            "a bad stored next {bad:?} must not be followed at read time"
        );
    }
}

// ── site 4: admin/webauthn.rs -- the same pending-`next` cookie, read ───
// by the passkey second-factor step. No real WebAuthn ceremony is
// fabricated here (this codebase's own test suite has no such fixture
// anywhere); the filter under test runs once, before the ceremony result
// is known, and gates both the success redirect and the failure one's
// `?next=` query value identically, so observing the failure redirect's
// `next` parameter proves the same filter without needing a successful
// assertion. Named here rather than silently assumed.

#[tokio::test]
async fn webauthn_pending_next_cookie_read_rejects_a_bad_value() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    enroll_mfa_for(&state, &session).await;
    for bad in BAD {
        let (pending, _) = password_step_to_mfa(&state, "").await;
        let page = send(
            &state,
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await;
        let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
        let resp = send(
            &state,
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login/webauthn/complete")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!(
                        "sui_id_pending_mfa={pending}; sui_id_csrf={csrf}; \
                         sui_id_pending_mfa_next={}",
                        urlencode(bad)
                    ),
                )
                .body(Body::from(format!("_csrf={csrf}&credential={{}}")))
                .expect("req"),
        )
        .await;
        assert_eq!(
            location(&resp).as_deref(),
            Some("/admin/login"),
            "a bad stored next {bad:?} must not even survive into the \
             failure redirect's own `?next=` value"
        );
    }
    // Control: a good stored value does survive into the failure redirect
    // (re-encoded, which is always safe) -- proving the filter accepts,
    // not only rejects.
    let (pending, _) = password_step_to_mfa(&state, "").await;
    let page = send(
        &state,
        Request::builder()
            .method(Method::GET)
            .uri("/admin/login/mfa")
            .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
            .body(Body::empty())
            .expect("req"),
    )
    .await;
    let csrf = extract_set_cookie(page.headers(), "sui_id_csrf").expect("csrf");
    let resp = send(
        &state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login/webauthn/complete")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!(
                    "sui_id_pending_mfa={pending}; sui_id_csrf={csrf}; \
                     sui_id_pending_mfa_next={}",
                    urlencode(CONTROL)
                ),
            )
            .body(Body::from(format!("_csrf={csrf}&credential={{}}")))
            .expect("req"),
    )
    .await;
    let expected = format!("/admin/login?next={}", urlencode(CONTROL));
    assert_eq!(location(&resp), Some(expected));
}

// ── sites 5/6: federation.rs's `next_path` write, read back through the ─
// federated-MFA continuation (admin/auth.rs's `find_fed_pending_row`) ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federated_next_path_write_and_read_reject_a_bad_value() {
    for bad in BAD {
        let mut state = test_app();
        complete_setup_and_login(&state).await;
        let setup = drive_to_federated_mfa_with_next(&mut state, bad).await;
        let admin_csrf = get_admin_csrf(&state, &setup).await;
        let code = totp_code_from_secret(&setup.totp_secret).await;
        let resp = post_mfa_code(&state, &setup, &code, &admin_csrf).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok());
        // Release prep §A: `drive_to_federated_mfa_with_next`'s user is a
        // plain `Role::User`, so the rejected-bad-value default is
        // `/me/security`, not `/admin`.
        assert_eq!(
            loc,
            Some("/me/security"),
            "bad next_path {bad:?} must not be written, let alone followed"
        );
    }
}

/// Isolates the write site from the read site: the MFA continuation's own
/// read-side check (admin/auth.rs's `find_fed_pending_row` filter) would
/// mask a broken write alone, so this queries the stored row directly
/// rather than relying on the final redirect.
async fn stored_next_path(state: &AppState) -> Option<String> {
    state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT next_path FROM federation_login_attempt LIMIT 1",
                [],
                |r| r.get::<_, Option<String>>(0),
            )?)
        })
        .await
        .expect("next_path")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federated_next_path_write_site_stores_nothing_for_a_bad_value() {
    for bad in BAD {
        let mut state = test_app();
        complete_setup_and_login(&state).await;
        drive_to_federated_mfa_with_next(&mut state, bad).await;
        assert_eq!(
            stored_next_path(&state).await,
            None,
            "bad next {bad:?} must not even be written to federation_login_attempt.next_path"
        );
    }
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    drive_to_federated_mfa_with_next(&mut state, CONTROL).await;
    assert_eq!(
        stored_next_path(&state).await,
        Some(CONTROL.to_owned()),
        "a good next must still be written verbatim"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federated_next_path_follows_a_same_site_control() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa_with_next(&mut state, CONTROL).await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;
    let code = totp_code_from_secret(&setup.totp_secret).await;
    let resp = post_mfa_code(&state, &setup, &code, &admin_csrf).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok());
    assert_eq!(loc, Some(CONTROL));
}

/// Release prep §A: the second-factor completion path, genuinely no
/// `next` at all (not a bad value falling back to the same default,
/// which the two tests above already cover) -- a plain `Role::User`
/// lands on `/me/security`, not `/admin`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federated_second_factor_completion_non_admin_with_no_next_lands_on_me_security() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa_with_next(&mut state, "").await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;
    let code = totp_code_from_secret(&setup.totp_secret).await;
    let resp = post_mfa_code(&state, &setup, &code, &admin_csrf).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let loc = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok());
    assert_eq!(loc, Some("/me/security"));
}

async fn totp_code_from_secret(secret: &[u8]) -> String {
    let step = chrono::Utc::now().timestamp() / 30;
    format!(
        "{:06}",
        sui_id_core::totp::code_for_step(secret, step).await
    )
}

/// Defence in depth, per the dispatch: a `federation_mfa_pending` row
/// written *before* this fix can still hold a bad value until it expires
/// (the MFA continuation reads this table, not `federation_login_attempt`
/// directly -- the attempt's own `next_path` is copied into it once the
/// upstream callback completes). Drives a legitimate attempt (so the
/// write site has nothing to object to), then corrupts the stored
/// `next_path` directly -- simulating a pre-fix row -- and proves the
/// read site still refuses it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federated_next_path_read_site_rejects_an_already_stored_bad_value() {
    for bad in BAD {
        let mut state = test_app();
        complete_setup_and_login(&state).await;
        let setup = drive_to_federated_mfa_with_next(&mut state, "/admin/clients").await;
        let bad = bad.to_owned();
        state
            .db
            .with_conn(move |c| {
                Ok(c.execute(
                    "UPDATE federation_mfa_pending SET next_path = ?1",
                    rusqlite::params![bad],
                )?)
            })
            .await
            .expect("corrupt next_path");
        let admin_csrf = get_admin_csrf(&state, &setup).await;
        let code = totp_code_from_secret(&setup.totp_secret).await;
        let resp = post_mfa_code(&state, &setup, &code, &admin_csrf).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let loc = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok());
        // Release prep §A: `drive_to_federated_mfa_with_next`'s user is a
        // plain `Role::User`.
        assert_eq!(
            loc,
            Some("/me/security"),
            "an already-stored bad next_path must not be followed at read time"
        );
    }
}
