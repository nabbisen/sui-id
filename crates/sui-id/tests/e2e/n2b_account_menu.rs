//! RFC 144 (Refresh, §4) / N2b §4: every page that shows the console
//! navigation renders the account menu with the signed-in person's
//! name, and never the retired bare "Sign out" fallback
//! (`app-nav__signout` — deleted by this dispatch, grepped for here as
//! a belt-and-braces check since the class no longer exists anywhere
//! to typo back in).
//!
//! RFC 144's own count: 38 `Chrome::Console` render call sites across
//! 21 files. 34 are reachable with a plain signed-in GET and no
//! further precondition, and are swept in
//! [`every_get_reachable_show_nav_route_renders_the_account_menu`].
//! Of the remaining 4: 3 render only as a POST's response
//! (`render_mfa_setup`, and `recovery_link.rs`'s issued/refused
//! pair), and `users_recovery_link_confirm_get`'s own GET needs the
//! issuing admin to hold a second factor (RFC 103 D6) before it
//! renders instead of refusing. All 4 are exercised directly, below.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
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

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

/// Asserts the account menu is present, names the signed-in admin, and
/// that the retired fallback never reappears.
fn assert_account_menu(html: &str, route: &str) {
    assert!(
        html.contains(r#"class="user-menu""#),
        "{route}: no account menu rendered"
    );
    assert!(
        html.contains(USERNAME),
        "{route}: account menu does not show the signed-in name ({USERNAME})"
    );
    assert!(
        !html.contains("app-nav__signout"),
        "{route}: rendered the retired bare sign-out fallback"
    );
}

async fn assert_route_has_account_menu(state: &AppState, session: &str, uri: &str) {
    let resp = get(state, uri, session).await;
    assert_eq!(resp.status(), StatusCode::OK, "{uri}: expected 200");
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert_account_menu(&html, uri);
}

fn disabled_provider_row(slug: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: sui_id_shared::ids::FederationProviderId::new(),
        slug: slug.into(),
        display_name: "N2b Up".into(),
        issuer: format!("https://{slug}.example.com"),
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

#[tokio::test]
async fn every_get_reachable_show_nav_route_renders_the_account_menu() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");

    // ---- Fixtures ----
    let (client_id, _secret) = create_client(&state, &session).await;

    let target = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "n2b-target",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "n2b-target-password-1",
    )
    .await
    .expect("create target user");

    let signing_key_id = sui_id_core::admin::list_signing_keys(&state.db)
        .await
        .expect("list signing keys")
        .into_iter()
        .next()
        .expect("setup seeds at least one signing key")
        .id;

    let disabled_provider_for_enable = disabled_provider_row("n2b-enable");
    let disabled_provider_id = disabled_provider_for_enable.id;
    federation_provider::create(&state.db, &disabled_provider_for_enable, None)
        .await
        .expect("seed disabled provider");

    let mut enabled_row = disabled_provider_row("n2b-disable");
    enabled_row.enabled = true;
    let enabled_provider_id = enabled_row.id;
    federation_provider::create_enabled_for_test(&state.db, &enabled_row, None)
        .await
        .expect("seed enabled provider");

    let linked_user = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "n2b-linked",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "n2b-linked-password-1",
    )
    .await
    .expect("create linked user");
    state
        .db
        .with_conn({
            let provider_id = enabled_provider_id;
            let user_id = linked_user.id;
            move |c| {
                Ok(c.execute(
                    "INSERT INTO federation_link \
                     (user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
                     VALUES (?1, ?2, 'n2b-sub', ?3, ?3)",
                    rusqlite::params![
                        user_id.to_string(),
                        provider_id.to_string(),
                        chrono::Utc::now().to_rfc3339(),
                    ],
                )?)
            }
        })
        .await
        .expect("link user to provider");

    // A pending SMTP-password change, so settings/email/confirm has a
    // real pending_change_id to render.
    let csrf = fetch_csrf(&state, &session).await;
    let email_resp = post_form(
        &state,
        "/admin/settings/email",
        &session,
        &csrf,
        &format!(
            "_csrf={csrf}&host=smtp.example.com&port=587&tls_mode=starttls\
             &username=&password={}&from_address=noreply%40example.com\
             &from_name=&base_url=https%3A%2F%2Fexample.com",
            urlencode("a-new-smtp-password")
        ),
    )
    .await;
    let email_confirm_path = location(&email_resp);
    assert!(
        email_confirm_path.starts_with("/admin/settings/email/confirm"),
        "expected a redirect to the email confirm page, got {email_confirm_path:?}"
    );

    // ---- The 35 GET-reachable routes ----
    let routes: Vec<String> = vec![
        "/admin".into(),
        "/admin/users".into(),
        "/admin/users/new".into(),
        format!("/admin/users/{}", target.id),
        format!("/admin/users/{}/disable-confirm", target.id),
        format!("/admin/users/{}/delete-confirm", target.id),
        format!("/admin/users/{}/mfa-reset-confirm", target.id),
        "/admin/clients".into(),
        "/admin/clients/new".into(),
        format!("/admin/clients/{client_id}/edit"),
        format!("/admin/clients/{client_id}/delete-confirm"),
        "/admin/signing-keys".into(),
        "/admin/signing-keys/rotate-confirm".into(),
        format!("/admin/signing-keys/{signing_key_id}/delete-confirm"),
        "/admin/federation-providers".into(),
        format!("/admin/federation-providers/{disabled_provider_id}/enable-confirm"),
        format!("/admin/federation-providers/{enabled_provider_id}/disable-confirm"),
        format!("/admin/federation-providers/{enabled_provider_id}/delete-confirm"),
        format!(
            "/admin/users/{}/federation/{enabled_provider_id}/unlink-confirm",
            linked_user.id
        ),
        "/admin/audit".into(),
        "/admin/settings/basic".into(),
        "/admin/settings/security".into(),
        "/admin/settings/authentication".into(),
        "/admin/settings/logs".into(),
        "/admin/settings/other".into(),
        "/admin/settings/email".into(),
        email_confirm_path,
        "/me/security/overview".into(),
        "/me/security/mfa".into(),
        "/me/security/sessions".into(),
        "/me/security/passkeys".into(),
        "/me/security/language".into(),
        "/me/apps".into(),
        "/me/security/password".into(),
    ];
    assert_eq!(
        routes.len(),
        34,
        "this is the GET-reachable-with-no-further-precondition share of RFC 144's \
         38 Chrome::Console call sites; 3 render only as a POST's response, and \
         users_recovery_link_confirm_get's GET additionally needs the issuing \
         admin to hold a second factor (RFC 103 D6) — all 4 are exercised in the \
         tests below instead"
    );
    for uri in &routes {
        assert_route_has_account_menu(&state, &session, uri).await;
    }
}

