//! RFC 096-B1 stage 12b — the federated MFA submission gets a caller.
//! Drives a federated sign-in to the MFA-required point, enrols TOTP for
//! the federated user directly (it has no password, so the self-service
//! enrolment flow does not apply), and submits against the real
//! `/admin/login/mfa` route.

use super::common::*;
use super::federation_fail_closed::{
    add_federation_provider_config, federated_signin_with_next, linked_user, mock_upstream,
};
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

/// Enrol an already-confirmed TOTP factor for `user_id` directly at the
/// store layer -- the federated user has no password, so the
/// self-service enrolment flow (`/me/security/mfa/enroll/*`) does not
/// apply to it.
async fn enroll_totp_directly(state: &AppState, user_id: sui_id_shared::ids::UserId) -> Vec<u8> {
    let secret = b"0123456789ABCDEF".to_vec();
    sui_id_store::repos::user_totp::upsert_pending(&state.db, user_id, &secret)
        .await
        .expect("upsert_pending");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, user_id, b"[]")
        .await
        .expect("confirm");
    secret
}

fn mfa_failure_count(
    state: &AppState,
    user_id: sui_id_shared::ids::UserId,
) -> impl std::future::Future<Output = i64> + '_ {
    let uid = user_id.to_string();
    async move {
        state
            .db
            .with_read(move |c| {
                Ok(
                    c.prepare("SELECT mfa_failure_count FROM users WHERE id = ?1")?
                        .query_row([uid], |r| r.get::<_, i64>(0))?,
                )
            })
            .await
            .expect("mfa_failure_count")
    }
}

/// RFC 096-B1 stage 12b fix round, item 1: how many `login_pending_mfa`
/// rows exist for a user -- the generic row stage 9's dual-issue writes,
/// separate from the federated continuation and its own table.
fn login_pending_mfa_count(
    state: &AppState,
    user_id: sui_id_shared::ids::UserId,
) -> impl std::future::Future<Output = i64> + '_ {
    let uid = user_id.to_string();
    async move {
        state
            .db
            .with_read(move |c| {
                Ok(
                    c.prepare("SELECT COUNT(*) FROM login_pending_mfa WHERE user_id = ?1")?
                        .query_row([uid], |r| r.get::<_, i64>(0))?,
                )
            })
            .await
            .expect("login_pending_mfa_count")
    }
}

struct FedMfaSetup {
    fed_pending: String,
    fed_csrf: String,
    fed_browser_binding: String,
    /// The generic `sui_id_pending_mfa` cookie, dual-issued alongside the
    /// federated one (stage 9) -- a real browser would hold and resend
    /// this on the same POST, which is what lets the fix round's own
    /// item 1 (consuming the row it names) ever run at all.
    pending_mfa_cookie: String,
    user_id: sui_id_shared::ids::UserId,
    totp_secret: Vec<u8>,
}

async fn drive_to_federated_mfa(state: &mut AppState) -> FedMfaSetup {
    drive_to_federated_mfa_with_next(state, "").await
}

/// Same as [`drive_to_federated_mfa`], but starts the upstream sign-in
/// with `?next=<next>` (empty means no `next` at all) -- RFC 096-B1
/// stage 12b fix round, item 2.
async fn drive_to_federated_mfa_with_next(state: &mut AppState, next: &str) -> FedMfaSetup {
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, slot) = mock_upstream().await;
    add_federation_provider_config(state, &issuer);
    let user_id = linked_user(state, &issuer).await;
    let totp_secret = enroll_totp_directly(state, user_id).await;

    let o = federated_signin_with_next(state, &slot, next).await;
    assert_eq!(o.location, "/admin/login/mfa", "MFA must be required");
    let fed_pending =
        extract_set_cookie(&o.headers, "sui_id_fed_mfa_pending").expect("fed pending cookie");
    let fed_csrf = extract_set_cookie(&o.headers, "sui_id_fed_mfa_csrf").expect("fed csrf cookie");
    let fed_browser_binding = extract_set_cookie(&o.headers, "sui_id_fed_mfa_browser_binding")
        .expect("fed browser binding cookie");
    let pending_mfa_cookie = o
        .pending_mfa_cookie
        .clone()
        .expect("the generic pending-mfa cookie is dual-issued alongside the federated one");

    FedMfaSetup {
        fed_pending,
        fed_csrf,
        fed_browser_binding,
        pending_mfa_cookie,
        user_id,
        totp_secret,
    }
}

