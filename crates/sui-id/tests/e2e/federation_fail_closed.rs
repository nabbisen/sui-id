//! Federated sign-in fails closed (roadmap: federation-signin-fail-closed).
//!
//! The callback runs against a mock upstream IdP on 127.0.0.1: discovery
//! (naming a `jwks_uri`), a `/jwks` endpoint, and a token endpoint that
//! signs a real RS256 ID token for a fixed `sub` -- RFC 096-B1 stage 4
//! routes the callback through `verify_id_token`'s real signature check
//! before any of this file's own assertions run, so an unsigned token (as
//! this file used before that stage) would be refused before the scenarios
//! below ever get the chance to exercise what they are actually testing.
//! A federation link ties that `sub` to a local user.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{Role, UserRow, UserSource};
use tower::ServiceExt;

const SUB: &str = "fed-sub-1";
const SIGNIN_FAILED: &str = "/admin/login?fed_error=signin_failed";

/// Throwaway RSA-2048 fixture key, PKCS#1 DER (`aws-lc-rs`'s
/// `RsaKeyPair::from_der` shape) -- the same literal already committed at
/// `crates/sui-id/src/http/id_token/tests.rs` and
/// `tests/e2e/r096_a_stage4a_verify.rs`, reused rather than generating yet
/// another one: it signs nothing outside throwaway test fixtures in this
/// repository.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-fail-closed-rsa";

fn der(b64_std: &str) -> Vec<u8> {
    Base64::decode_vec(b64_std).expect("valid base64 DER")
}

fn jwks_json() -> serde_json::Value {
    serde_json::json!({ "keys": [{"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E}] })
}

/// Signs a real RS256 ID token for the attempt-row pipeline (stage 4) to
/// verify. `nonce` must be the value the *caller's own* `/start` just
/// generated -- the mock upstream here never actually receives a request
/// at `authorization_endpoint` (this harness jumps straight from `/start`'s
/// redirect to `/callback`, as real OAuth's own browser hop would), so it
/// has no way to learn the nonce except being told, via [`NonceSlot`].
fn sign_id_token(issuer: &str, nonce: &str, sub: &str) -> String {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER_B64));
    let mut header = JwtHeader::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(KID.to_string());
    let claims = serde_json::json!({
        "sub": sub,
        "iss": issuer,
        "aud": "client",
        "nonce": nonce,
        "iat": chrono::Utc::now().timestamp(),
        // `jsonwebtoken::Validation::new`'s default `required_spec_claims`
        // includes "exp" -- far future, since this suite is not testing
        // expiry.
        "exp": 9_999_999_999i64,
    });
    encode(&header, &claims, &key).expect("sign RS256")
}

/// Carries the one value the mock token endpoint cannot otherwise learn
/// (see [`sign_id_token`]): `federated_signin` fills it from `/start`'s own
/// redirect immediately before firing `/callback`, and the mock `/token`
/// handler reads it when building the ID token it returns.
pub(super) type NonceSlot = Arc<Mutex<Option<String>>>;

pub(super) fn nonce_slot() -> NonceSlot {
    Arc::new(Mutex::new(None))
}

/// Adds a `[[federation_providers]]` config entry naming the mock's own
/// issuer and the slug every test here uses ("up") -- stage 4's new
/// validation pipeline looks this up by slug before it will trust any
/// signature at all (`identity_claims`/`id_token_algs`), independently of
/// the `federation_provider` *database* row `linked_user`/`seed_provider`
/// insert elsewhere. Absent from startup config in production would be a
/// real misconfiguration (`fed_error=config_drift`); here, it is simply
/// the fixture this harness is responsible for providing.
pub(super) fn add_federation_provider_config(state: &mut AppState, issuer: &str) {
    let mut cfg = (*state.config).clone();
    cfg.federation_providers
        .push(sui_id::config::FederationProviderConfig {
            slug: "up".into(),
            display_name: "Up".into(),
            issuer: issuer.into(),
            client_id: "client".into(),
            client_secret_env: String::new(),
            scopes: "openid email".into(),
            provision_mode: "link_only".into(),
            enabled: true,
            allowed_origins: Vec::new(),
            id_token_algs: vec!["RS256".into()],
        });
    state.config = Arc::new(cfg);
}

/// Start a mock upstream IdP (over self-signed HTTPS, RFC 134 D3 requires
/// it) and return its issuer URL and the [`NonceSlot`] its `/token` route
/// reads. Callers must also point `state.http_client` at
/// [`super::tls_mock::federation_test_client`] — the real egress client has
/// no reason to trust a cert generated fresh per test — and call
/// [`add_federation_provider_config`] with the returned issuer before
/// driving a sign-in through it.
pub(super) async fn mock_upstream() -> (String, NonceSlot) {
    let slot = nonce_slot();
    let slot_for_token = slot.clone();
    let issuer = super::tls_mock::serve_https(move |issuer| {
        let issuer_for_token = issuer.clone();
        axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let issuer = issuer.clone();
                    async move {
                        axum::Json(serde_json::json!({
                            "issuer": issuer,
                            "authorization_endpoint": format!("{issuer}/authorize"),
                            "token_endpoint": format!("{issuer}/token"),
                            "jwks_uri": format!("{issuer}/jwks"),
                            "response_types_supported": ["code"],
                            "code_challenge_methods_supported": ["S256"],
                            "authorization_response_iss_parameter_supported": true,
                            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
                            "id_token_signing_alg_values_supported": ["RS256"],
                            "subject_types_supported": ["public"],
                        }))
                    }
                }),
            )
            .route(
                "/jwks",
                axum::routing::get(|| async { axum::Json(jwks_json()) }),
            )
            .route(
                "/token",
                axum::routing::post(move || {
                    let slot = slot_for_token.clone();
                    let issuer = issuer_for_token.clone();
                    async move {
                        let nonce = slot
                            .lock()
                            .expect("lock")
                            .take()
                            .expect("federated_signin sets the nonce before calling back");
                        let id_token = sign_id_token(&issuer, &nonce, SUB);
                        axum::Json(
                            serde_json::json!({ "access_token": "at", "id_token": id_token }),
                        )
                    }
                }),
            )
    })
    .await;
    (issuer, slot)
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

