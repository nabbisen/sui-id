//! RFC 149 Q3b (`0.95.0-q3b-the-breach-check-can-be-changed`), §4: the
//! password-breach check's own control on Sign-in settings — direction
//! decided on the server, a weakening gated by confirmation and a
//! fresh step-up, the audit record, and the dashboard link. Tests 1-7
//! of that dispatch's §4 table live here.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use sui_id_store::models::HibpMode;
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

async fn post_form(
    state: &AppState,
    uri: &str,
    session: &str,
    csrf: &str,
    body: &str,
) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("POST")
}

async fn html_of(resp: axum::response::Response) -> String {
    let body = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&body).to_string()
}

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

/// `UPDATE users SET role = 'auditor'` directly -- the established
/// shape (`r096_b2_stage5_admin_surface.rs`, `q3_help_and_dashboard.rs`,
/// ...).
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

/// Seed `server_settings.hibp_mode` directly, bypassing the audited
/// command -- test setup, not a thing the product does (same
/// reasoning `dev_mode.rs`'s own direct write now uses in production,
/// but here purely to arrange a starting mode without an audit row of
/// its own skewing the counts below).
async fn seed_hibp_mode(state: &AppState, mode: HibpMode) {
    sui_id_store::repos::server_settings::update_hibp_mode(&state.db, mode, chrono::Utc::now())
        .await
        .expect("seed hibp_mode");
}

async fn stored_hibp_mode(state: &AppState) -> HibpMode {
    sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings")
        .hibp_mode
}

/// Count of `settings.hibp_mode.changed` audit rows.
async fn s02_event_count(state: &AppState) -> i64 {
    state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'settings.hibp_mode.changed'",
                [],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .expect("count S02 events")
}

/// Enrol TOTP directly via the store layer (same shape
/// `step_up_totp.rs` uses) and complete a fresh step-up through the
/// real `/me/security/step-up` GET+POST, landing back on `return_to`.
async fn complete_fresh_step_up(state: &AppState, session: &str, return_to: &str) {
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin exists");
    let secret = b"q3b-step-up-secret\x00\x00\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, secret)
        .await
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .await
        .expect("confirm totp");

    let encoded_return_to = utf8_encode(return_to);
    let resp = get(
        state,
        &format!("/me/security/step-up?return_to={encoded_return_to}"),
        session,
    )
    .await;
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf cookie");

    let now = chrono::Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;
    let body = format!("_csrf={csrf}&code={code}&return_to={encoded_return_to}");
    let resp = post_form(state, "/me/security/step-up", session, &csrf, &body).await;
    assert!(
        resp.status().is_redirection(),
        "step-up POST must redirect, got {}",
        resp.status()
    );
}

// ---------- test 1: strengthening saves directly, no step-up ----------

#[tokio::test]
async fn strengthening_saves_directly_with_no_step_up() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    seed_hibp_mode(&state, HibpMode::Off).await;
    let before = s02_event_count(&state).await;

    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode",
        &session,
        &csrf,
        &format!("_csrf={csrf}&hibp_mode=block"),
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "expected a redirect, got {}",
        resp.status()
    );
    assert_eq!(
        location(&resp),
        "/admin/settings/authentication",
        "strengthening must save directly, not detour through the confirm screen"
    );
    assert_eq!(stored_hibp_mode(&state).await, HibpMode::Block);
    assert_eq!(
        s02_event_count(&state).await,
        before + 1,
        "exactly one S02 record"
    );
}

// ---------- test 2: weakening with no fresh step-up is not saved ----------

#[tokio::test]
async fn weakening_with_no_fresh_step_up_is_not_saved() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    seed_hibp_mode(&state, HibpMode::Block).await;

    // The admin has MFA enrolled (step 3's helper does this too, but
    // this test needs the *absence* of a fresh step-up specifically,
    // so enrol directly without completing one).
    let user = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin exists");
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user.id, b"unused-secret-1234567")
        .await
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user.id, b"[]")
        .await
        .expect("confirm totp");

    let csrf = fetch_csrf(&state, &session).await;
    let confirm_resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode",
        &session,
        &csrf,
        &format!("_csrf={csrf}&hibp_mode=off"),
    )
    .await;
    assert!(confirm_resp.status().is_redirection());
    let confirm_url = location(&confirm_resp);
    assert!(confirm_url.contains("hibp-mode-confirm"));

    let csrf2 = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode-confirm",
        &session,
        &csrf2,
        &format!("_csrf={csrf2}&confirmed=1&mode=off"),
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "no fresh step-up must redirect to the step-up challenge, got {}",
        resp.status()
    );
    assert!(
        location(&resp).contains("/me/security/step-up"),
        "expected the step-up challenge, got {}",
        location(&resp)
    );
    assert_eq!(
        stored_hibp_mode(&state).await,
        HibpMode::Block,
        "must not be saved without a fresh step-up"
    );
}

