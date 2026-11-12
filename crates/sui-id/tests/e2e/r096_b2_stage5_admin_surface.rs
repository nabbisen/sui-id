//! RFC 096-B2 stage 5 — the federation-provider admin surface, through
//! the real HTTP router (`/admin/federation-providers`, its two confirm
//! pages, and their POSTs).
//!
//! Setup is shared with stage 4's own e2e test
//! (`r096_b2_stage4_c17_enable_disable.rs`): the same mock upstream
//! (`federation_fail_closed::mock_upstream`), the same confidential-
//! provider shape (`TEST_CLIENT_SECRET`), the same `SUB`. This file
//! drives the identical scenario through the admin pages this stage
//! built, rather than calling `check_and_enable`/
//! `disable_federation_provider` directly.

use super::common::*;
use super::federation_fail_closed::{TEST_CLIENT_SECRET, add_federation_provider_config};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{FederationProviderId, UserId};
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

const SUB: &str = "fed-sub-1";

fn disabled_row(issuer: &str, slug: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Up".into(),
        issuer: issuer.into(),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: Some("RS256".into()),
    }
}

async fn seed_disabled_configured_provider(state: &AppState, issuer: &str) -> FederationProviderId {
    let row = disabled_row(issuer, "up");
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, Some(TEST_CLIENT_SECRET))
        .await
        .expect("seed disabled provider");
    provider_id
}

async fn link_user(state: &AppState, provider_id: FederationProviderId) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    sui_id_store::repos::users::create(
        &state.db,
        &sui_id_store::models::UserRow {
            id: uid,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: sui_id_store::models::Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: sui_id_store::models::UserSource::Local,
            external_stable_id: None,
            email: None,
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "INSERT INTO federation_link \
                 (user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                rusqlite::params![
                    uid.to_string(),
                    provider_id.to_string(),
                    SUB,
                    now.to_rfc3339()
                ],
            )?)
        })
        .await
        .expect("link user to provider");
    uid
}

/// Demotes the session's own user to `Role::Auditor` directly (no HTTP
/// path creates an auditor in this test suite yet) -- `CurrentAdminOrAuditor`
/// re-reads the user row from the database on every request, so the same
/// session cookie immediately starts behaving as an auditor's.
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

/// `resolve_admin_locale`'s own order is `preferred_lang` first, then
/// `server_settings.default_lang`, then a hardcoded `Locale::Ja`
/// fallback -- a freshly set-up admin has none set, so every admin
/// page renders Japanese by default. Tests here assert on the English
/// text (already pinned, in both locales, by the lib-level tests in
/// `handlers/admin/federation_providers.rs`), so every test that reads
/// rendered text sets this first.
async fn log_in_as_admin_in_english(state: &AppState) -> (String, UserId) {
    let session = complete_setup_and_login(state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE users SET preferred_lang = 'en' WHERE id = ?1",
                [admin.id.to_string()],
            )?)
        })
        .await
        .expect("set preferred_lang");
    (session, admin.id)
}

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

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

async fn with_federation_client_and_mock(
    state: &mut AppState,
) -> (String, super::federation_fail_closed::NonceSlot) {
    state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
        super::tls_mock::LoopbackFixtureResolver,
    )));
    super::federation_fail_closed::mock_upstream().await
}

// ── The list page ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_shows_state_review_status_last_checked_and_linked_users() {
    let mut state = test_app();
    let (session, _admin_id) = log_in_as_admin_in_english(&state).await;
    let (issuer, _slot) = with_federation_client_and_mock(&mut state).await;
    add_federation_provider_config(&mut state, &issuer);
    let provider_id = seed_disabled_configured_provider(&state, &issuer).await;
    link_user(&state, provider_id).await;

    let resp = get(&state, "/admin/federation-providers", &session).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        body.contains("Up"),
        "does not show the display name: {body}"
    );
    assert!(body.contains("Disabled"), "does not show state: {body}");
    assert!(
        body.contains("Requires review"),
        "does not show review status: {body}"
    );
    // S5 review: the column is the check behind the *current* enablement
    // (`evidence_observed_at`, cleared on disable), so a disabled provider
    // shows a dash, never "Never", which would be false for one that was
    // checked and enabled before.
    assert!(
        body.contains("Enabled after a check at"),
        "does not show the check column: {body}"
    );
    assert!(
        body.contains(r#"<td class="muted">—</td>"#),
        "no dash cell for a provider with no current check: {body}"
    );
    assert!(
        body.contains(&format!(
            "/admin/federation-providers/{provider_id}/enable-confirm"
        )),
        "does not offer an enable action: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unconfigured_disabled_provider_offers_no_enable_action() {
    let state = test_app();
    let (session, _admin_id) = log_in_as_admin_in_english(&state).await;
    // Deliberately not `add_federation_provider_config` -- the C25 case.
    let provider_id =
        seed_disabled_configured_provider(&state, "https://unconfigured.example").await;

    let resp = get(&state, "/admin/federation-providers", &session).await;
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        !body.contains(&format!(
            "/admin/federation-providers/{provider_id}/enable-confirm"
        )),
        "must not offer Enable for an unconfigured provider: {body}"
    );
    assert!(
        body.contains("No configuration block names this provider"),
        "does not say what to do instead: {body}"
    );
}