/// A local user linked to the mock upstream's `SUB`.
pub(super) async fn linked_user(state: &AppState, issuer: &str) -> UserId {
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
    let provider = uuid::Uuid::new_v4();
    exec(
        state,
        format!(
            "INSERT INTO federation_provider (id, slug, display_name, issuer, client_id, \
             enabled, created_at, updated_at) VALUES ('{provider}', 'up', 'Up', '{issuer}', \
             'client', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'); \
             INSERT INTO federation_link (user_id, provider_id, upstream_sub, linked_at, \
             last_seen_at) VALUES ('{uid}', '{provider}', '{SUB}', '2026-01-01T00:00:00Z', \
             '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
    uid
}

pub(super) struct Outcome {
    pub(super) status: StatusCode,
    pub(super) location: String,
    pub(super) session_cookie: Option<String>,
    pub(super) pending_mfa_cookie: Option<String>,
}

/// Run `/auth/federated/up/start` and then the callback. `nonce_slot` must
/// be the same one the mock upstream's `/token` route reads (from
/// [`mock_upstream`], or a test's own manually-built mock) — this function
/// extracts the nonce `/start` generated from its own upstream-redirect
/// URL and fills the slot immediately before firing the callback, since
/// the mock has no other way to learn it (see [`sign_id_token`]).
pub(super) async fn federated_signin(state: &AppState, nonce_slot: &NonceSlot) -> Outcome {
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
    let fed_state = extract_set_cookie(start.headers(), "sui_id_fed_state").expect("state cookie");
    let location = start
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("upstream redirect");
    let state_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("state="))
        .expect("state parameter");
    // Still percent-encoded: `federated_start` built this URL with
    // `utf8_percent_encode(nonce, NON_ALPHANUMERIC)`, unlike `state_param`
    // just above, which happens to round-trip correctly only because it
    // is re-embedded in a *second* URL's query string and so gets
    // percent-decoded again by `Query<CallbackQuery>` on the way in. The
    // nonce goes straight into a JSON claim below instead, so it must be
    // decoded explicitly here or a non-alphanumeric byte in the random
    // token (a `tokio::test` away from "always", not an edge case) would
    // corrupt it and the real `validate_nonce` would, correctly, refuse.
    let nonce_param = location
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("nonce="))
        .expect("nonce parameter");
    let nonce_param = percent_encoding::percent_decode_str(nonce_param)
        .decode_utf8()
        .expect("nonce parameter is valid UTF-8")
        .into_owned();
    *nonce_slot.lock().expect("lock") = Some(nonce_param);

    let cb = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={state_param}"
                ))
                .header(header::COOKIE, format!("sui_id_fed_state={fed_state}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    Outcome {
        status: cb.status(),
        location: cb
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned(),
        session_cookie: extract_set_cookie(cb.headers(), "sui_id_session")
            .filter(|v| !v.is_empty()),
        pending_mfa_cookie: extract_set_cookie(cb.headers(), "sui_id_pending_mfa")
            .filter(|v| !v.is_empty()),
    }
}