/// `render_mfa_setup` (`auth.rs`): rendered only as the response to
/// `POST /me/security/mfa/enroll/start`, never a GET of its own.
#[tokio::test]
async fn mfa_setup_renders_the_account_menu() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        "/me/security/mfa/enroll/start",
        &session,
        &csrf,
        &format!("_csrf={csrf}&current_password={}", urlencode(PASSWORD)),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert_account_menu(&html, "POST /me/security/mfa/enroll/start");
}

/// `render_recovery_link_refused` (`recovery_link.rs`): a freshly
/// set-up admin holds no second factor, so RFC 103 D6 refuses the
/// issuance outright (`NeedsSecondFactor`) — reachable with no
/// further setup.
#[tokio::test]
async fn recovery_link_refused_renders_the_account_menu() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    let target = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "n2b-recovery-target",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "n2b-recovery-password-1",
    )
    .await
    .expect("create target user");
    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        &format!("/admin/users/{}/recovery-link", target.id),
        &session,
        &csrf,
        &format!(
            "_csrf={csrf}&_confirmed=1&reason={}",
            urlencode("n2b account-menu check")
        ),
    )
    .await;
    // RFC 103 D6's refusal page: the issuing admin holds no second
    // factor, which `recovery_refused_page` maps to 403 — a full
    // rendered page, not an empty error response.
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert_account_menu(&html, "POST /admin/users/{id}/recovery-link (refused)");
}

/// `render_recovery_link_issued` (`recovery_link.rs`): the success
/// path, which RFC 103 D6 gates on the issuing administrator holding
/// a second factor and a fresh step-up. Also covers
/// `users_recovery_link_confirm_get`'s GET, gated on the same rule —
/// left out of the main sweep above for exactly this reason.
#[tokio::test]
async fn recovery_link_issued_renders_the_account_menu() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    // Enrol TOTP directly via the store layer (same shape as
    // `step_up_totp.rs`'s own fixture) so the admin holds a second
    // factor, which RFC 103 D6 requires to issue a link. Enrolling
    // alone is not enough: `policy_for_session` also requires a
    // *fresh* step-up once `user_has_mfa` is true, so a real step-up
    // POST follows, the same way `step_up_post_with_correct_totp_
    // marks_session_fresh_and_redirects` establishes one.
    let secret = b"n2b-recovery-link-secret\x00\x00\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, admin.id, secret)
        .await
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, admin.id, b"[]")
        .await
        .expect("confirm totp");
    let step_up_csrf = fetch_csrf(&state, &session).await;
    let step = chrono::Utc::now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;
    let step_up_resp = post_form(
        &state,
        "/me/security/step-up",
        &session,
        &step_up_csrf,
        &format!("_csrf={step_up_csrf}&code={code}&return_to=/admin"),
    )
    .await;
    assert!(
        step_up_resp.status().is_redirection(),
        "expected the step-up POST to redirect, got {}",
        step_up_resp.status()
    );

    let target = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "n2b-issued-target",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "n2b-issued-password-1",
    )
    .await
    .expect("create target user");

    assert_route_has_account_menu(
        &state,
        &session,
        &format!("/admin/users/{}/recovery-link-confirm", target.id),
    )
    .await;

    let csrf = fetch_csrf(&state, &session).await;
    let resp = post_form(
        &state,
        &format!("/admin/users/{}/recovery-link", target.id),
        &session,
        &csrf,
        &format!(
            "_csrf={csrf}&_confirmed=1&reason={}",
            urlencode("n2b account-menu check, issued path")
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp.into_body()).await;
    let html = String::from_utf8_lossy(&body).to_string();
    assert_account_menu(&html, "POST /admin/users/{id}/recovery-link (issued)");
}
