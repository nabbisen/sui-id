//! RFC 096-B2 stage 6 — C18 (federation-provider delete) and C20
//! (unlink), through the real HTTP router.
//!
//! Store-level preconditions, rollback, and counts are already covered
//! exhaustively in `sui-id-store`'s own
//! `commands/tests/runner/federation_provider.rs` (`delete_*`,
//! `unlink_*`, `c18_injected_failure_before_append_rolls_back_everything`,
//! `c20_injected_failure_before_append_rolls_back_everything`). This file
//! covers what only the HTTP surface can prove: the pages, the auditor
//! boundary, CSRF, and the two end-to-end flows with their audit rows.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, federated_signin};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{FederationProviderId, UserId};
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

fn disabled_row(issuer: &str, slug: &str, display_name: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: display_name.into(),
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

async fn audit_rows(state: &AppState) -> Vec<sui_id_store::models::AuditLogRow> {
    sui_id_store::repos::audit::recent(&state.db, 10_000)
        .await
        .expect("audit tail")
}

async fn seed_disabled_unconfigured(state: &AppState) -> FederationProviderId {
    let row = disabled_row(
        "https://unconfigured-delete.example",
        "unconfigured-delete",
        "Up",
    );
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed disabled provider");
    provider_id
}

// ── C18: the list offers Delete only when disabled and unconfigured ─────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_configured_provider_offers_no_delete_and_its_post_is_refused() {
    let mut state = test_app();
    let (session, _admin_id) = log_in_as_admin_in_english(&state).await;
    let row = disabled_row("https://configured.example", "up", "Up");
    let provider_id = row.id;
    add_federation_provider_config(&mut state, &row.issuer);
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed disabled, configured provider");

    let body = String::from_utf8_lossy(
        &read_body(
            get(&state, "/admin/federation-providers", &session)
                .await
                .into_body(),
        )
        .await,
    )
    .into_owned();
    assert!(
        !body.contains(&format!(
            "/admin/federation-providers/{provider_id}/delete-confirm"
        )),
        "a configured provider must not offer Delete: {body}"
    );

    // Refused even if posted directly.
    let resp = get(&state, "/admin/federation-providers", &session).await;
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/delete"),
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
        body.contains("[[federation_providers]]"),
        "does not name the reason: {body}"
    );
    assert!(
        federation_provider::get(&state.db, provider_id)
            .await
            .is_ok(),
        "nothing was deleted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auditor_sees_no_delete_and_gets_403_on_the_confirm_page_and_the_post() {
    let state = test_app();
    let (session, admin_id) = log_in_as_admin_in_english(&state).await;
    let provider_id = seed_disabled_unconfigured(&state).await;
    demote_to_auditor(&state, admin_id).await;

    let resp = get(
        &state,
        &format!("/admin/federation-providers/{provider_id}/delete-confirm"),
        &session,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let csrf = {
        let resp = get(&state, "/admin/federation-providers", &session).await;
        extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf")
    };
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/delete"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// ── C18: end to end, with the audit row ──────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_through_the_admin_surface_states_the_count_and_records_the_audit_row() {
    let state = test_app();
    let (session, admin_id) = log_in_as_admin_in_english(&state).await;
    let provider_id = seed_disabled_unconfigured(&state).await;
    let user_a = an_ordinary_user(&state).await;
    let user_b = an_ordinary_user(&state).await;
    link_a_user(&state, user_a, provider_id).await;
    link_a_user(&state, user_b, provider_id).await;

    let confirm = get(
        &state,
        &format!("/admin/federation-providers/{provider_id}/delete-confirm"),
        &session,
    )
    .await;
    assert_eq!(confirm.status(), StatusCode::OK);
    let csrf = extract_set_cookie(confirm.headers(), "sui_id_csrf").expect("csrf");
    let confirm_body = String::from_utf8_lossy(&read_body(confirm.into_body()).await).into_owned();
    assert!(
        confirm_body.contains("2 users"),
        "does not state the exact count: {confirm_body}"
    );
    assert!(confirm_body.contains("cannot be undone"));

    let before = audit_rows(&state).await;
    let resp = post_form(
        &state,
        &format!("/admin/federation-providers/{provider_id}/delete"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert_eq!(location(&resp), "/admin/federation-providers?flash=deleted");

    assert!(matches!(
        federation_provider::get(&state.db, provider_id).await,
        Err(sui_id_store::StoreError::NotFound)
    ));
    for u in [user_a, user_b] {
        assert!(
            sui_id_store::repos::users::get(&state.db, u).await.is_ok(),
            "the user must still exist"
        );
    }

    let after = audit_rows(&state).await;
    let new_rows = &after[..after.len() - before.len()];
    let event = new_rows
        .iter()
        .find(|r| r.action == "federation.provider.deleted")
        .expect("federation.provider.deleted was audited");
    assert_eq!(event.actor, Some(admin_id));

    let list_body = String::from_utf8_lossy(
        &read_body(
            get(
                &state,
                "/admin/federation-providers?flash=deleted",
                &session,
            )
            .await
            .into_body(),
        )
        .await,
    )
    .into_owned();
    assert!(list_body.contains("deleted"), "{list_body}");
}

async fn an_ordinary_user(state: &AppState) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    sui_id_store::repos::users::create(
        &state.db,
        &sui_id_store::models::UserRow {
            id: uid,
            username: format!("user-{uid}"),
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
    uid
}

async fn link_a_user(state: &AppState, user_id: UserId, provider_id: FederationProviderId) {
    let now = chrono::Utc::now();
    sui_id_store::repos::federation_link::upsert(
        &state.db,
        sui_id_store::models::FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: format!("sub-{user_id}"),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .expect("seed link");
}

// ── C20: the user-detail section, the confirm page, auditor read-only ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_detail_lists_the_link_admin_only_unlinks_auditor_read_only() {
    let mut state = test_app();
    let (session, admin_id) = log_in_as_admin_in_english(&state).await;
    let (issuer, _slot) = {
        state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
            super::tls_mock::LoopbackFixtureResolver,
        )));
        super::federation_fail_closed::mock_upstream().await
    };
    add_federation_provider_config(&mut state, &issuer);
    let user = super::federation_fail_closed::linked_user(&state, &issuer).await;
    let provider_id = federation_provider::get_by_slug(&state.db, "up")
        .await
        .expect("seeded by linked_user")
        .id;

    let body = String::from_utf8_lossy(
        &read_body(
            get(&state, &format!("/admin/users/{user}"), &session)
                .await
                .into_body(),
        )
        .await,
    )
    .into_owned();
    assert!(
        body.contains("Up"),
        "does not show the provider name: {body}"
    );
    assert!(
        body.contains(&format!(
            "/admin/users/{user}/federation/{provider_id}/unlink-confirm"
        )),
        "does not offer Unlink: {body}"
    );

    demote_to_auditor(&state, admin_id).await;
    let resp = get(
        &state,
        &format!("/admin/users/{user}/federation/{provider_id}/unlink-confirm"),
        &session,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let csrf = {
        let resp = get(&state, &format!("/admin/users/{user}"), &session).await;
        extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf")
    };
    let resp = post_form(
        &state,
        &format!("/admin/users/{user}/federation/{provider_id}/unlink"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unlink_through_the_admin_surface_keeps_the_user_and_records_the_audit_row() {
    let mut state = test_app();
    let (session, admin_id) = log_in_as_admin_in_english(&state).await;
    let (issuer, _slot) = {
        state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
            super::tls_mock::LoopbackFixtureResolver,
        )));
        super::federation_fail_closed::mock_upstream().await
    };
    add_federation_provider_config(&mut state, &issuer);
    let user = super::federation_fail_closed::linked_user(&state, &issuer).await;
    let provider_id = federation_provider::get_by_slug(&state.db, "up")
        .await
        .expect("seeded by linked_user")
        .id;

    let confirm = get(
        &state,
        &format!("/admin/users/{user}/federation/{provider_id}/unlink-confirm"),
        &session,
    )
    .await;
    assert_eq!(confirm.status(), StatusCode::OK);
    let csrf = extract_set_cookie(confirm.headers(), "sui_id_csrf").expect("csrf");
    let confirm_body = String::from_utf8_lossy(&read_body(confirm.into_body()).await).into_owned();
    // S6 review: the page says open sessions and app tokens stay valid, and
    // names the real way to end them, disabling the account (which revokes
    // both, `disable_user`). It also says the unlink cannot be undone, with
    // the irreversible badge: C19 is withdrawn, and nothing re-links a user.
    assert!(confirm_body.contains("stay valid"), "{confirm_body}");
    assert!(
        confirm_body.contains("disable the account"),
        "{confirm_body}"
    );
    assert!(
        confirm_body.contains("Go to this user&#x27;s page")
            || confirm_body.contains("Go to this user's page"),
        "{confirm_body}"
    );
    assert!(
        confirm_body.contains("cannot be made again"),
        "the page must say the unlink cannot be undone: {confirm_body}"
    );
    // The element, not the bare class name: the page inlines its stylesheet,
    // where both class names appear as selectors.
    assert!(
        confirm_body
            .contains(r#"<span class="reversibility-badge reversibility-badge--permanent">"#),
        "the badge must be the irreversible one: {confirm_body}"
    );
    assert!(
        !confirm_body
            .contains(r#"<span class="reversibility-badge reversibility-badge--recoverable">"#),
        "no recoverable badge: {confirm_body}"
    );

    let before = audit_rows(&state).await;
    let resp = post_form(
        &state,
        &format!("/admin/users/{user}/federation/{provider_id}/unlink"),
        &session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert!(resp.status().is_redirection());
    assert_eq!(location(&resp), format!("/admin/users/{user}"));

    assert!(
        sui_id_store::repos::users::get(&state.db, user)
            .await
            .is_ok(),
        "the user must still exist"
    );
    let after = audit_rows(&state).await;
    let new_rows = &after[..after.len() - before.len()];
    let event = new_rows
        .iter()
        .find(|r| r.action == "federation.link.deleted")
        .expect("federation.link.deleted was audited");
    assert_eq!(event.actor, Some(admin_id));
    assert_eq!(event.target.as_deref(), Some(user.to_string().as_str()));

    // A sign-in through that provider afterward takes the no-link path.
    let o = federated_signin(&state, &_slot).await;
    assert_eq!(o.location, "/admin/login?fed_error=link_required");
    assert!(o.session_cookie.is_none());
}