async fn sessions_of(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM sessions WHERE user_id = '{user}'"),
    )
    .await
}

fn assert_refused(o: &Outcome) {
    assert!(o.status.is_redirection(), "status {}", o.status);
    assert_eq!(o.location, SIGNIN_FAILED);
    assert!(o.session_cookie.is_none(), "no session cookie");
    assert!(o.pending_mfa_cookie.is_none(), "no pending-MFA cookie");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_active_linked_user_signs_in() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = linked_user(&state, &issuer).await;

    let o = federated_signin(&state, &nonce_slot).await;
    assert_eq!(o.location, "/admin");
    assert!(o.session_cookie.is_some(), "the harness reaches a session");
    assert_eq!(sessions_of(&state, user).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_user_with_mfa_gets_the_mfa_step_not_a_session() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{user}', X'0102', X'00', 'test', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;

    let o = federated_signin(&state, &nonce_slot).await;
    assert_eq!(o.location, "/admin/login/mfa");
    assert!(o.session_cookie.is_none());
    assert!(o.pending_mfa_cookie.is_some());
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_mfa_read_error_gives_no_session() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = linked_user(&state, &issuer).await;
    // Inject a storage failure into the MFA state read only.
    exec(
        &state,
        "ALTER TABLE user_totp RENAME TO user_totp_gone".into(),
    )
    .await;

    let o = federated_signin(&state, &nonce_slot).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_disabled_user_gets_no_session() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!("UPDATE users SET is_disabled = 1 WHERE id = '{user}'"),
    )
    .await;

    let o = federated_signin(&state, &nonce_slot).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_deleted_user_gets_no_session() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = linked_user(&state, &issuer).await;
    exec(
        &state,
        format!("UPDATE users SET is_deleted = 1 WHERE id = '{user}'"),
    )
    .await;

    let o = federated_signin(&state, &nonce_slot).await;
    assert_refused(&o);
    assert_eq!(sessions_of(&state, user).await, 0);
}

async fn get_with_session(state: &AppState, uri: &str, session: &str) -> StatusCode {
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
        .expect("get")
        .status()
}

async fn refused_after(state: &AppState, session: &str, uri: &str, column: &str) {
    let id: SessionId = session.parse().expect("session id");
    let user = sui_id_store::repos::sessions::get(&state.db, id)
        .await
        .expect("session")
        .user_id;
    exec(
        state,
        format!("UPDATE users SET {column} = 1 WHERE id = '{user}'"),
    )
    .await;
    let status = get_with_session(state, uri, session).await;
    assert!(
        status.is_redirection() || status == StatusCode::UNAUTHORIZED,
        "{uri} after {column}: expected refusal, got {status}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_existing_session_of_a_user_disabled_afterwards_is_refused_on_me_security() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let session = federated_signin(&state, &nonce_slot)
        .await
        .session_cookie
        .expect("session");
    // Both extractors: `CurrentUser` (the MFA tab) and `SessionContext`
    // (the step-up page) accept the session while the user is active.
    assert_eq!(
        get_with_session(&state, "/me/security/mfa", &session).await,
        StatusCode::OK
    );
    assert_eq!(
        get_with_session(&state, "/me/security/step-up", &session).await,
        StatusCode::OK
    );
    refused_after(&state, &session, "/me/security/mfa", "is_disabled").await;
    assert!(
        !get_with_session(&state, "/me/security/step-up", &session)
            .await
            .is_success()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fed_existing_session_of_a_user_deleted_afterwards_is_refused_on_me_security() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let session = federated_signin(&state, &nonce_slot)
        .await
        .session_cookie
        .expect("session");
    refused_after(&state, &session, "/me/security/step-up", "is_deleted").await;
}