// ── Auditor read-only ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auditor_sees_the_list_with_no_action_links_and_gets_403_everywhere_else() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    let provider_id =
        seed_disabled_configured_provider(&state, "https://auditor-test.example").await;
    demote_to_auditor(&state, admin.id).await;

    let list = get(&state, "/admin/federation-providers", &session).await;
    assert_eq!(
        list.status(),
        StatusCode::OK,
        "auditor can still see the list"
    );
    let body = String::from_utf8_lossy(&read_body(list.into_body()).await).into_owned();
    assert!(
        !body.contains("enable-confirm") && !body.contains("disable-confirm"),
        "an auditor must see no action link: {body}"
    );

    for uri in [
        format!("/admin/federation-providers/{provider_id}/enable-confirm"),
        format!("/admin/federation-providers/{provider_id}/disable-confirm"),
    ] {
        let resp = get(&state, &uri, &session).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "GET {uri}");
    }

    let csrf = {
        let resp = get(&state, "/admin/federation-providers", &session).await;
        extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf")
    };
    for uri in [
        format!("/admin/federation-providers/{provider_id}/enable"),
        format!("/admin/federation-providers/{provider_id}/check"),
        format!("/admin/federation-providers/{provider_id}/disable"),
    ] {
        let resp = post_form(
            &state,
            &uri,
            &session,
            &csrf,
            &format!("_csrf={csrf}&_confirmed=1"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "POST {uri}");
    }
}

// ── CSRF ─────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_post_without_a_csrf_token_is_refused() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let provider_id = seed_disabled_configured_provider(&state, "https://csrf-test.example").await;

    for uri in [
        format!("/admin/federation-providers/{provider_id}/enable"),
        format!("/admin/federation-providers/{provider_id}/check"),
        format!("/admin/federation-providers/{provider_id}/disable"),
    ] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(&uri)
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .header(header::COOKIE, format!("sui_id_session={session}"))
                    .body(Body::from("_confirmed=1"))
                    .expect("req"),
            )
            .await
            .expect("POST");
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "POST {uri} with no CSRF token"
        );
    }
}

