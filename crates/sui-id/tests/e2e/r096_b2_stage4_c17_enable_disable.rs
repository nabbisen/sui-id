//! RFC 096-B2 stage 4 §E: "end to end, the reason this exists." A
//! disabled provider that `check_and_enable` validates against a real
//! mock upstream becomes one a real federated sign-in can actually
//! complete through, via the harness's own sign-in path
//! (`federation_fail_closed::federated_signin`). Then
//! `disable_federation_provider` takes that away: an attempt already
//! past `/start` when the disable lands cannot finish, and a brand-new
//! attempt afterward is refused before it ever reaches the mock.

use super::common::{extract_set_cookie, test_app};
use super::federation_fail_closed::{add_federation_provider_config, federated_signin};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::str::FromStr;
use sui_id::{AppState, build_router};
use sui_id_core::actor::{Actor, AdminActor};
use sui_id_shared::ids::{FederationProviderId, SessionId, UserId};
use sui_id_store::commands::disable_federation_provider;
use sui_id_store::models::{
    FederationProviderRow, ProvisionMode, Role, UserRow, UserSource, ValidationStatus,
};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

/// Must match `federation_fail_closed`'s own private `SUB` -- the mock
/// upstream's `/token` route signs this subject regardless of who seeds
/// the link; this is plain data, not a reference to that module's
/// internals.
const SUB: &str = "fed-sub-1";