async fn post_mfa_code(
    state: &AppState,
    setup: &FedMfaSetup,
    code: &str,
    admin_csrf: &str,
) -> axum::http::Response<Body> {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login/mfa")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!(
                        "sui_id_fed_mfa_pending={}; sui_id_fed_mfa_csrf={}; \
                         sui_id_fed_mfa_browser_binding={}; sui_id_pending_mfa={}; \
                         sui_id_csrf={admin_csrf}",
                        setup.fed_pending,
                        setup.fed_csrf,
                        setup.fed_browser_binding,
                        setup.pending_mfa_cookie
                    ),
                )
                .body(Body::from(format!(
                    "code={code}&_csrf={admin_csrf}&fed_csrf={}",
                    setup.fed_csrf
                )))
                .expect("req"),
        )
        .await
        .expect("mfa POST")
}

async fn get_admin_csrf(state: &AppState, setup: &FedMfaSetup) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login/mfa")
                .header(
                    header::COOKIE,
                    format!(
                        "sui_id_fed_mfa_pending={}; sui_id_fed_mfa_csrf={}",
                        setup.fed_pending, setup.fed_csrf
                    ),
                )
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("mfa GET");
    extract_set_cookie(resp.headers(), "sui_id_csrf").expect("admin csrf cookie")
}

/// RFC 096-B1 stage 12b §3: the account lock. A wrong federated second
/// factor must advance `users.mfa_failure_count` -- the same counter a
/// wrong *local* one does -- not merely fail the request. Asserted as a
/// before/after value, not only the response, per the dispatch's own
/// instruction: "a test that only checks the response cannot tell the
/// hook's absence from its presence."
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_federated_second_factor_advances_the_same_lock_counter_a_wrong_local_one_does() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa(&mut state).await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;

    let before = mfa_failure_count(&state, setup.user_id).await;
    assert_eq!(before, 0, "no failure recorded yet");

    let resp = post_mfa_code(&state, &setup, "000000", &admin_csrf).await;
    assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);

    let after = mfa_failure_count(&state, setup.user_id).await;
    assert_eq!(
        after, 1,
        "a wrong federated code must advance users.mfa_failure_count exactly \
         the way a wrong local one does (before={before}, after={after})"
    );
}

/// The direct contrast: the *correct* code promotes the continuation,
/// opens a session, and clears all four federated cookies plus the
/// generic pending-MFA pair.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_correct_federated_totp_code_promotes_and_clears_every_pending_cookie() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa(&mut state).await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;

    let step = chrono::Utc::now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&setup.totp_secret, step).await;

    let resp = post_mfa_code(&state, &setup, &format!("{code:06}"), &admin_csrf).await;
    assert_eq!(resp.status(), axum::http::StatusCode::SEE_OTHER);
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_session").is_some(),
        "the federated second factor must open a real session"
    );
    for name in [
        "sui_id_fed_mfa_pending",
        "sui_id_fed_mfa_csrf",
        "sui_id_fed_mfa_browser_binding",
        "sui_id_pending_mfa",
    ] {
        let set_cookie = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|s| s.starts_with(&format!("{name}=")))
            .unwrap_or_else(|| panic!("no Set-Cookie for {name}"));
        assert!(
            set_cookie.contains("Max-Age=0"),
            "{name} must be cleared on promotion: {set_cookie}"
        );
    }
}