// ── Step-up: redirect, return to the confirm page, no auto-execute ───────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enable_confirm_without_fresh_step_up_redirects_and_returns_without_executing() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    let secret = b"fed-admin-step-up-secret\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, admin.id, secret)
        .await
        .expect("pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, admin.id, b"[]")
        .await
        .expect("confirm totp");
    let provider_id =
        seed_disabled_configured_provider(&state, "https://step-up-enable.example").await;

    let confirm_uri = format!("/admin/federation-providers/{provider_id}/enable-confirm");
    let resp = get(&state, &confirm_uri, &session).await;
    assert!(
        resp.status().is_redirection(),
        "expected step-up redirect, got {}",
        resp.status()
    );
    let step_up_location = location(&resp);
    assert!(
        step_up_location.starts_with("/me/security/step-up?"),
        "expected the step-up challenge, got {step_up_location}"
    );
    assert!(
        step_up_location.contains("federation-providers"),
        "return_to must preserve the confirm page's own path: {step_up_location}"
    );

    // The provider must still be exactly as it was -- nothing executed.
    let row = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(
        !row.enabled,
        "step-up redirect must not have enabled anything"
    );

    // Complete step-up, and land back on the confirm page itself (RFC
    // 089 P5), not on an already-executed action. The step-up
    // challenge's own CSRF cookie comes from a GET to it, not from the
    // redirect response that sent us there.
    let step_up_get = get(&state, &step_up_location, &session).await;
    let csrf = extract_set_cookie(step_up_get.headers(), "sui_id_csrf").expect("csrf cookie");
    let now = chrono::Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;
    let body = format!("_csrf={csrf}&code={code}&return_to={confirm_uri}");
    let step_up_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("step-up POST");
    assert!(step_up_resp.status().is_redirection());
    assert_eq!(location(&step_up_resp), confirm_uri);

    let row_after = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(
        !row_after.enabled,
        "completing step-up must only land back on the confirm page, not execute enable"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_confirm_without_fresh_step_up_redirects_preserving_its_own_path() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    let secret = b"fed-admin-step-up-disable\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, admin.id, secret)
        .await
        .expect("pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, admin.id, b"[]")
        .await
        .expect("confirm totp");
    // Enabled, not disabled: disable has something real to prove it
    // did not do, below.
    let row = disabled_row("https://step-up-disable.example", "up");
    let provider_id = row.id;
    federation_provider::create_enabled_for_test(&state.db, &row, None)
        .await
        .expect("seed enabled provider");

    let confirm_uri = format!("/admin/federation-providers/{provider_id}/disable-confirm");
    let resp = get(&state, &confirm_uri, &session).await;
    assert!(resp.status().is_redirection());
    let step_up_location = location(&resp);
    assert!(step_up_location.starts_with("/me/security/step-up?"));
    assert!(step_up_location.contains("federation-providers"));

    // Completing step-up returns to this same confirm page -- nothing
    // executes until a separate POST (RFC 089 P5).
    let step_up_get = get(&state, &step_up_location, &session).await;
    let csrf = extract_set_cookie(step_up_get.headers(), "sui_id_csrf").expect("csrf cookie");
    let now = chrono::Utc::now().timestamp();
    let step = now / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;
    let body = format!("_csrf={csrf}&code={code}&return_to={confirm_uri}");
    let step_up_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/me/security/step-up")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("step-up POST");
    assert!(step_up_resp.status().is_redirection());
    assert_eq!(location(&step_up_resp), confirm_uri);

    let row_after = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(
        row_after.enabled,
        "completing step-up must only land back on the confirm page, not execute disable"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admin_with_no_mfa_passes_the_step_up_gate_transparently() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let provider_id = seed_disabled_configured_provider(&state, "https://no-mfa.example").await;

    let confirm_uri = format!("/admin/federation-providers/{provider_id}/enable-confirm");
    let resp = get(&state, &confirm_uri, &session).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the default test admin has no MFA; the gate must pass through"
    );
}

// ── Already disabled: a neutral flash, not an error ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabling_an_already_disabled_provider_is_a_neutral_flash() {
    let state = test_app();
    let (session, _admin_id) = log_in_as_admin_in_english(&state).await;
    let provider_id =
        seed_disabled_configured_provider(&state, "https://already-disabled.example").await;

    let resp = get(&state, "/admin/federation-providers", &session).await;
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/disable"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert_eq!(
        location(&resp),
        "/admin/federation-providers?flash=already_disabled"
    );

    let list = get(
        &state,
        "/admin/federation-providers?flash=already_disabled",
        &session,
    )
    .await;
    let body = String::from_utf8_lossy(&read_body(list.into_body()).await).into_owned();
    assert!(
        body.contains("already"),
        "the neutral flash must say the provider was already disabled: {body}"
    );
}

// ── A representative refusal, re-rendered inline ─────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enabling_an_already_enabled_provider_re_renders_the_confirm_page_with_the_refusal() {
    let state = test_app();
    let (session, _admin_id) = log_in_as_admin_in_english(&state).await;
    let row = disabled_row("https://already-enabled.example", "up");
    let provider_id = row.id;
    federation_provider::create_enabled_for_test(&state.db, &row, None)
        .await
        .expect("seed enabled provider");

    let resp = get(&state, "/admin/federation-providers", &session).await;
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/enable"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a refusal re-renders the confirm page, it does not redirect"
    );
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        body.contains("already enabled"),
        "does not show the refusal inline: {body}"
    );
    assert!(
        body.contains("Disable it first"),
        "does not give the next step: {body}"
    );
}