/// A disabled row with a recorded trust policy (`id_token_algs =
/// Some("RS256")`, matching what the mock upstream's token endpoint
/// signs with) -- `check_and_enable`'s own required starting state,
/// unlike `federation_fail_closed::linked_user`'s raw `INSERT`, which
/// seeds an already-*enabled* row.
fn disabled_row(issuer: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "up".into(),
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

/// A local user linked to the mock upstream's `SUB`, against whatever
/// provider id the caller has already seeded -- the link half of
/// `federation_fail_closed::linked_user`, without that function's own
/// (always-enabled) provider row.
async fn link_user(state: &AppState, provider_id: FederationProviderId) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    sui_id_store::repos::users::create(
        &state.db,
        &UserRow {
            id: uid,
            username: "fed-user".into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: false,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source: UserSource::Local,
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

async fn sessions_of(state: &AppState, user: UserId) -> i64 {
    state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [user.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count sessions")
}

/// `complete_setup_and_login`'s own session cookie, resolved into the
/// `(UserId, AdminActor)` the store commands and `check_and_enable`
/// need -- the same resolution `CurrentAdmin`'s extractor does
/// (`crates/sui-id/src/http/handlers.rs`), done here directly since
/// this test drives the store layer, not every admin HTTP route.
async fn admin_actor(state: &AppState, session_cookie: &str) -> (UserId, SessionId, AdminActor) {
    let session_id = SessionId::from_str(session_cookie).expect("parse session id");
    let user_id = sui_id_core::session::resolve(&state.db, &state.clock, session_id)
        .await
        .expect("resolve session");
    let user = sui_id_store::repos::users::get(&state.db, user_id)
        .await
        .expect("get admin user");
    let admin = Actor::from_session(user_id, user.role, session_id)
        .into_admin()
        .unwrap_or_else(|_| panic!("the setup-wizard user must be Role::Admin"));
    (user_id, session_id, admin)
}

/// `/auth/federated/up/start`, stopping short of the callback: returns
/// the `state` query parameter the callback needs and fills
/// `nonce_slot` the way `federation_fail_closed::federated_signin_with_
/// next` does -- split out here so a disable can land in between
/// `/start` succeeding and the callback actually running.
async fn start_federated_flow(
    state: &AppState,
    nonce_slot: &super::federation_fail_closed::NonceSlot,
) -> String {
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
    assert!(start.status().is_redirection(), "start: {}", start.status());
    let location = start
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("upstream redirect")
        .to_owned();
    let state_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("state="))
        .expect("state parameter")
        .to_owned();
    let nonce_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("nonce="))
        .expect("nonce parameter");
    let nonce_param = percent_encoding::percent_decode_str(nonce_param)
        .decode_utf8()
        .expect("nonce parameter is valid UTF-8")
        .into_owned();
    *nonce_slot.lock().expect("lock") = Some(nonce_param);
    state_param
}

/// The callback half of `start_federated_flow`: fires
/// `/auth/federated/callback` with the `state` that call captured, and
/// returns `(redirect location, session cookie)`.
async fn finish_federated_flow(state: &AppState, state_param: &str) -> (String, Option<String>) {
    let cb = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={state_param}"
                ))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    let location = cb
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let session_cookie =
        extract_set_cookie(cb.headers(), "sui_id_session").filter(|v| !v.is_empty());
    (location, session_cookie)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_and_enable_lets_a_real_sign_in_through_then_disable_takes_it_away() {
    let mut state = test_app();
    let session_cookie = super::common::complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    )));
    let (issuer, nonce_slot) = super::federation_fail_closed::mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);

    let row = disabled_row(&issuer);
    let provider_id = row.id;
    // Confidential: the mock discovery document (`federation_fail_closed
    // ::mock_upstream`) advertises only `client_secret_basic`, never
    // `none` -- same reason `linked_user` seals a secret too.
    federation_provider::create(
        &state.db,
        &row,
        Some(super::federation_fail_closed::TEST_CLIENT_SECRET),
    )
    .await
    .expect("seed disabled provider");
    let user = link_user(&state, provider_id).await;
    let (admin_id, session_id, admin) = admin_actor(&state, &session_cookie).await;

    // §A/§E: `check_and_enable` against the real mock upstream.
    sui_id::federation_preflight::check_and_enable(&state, &admin, provider_id)
        .await
        .expect("check_and_enable must succeed against the mock upstream");
    let enabled = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(enabled.enabled, "the provider must now be enabled");
    assert_eq!(enabled.activation_generation, 1);
    assert_eq!(
        enabled.validation_status,
        sui_id_store::models::ValidationStatus::Validated
    );

    // The reason this exists: a real federated sign-in now succeeds.
    let o = federated_signin(&state, &nonce_slot).await;
    assert_eq!(
        o.location, "/admin",
        "a real sign-in must succeed once the provider is enabled"
    );
    assert!(o.session_cookie.is_some(), "the harness reaches a session");
    assert_eq!(sessions_of(&state, user).await, 1);

    // An attempt already past `/start` when the disable lands: capture
    // its `state` and nonce first, *then* disable, *then* finish it.
    let in_flight_state_param = start_federated_flow(&state, &nonce_slot).await;

    let now = chrono::Utc::now();
    disable_federation_provider(&state.db, admin_id, session_id, provider_id, now)
        .await
        .expect("disable must not error")
        .expect("an enabled provider must be disabled");
    let disabled_again = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert!(!disabled_again.enabled);
    assert_eq!(disabled_again.activation_generation, 2);

    let (in_flight_location, in_flight_session) =
        finish_federated_flow(&state, &in_flight_state_param).await;
    assert_eq!(
        in_flight_location, "/admin/login?fed_error=disabled",
        "an attempt started before the disable must not be able to complete it"
    );
    assert!(
        in_flight_session.is_none(),
        "no session from the refused completion"
    );
    assert_eq!(
        sessions_of(&state, user).await,
        1,
        "the in-flight attempt's completion must not have created a session"
    );

    // A brand-new sign-in attempt afterward is refused before it ever
    // reaches the mock -- `/start`'s own re-check (`federated_start`,
    // `if !provider.enabled`), not a round trip to the upstream at all.
    let new_start = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("new start attempt");
    assert_eq!(
        new_start.status(),
        StatusCode::BAD_REQUEST,
        "a disabled provider must refuse `/start` outright, not redirect toward the upstream"
    );
    assert_eq!(
        sessions_of(&state, user).await,
        1,
        "the refused new attempt must not have created a session"
    );
}