// ---------- test 3: weakening after confirmation and a fresh step-up ----------

#[tokio::test]
async fn weakening_after_confirmation_and_fresh_step_up_saves_with_the_record() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    seed_hibp_mode(&state, HibpMode::Block).await;
    let before = s02_event_count(&state).await;

    complete_fresh_step_up(
        &state,
        &session,
        "/admin/settings/authentication/hibp-mode-confirm?mode=off",
    )
    .await;

    // Without `confirmed=1`, even with a fresh step-up, nothing is
    // saved — the confirmation itself is a separate gate from step-up.
    let csrf0 = fetch_csrf(&state, &session).await;
    let unconfirmed = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode-confirm",
        &session,
        &csrf0,
        &format!("_csrf={csrf0}&mode=off"),
    )
    .await;
    assert!(
        unconfirmed.status().is_client_error(),
        "skipping the confirmation must be refused, got {}",
        unconfirmed.status()
    );
    assert_eq!(stored_hibp_mode(&state).await, HibpMode::Block);

    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode-confirm",
        &session,
        &csrf,
        &format!("_csrf={csrf}&confirmed=1&mode=off"),
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "expected a redirect after saving, got {}",
        resp.status()
    );
    assert_eq!(stored_hibp_mode(&state).await, HibpMode::Off);
    assert_eq!(
        s02_event_count(&state).await,
        before + 1,
        "exactly one S02 record, old=block new=off"
    );

    let old_new = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'settings.hibp_mode.changed' \
                 ORDER BY at DESC LIMIT 1",
                [],
                |row| row.get::<_, Option<String>>(0),
            )?)
        })
        .await
        .expect("read note")
        .expect("note is populated");
    assert!(old_new.contains("old=block"), "{old_new}");
    assert!(old_new.contains("new=off"), "{old_new}");
}

// ---------- test 4: the direction is the server's ----------

#[tokio::test]
async fn direction_is_the_servers_not_the_forms() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    seed_hibp_mode(&state, HibpMode::Block).await;

    // Straight to the save route with a weakening value -- it must
    // redirect to the confirm screen, never save directly, no matter
    // what the request otherwise looks like.
    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode",
        &session,
        &csrf,
        &format!("_csrf={csrf}&hibp_mode=warn"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert!(
        location(&resp).contains("hibp-mode-confirm"),
        "a weakening POST straight to the save route must not be saved there"
    );
    assert_eq!(
        stored_hibp_mode(&state).await,
        HibpMode::Block,
        "must still be the old mode"
    );
}

// ---------- test 5: an auditor ----------

#[tokio::test]
async fn auditor_sees_the_mode_and_no_form_and_the_post_is_refused() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    seed_hibp_mode(&state, HibpMode::Block).await;
    demote_to_auditor(&state, admin.id).await;

    let html = html_of(get(&state, "/admin/settings/authentication", &session).await).await;
    assert!(
        !html.contains(r#"name="hibp_mode""#),
        "an auditor must see no form"
    );
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        html.contains(ja.settings_auth_hibp_block_label),
        "an auditor must still see the current mode"
    );

    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode",
        &session,
        &csrf,
        &format!("_csrf={csrf}&hibp_mode=off"),
    )
    .await;
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::FORBIDDEN,
        "an auditor's POST must be refused"
    );
}

// ---------- test 6: saving the stored mode again ----------

#[tokio::test]
async fn saving_the_stored_mode_again_writes_no_audit_record() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    seed_hibp_mode(&state, HibpMode::Block).await;
    let before = s02_event_count(&state).await;

    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/admin/settings/authentication/hibp-mode",
        &session,
        &csrf,
        &format!("_csrf={csrf}&hibp_mode=block"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert_eq!(s02_event_count(&state).await, before, "no new record");
}

// ---------- test 7: the dashboard link ----------

#[tokio::test]
async fn dashboard_names_sign_in_settings_when_off_and_says_nothing_when_warn() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    seed_hibp_mode(&state, HibpMode::Off).await;
    let html = html_of(get(&state, "/admin", &session).await).await;
    let ja = sui_id_i18n::Locale::Ja.strings();
    let warn_pos = html
        .find(ja.dashboard_warn_hibp)
        .unwrap_or_else(|| panic!("warning text not found: {html}"));
    // The link sits right after the warning text, inside the same
    // `<li>` -- a short window, not a page-wide substring search
    // (the sidebar nav links to the same URL regardless).
    let after = &html[warn_pos..(warn_pos + 400).min(html.len())];
    assert!(
        after.contains("/admin/settings/authentication") && after.contains("</a>"),
        "the warning must be followed by a link to Sign-in settings: {after}"
    );

    seed_hibp_mode(&state, HibpMode::Warn).await;
    let html = html_of(get(&state, "/admin", &session).await).await;
    assert!(
        !html.contains(ja.dashboard_warn_hibp),
        "no warning once the check is on"
    );
}