// ── End to end: enable through the admin surface, a real sign-in, ────────
// ── then disable through the admin surface ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_and_enable_through_the_admin_surface_then_a_real_sign_in_then_disable() {
    let mut state = test_app();
    let (session, admin_id) = log_in_as_admin_in_english(&state).await;
    let (issuer, nonce_slot) = with_federation_client_and_mock(&mut state).await;
    add_federation_provider_config(&mut state, &issuer);
    let provider_id = seed_disabled_configured_provider(&state, &issuer).await;
    let user = link_user(&state, provider_id).await;

    let before_audit = audit_rows(&state).await;

    // §A: the enable-confirm page, then the enable POST.
    let confirm = get(
        &state,
        &format!("/admin/federation-providers/{provider_id}/enable-confirm"),
        &session,
    )
    .await;
    assert_eq!(confirm.status(), StatusCode::OK);
    let csrf = extract_set_cookie(confirm.headers(), "sui_id_csrf").expect("csrf");
    let confirm_body = String::from_utf8_lossy(&read_body(confirm.into_body()).await).into_owned();
    assert!(confirm_body.contains("Check and enable"));
    assert!(confirm_body.contains("Check only"));

    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/enable"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert!(resp.status().is_redirection(), "status {}", resp.status());
    assert_eq!(location(&resp), "/admin/federation-providers?flash=enabled");

    let enabled_row = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(enabled_row.enabled);
    assert_eq!(enabled_row.activation_generation, 1);

    // The audit row: `federation.provider.enabled`, administrator actor.
    let after_enable_audit = audit_rows(&state).await;
    // `recent` orders newest first; the rows written since `before_audit`
    // are the leading slice, not the trailing one.
    let new_rows = &after_enable_audit[..after_enable_audit.len() - before_audit.len()];
    let enabled_event = new_rows
        .iter()
        .find(|r| r.action == "federation.provider.enabled")
        .expect("federation.provider.enabled was audited");
    assert_eq!(enabled_event.actor, Some(admin_id));

    // The reason this exists: a real federated sign-in now succeeds.
    let o = super::federation_fail_closed::federated_signin(&state, &nonce_slot).await;
    assert_eq!(o.location, "/admin");
    assert!(o.session_cookie.is_some());

    // §A: the disable-confirm page, then the disable POST.
    let confirm = get(
        &state,
        &format!("/admin/federation-providers/{provider_id}/disable-confirm"),
        &session,
    )
    .await;
    assert_eq!(confirm.status(), StatusCode::OK);
    let csrf = extract_set_cookie(confirm.headers(), "sui_id_csrf").expect("csrf");
    let confirm_body = String::from_utf8_lossy(&read_body(confirm.into_body()).await).into_owned();
    assert!(confirm_body.contains("stops now"));
    assert!(confirm_body.contains("keep their links"));

    let before_disable_audit = audit_rows(&state).await;
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/disable"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert_eq!(
        location(&resp),
        "/admin/federation-providers?flash=disabled"
    );

    let disabled_row_after = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(!disabled_row_after.enabled);
    assert_eq!(disabled_row_after.activation_generation, 2);

    let after_disable_audit = audit_rows(&state).await;
    let disable_rows =
        &after_disable_audit[..after_disable_audit.len() - before_disable_audit.len()];
    let disabled_event = disable_rows
        .iter()
        .find(|r| r.action == "federation.provider.disabled")
        .expect("federation.provider.disabled was audited");
    assert_eq!(disabled_event.actor, Some(admin_id));

    // The login screen's own mechanism for offering this provider --
    // `/auth/federated/up/start` -- no longer offers it.
    let start = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");
    assert_eq!(start.status(), StatusCode::BAD_REQUEST);

    let _ = user;
}

async fn audit_rows(state: &AppState) -> Vec<sui_id_store::models::AuditLogRow> {
    sui_id_store::repos::audit::recent(&state.db, 10_000)
        .await
        .expect("audit tail")
}

/// S5 review: a malformed id is refused with 400 before any step-up
/// redirect, so a raw path segment (a percent-encoded `/` decodes to one)
/// can never become a post-step-up destination. The administrator has TOTP
/// and no fresh step-up, which is exactly when the old order redirected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_provider_id_is_refused_before_any_step_up_redirect() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    let secret = b"fed-admin-step-up-secret\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, admin.id, secret)
        .await
        .expect("pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, admin.id, b"[]")
        .await
        .expect("confirm totp");

    for page in ["enable-confirm", "disable-confirm"] {
        let uri = format!("/admin/federation-providers/x%2F..%2F..%2Fusers/{page}");
        let resp = get(&state, &uri, &session).await;
        assert_eq!(
            resp.status(),
            axum::http::StatusCode::BAD_REQUEST,
            "{page}: a malformed id must be refused, not carried into return_to"
        );
    }
}