/// RFC 096-B1 stage 12b §5: direction-2 `MethodMismatch` (the row is
/// bound to WebAuthn via a live ceremony; a code arrives instead) must
/// render the specific "waiting for your passkey" copy, not the generic
/// "try again, or use a recovery code" flash -- which would be actively
/// wrong here, since neither remedy can succeed. `promote`/`verify`
/// (the only entry points `mfa_challenge_post` calls) can only ever
/// produce `MethodMismatch` in this one direction, since this stage does
/// not wire `verify_webauthn`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direction_2_method_mismatch_renders_the_passkey_waiting_copy() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa(&mut state).await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;

    // `begin_federation_webauthn_ceremony` itself requires the user to
    // already hold at least one passkey (it would make no sense to open
    // a ceremony for a user who cannot complete it) -- seed a fake
    // credential row directly, same as `step_up_webauthn.rs` does; only
    // its existence matters, never its content.
    let cred_row = sui_id_store::models::UserWebauthnCredentialRow {
        id: sui_id_shared::ids::WebauthnCredentialId::new(),
        user_id: setup.user_id,
        credential_id: vec![9, 9, 9, 9],
        passkey_enc: vec![],
        nickname: "Test Key".into(),
        created_at: chrono::Utc::now(),
        last_used_at: None,
    };
    sui_id_store::repos::user_webauthn_credentials::create(&state.db, &cred_row, b"{}")
        .await
        .expect("seed a passkey for the user");

    // Bind the row to WebAuthn directly at the store layer -- this stage
    // does not wire `/admin/login/mfa`'s own submission through
    // `verify_webauthn`, so the only way to reach direction 2 in a test
    // is the same way F06 itself would: a live
    // `federation_webauthn_ceremony` row for this pending id. The sealed
    // state's own content is never opened by this test's own code path
    // (`verify`'s own `MethodMismatch` check only asks whether a row
    // exists at all), so it is a placeholder, not a real ceremony.
    let pending_id: sui_id_shared::ids::FederationMfaPendingId = setup
        .fed_pending
        .parse()
        .expect("fed pending cookie value parses");
    sui_id_store::commands::begin_federation_webauthn_ceremony(
        &state.db,
        pending_id,
        vec![0u8; 16],
        "idp.test".into(),
        "https://idp.test".into(),
        chrono::Utc::now(),
    )
    .await
    .expect("seed a live webauthn ceremony");

    // Pin the locale to English: the test environment's own default
    // (no cookie, no Accept-Language) is Japanese, as every other
    // i18n-aware e2e test here already accounts for.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login/mfa")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!(
                        "sui_id_lang=en; sui_id_fed_mfa_pending={}; sui_id_fed_mfa_csrf={}; \
                         sui_id_fed_mfa_browser_binding={}; sui_id_csrf={admin_csrf}",
                        setup.fed_pending, setup.fed_csrf, setup.fed_browser_binding
                    ),
                )
                .body(Body::from(format!(
                    "code=000000&_csrf={admin_csrf}&fed_csrf={}",
                    setup.fed_csrf
                )))
                .expect("req"),
        )
        .await
        .expect("mfa POST");
    assert_eq!(resp.status(), axum::http::StatusCode::UNAUTHORIZED);
    let body = read_body(resp.into_body()).await;
    let text = std::str::from_utf8(&body).expect("utf8");
    assert!(
        text.contains("waiting for your passkey"),
        "must render the method-mismatch copy, not the generic failed flash: {text}"
    );
    assert!(
        !text.contains("use a recovery code"),
        "must not render the generic flash's actively-wrong remedy: {text}"
    );
}

/// RFC 096-B1 stage 12b fix round, item 1: a successful federated
/// promotion must consume the generic `login_pending_mfa` row dual-issued
/// alongside the federated one (stage 9) -- not merely clear its cookie,
/// which the existing cookie-clearing test already passes regardless of
/// whether the row itself survives. Asserted as a row count, not the
/// `Set-Cookie` header.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_correct_federated_totp_code_consumes_the_generic_pending_mfa_row() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa(&mut state).await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;

    let before = login_pending_mfa_count(&state, setup.user_id).await;
    assert_eq!(
        before, 1,
        "stage 9's dual-issue must have written exactly one generic row"
    );

    let step = chrono::Utc::now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&setup.totp_secret, step).await;
    let resp = post_mfa_code(&state, &setup, &format!("{code:06}"), &admin_csrf).await;
    assert_eq!(resp.status(), axum::http::StatusCode::SEE_OTHER);

    let after = login_pending_mfa_count(&state, setup.user_id).await;
    assert_eq!(
        after, 0,
        "a successful federated promotion must consume the generic row \
         (before={before}, after={after})"
    );
}

/// RFC 096-B1 stage 12b fix round, item 2: the `Promoted` arm's redirect
/// target is the pending row's own `next_path`, carried from the upstream
/// sign-in's own `?next=` -- not a hardcoded `/admin`. No existing test
/// read `Location` at all; replacing the lookup with the literal `"/admin"`
/// would have stayed green.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_correct_federated_totp_code_redirects_to_the_row_s_own_next_path() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    let setup = drive_to_federated_mfa_with_next(&mut state, "/admin/clients").await;
    let admin_csrf = get_admin_csrf(&state, &setup).await;

    let step = chrono::Utc::now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(&setup.totp_secret, step).await;
    let resp = post_mfa_code(&state, &setup, &format!("{code:06}"), &admin_csrf).await;
    assert_eq!(resp.status(), axum::http::StatusCode::SEE_OTHER);

    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        location, "/admin/clients",
        "must redirect to the row's own next_path, not a hardcoded default"
    );
}
