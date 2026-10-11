//! RFC 096-B1 stage 5 — the two properties that only the live path can
//! prove.
//!
//! RFC 096 `:699`: `link_only`'s result for an unlinked upstream identity
//! is one generic "local account link required" result — the *same*
//! observable result whether or not that identity's email collides with a
//! local account. `identity_resolution`'s own unit tests prove the module
//! returns the same state either way; what a unit test cannot prove is
//! what a caller observes, which depends on every branch the callback
//! takes before the resolution's own. These two tests pin that, driven
//! against the real router: the generic result under `link_only`, and —
//! since collision denial is specific to `provision_on_first_login`
//! (`:707-708`) — that the denial genuinely still fires under that mode.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, nonce_slot};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use sui_id::{AppState, build_router};
use sui_id_store::models::{Role, UserRow, UserSource};
use tower::ServiceExt;

/// Same throwaway RSA-2048 fixture key as the other stage-4/5 e2e files.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-stage5-fix-rsa";

fn der(b64_std: &str) -> Vec<u8> {
    Base64::decode_vec(b64_std).expect("valid base64 DER")
}

fn jwks_json() -> serde_json::Value {
    serde_json::json!({ "keys": [{"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E}] })
}

fn sign(claims: &serde_json::Value) -> String {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER_B64));
    let mut header = JwtHeader::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(KID.to_string());
    encode(&header, claims, &key).expect("sign RS256")
}

/// Unlike the stage-4 file's `claims()`, `sub` and `email` are both
/// parameters: these tests need two different upstream identities in one
/// run (one colliding, one not), which the stage-4 file's fixed `SUB`
/// constant cannot provide.
fn claims(issuer: &str, nonce: &str, sub: &str, email: &str) -> serde_json::Value {
    serde_json::json!({
        "sub": sub,
        "iss": issuer,
        "aud": "client",
        "nonce": nonce,
        "iat": chrono::Utc::now().timestamp(),
        "exp": 9_999_999_999i64,
        "email": email,
        "email_verified": true,
    })
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn seed_unlinked_local_user_with_email(state: &AppState, email: &str) {
    let now = chrono::Utc::now();
    sui_id_store::repos::users::create(
        &state.db,
        &UserRow {
            id: sui_id_shared::ids::UserId::new(),
            username: format!("existing-{}", uuid::Uuid::new_v4().simple()),
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
            email: Some(email.to_owned()),
            preferred_lang: None,
            email_normalized: None,
            email_verified_at: None,
        },
    )
    .await
    .expect("create local user");
}

/// Inserts the `federation_provider` row the real handler needs
/// (`add_federation_provider_config` only adds the startup-config side;
/// `federated_start`'s `federation_provider::get_by_slug` reads this
/// table). Deliberately not `linked_user`: that helper always creates a
/// link, and these tests need the identity to stay *unlinked*.
async fn seed_provider_row(state: &AppState, issuer: &str, provision_mode: &str) {
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, provision_mode, enabled, \
              created_at, updated_at) \
             VALUES ('{}', 'up', 'Up', '{issuer}', 'client', '{provision_mode}', 1, \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
}

struct Started {
    fed_state_cookie: String,
    state_param: String,
}

async fn start(state: &AppState, slot: &super::federation_fail_closed::NonceSlot) -> Started {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/up/start")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");
    assert!(resp.status().is_redirection(), "start: {}", resp.status());
    let fed_state_cookie =
        extract_set_cookie(resp.headers(), "sui_id_fed_state").expect("state cookie");
    let location = resp
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
        .expect("nonce is valid UTF-8")
        .into_owned();
    *slot.lock().expect("lock") = Some(nonce_param);
    Started {
        fed_state_cookie,
        state_param,
    }
}

async fn callback(state: &AppState, started: &Started) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={}",
                    started.state_param
                ))
                .header(
                    header::COOKIE,
                    format!("sui_id_fed_state={}", started.fed_state_cookie),
                )
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    assert!(
        resp.status().is_redirection(),
        "callback: {}",
        resp.status()
    );
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

async fn base_state() -> AppState {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    state
}

/// A mock upstream signing a token for a caller-supplied `(sub, email)`
/// pair rather than a fixed one.
async fn mock_upstream_for(
    sub: String,
    email: String,
) -> (String, super::federation_fail_closed::NonceSlot) {
    let slot = nonce_slot();
    let slot_for_token = slot.clone();
    let issuer = super::tls_mock::serve_https(move |issuer| {
        let issuer_for_token = issuer.clone();
        let sub = sub.clone();
        let email = email.clone();
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
                    let sub = sub.clone();
                    let email = email.clone();
                    async move {
                        let nonce = slot
                            .lock()
                            .expect("lock")
                            .take()
                            .expect("start sets the nonce before the callback");
                        let token = sign(&claims(&issuer, &nonce, &sub, &email));
                        axum::Json(serde_json::json!({ "access_token": "at", "id_token": token }))
                    }
                }),
            )
    })
    .await;
    (issuer, slot)
}

/// R1's central regression: a `link_only`, unlinked upstream identity
/// whose email collides with a local account must redirect to the exact
/// same place as one whose email does not. `assert_eq!(a, b)` on the
/// observed value, not merely that each is individually plausible —
/// mirroring the shape the unit-level generic-result test already got
/// right (`identity_resolution::tests::
/// link_only_returns_the_same_generic_result_even_when_the_email_would_collide`),
/// now proven against the live router rather than the module alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn link_only_gives_the_same_result_whether_or_not_the_email_collides() {
    let mut state_a = base_state().await;
    let (issuer_a, slot_a) = mock_upstream_for("sub-a".into(), "probe@example.com".into()).await;
    add_federation_provider_config(&mut state_a, &issuer_a);
    seed_provider_row(&state_a, &issuer_a, "link_only").await;
    seed_unlinked_local_user_with_email(&state_a, "Probe@Example.com").await;
    let started_a = start(&state_a, &slot_a).await;
    let location_a = callback(&state_a, &started_a).await;

    // A fresh app state for the second run: each run needs its own
    // `federation_provider` row and mock upstream, and nothing about the
    // property under test requires sharing either between them.
    let mut state_b = base_state().await;
    let (issuer_b, slot_b) =
        mock_upstream_for("sub-b".into(), "never-seen@example.com".into()).await;
    add_federation_provider_config(&mut state_b, &issuer_b);
    seed_provider_row(&state_b, &issuer_b, "link_only").await;
    let started_b = start(&state_b, &slot_b).await;
    let location_b = callback(&state_b, &started_b).await;

    assert_eq!(location_a, "/admin/login?fed_error=link_required");
    assert_eq!(location_b, "/admin/login?fed_error=link_required");
    assert_eq!(
        location_a, location_b,
        "link_only must not distinguish a collision from any other unlinked identity"
    );
}

/// The move must not disarm `provision_on_first_login`'s own takeover
/// denial: the exact same colliding identity, under the other mode, must
/// still land on `fed_error=email_collision`, not provision a new account.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provision_on_first_login_still_denies_a_genuine_collision() {
    let mut state = base_state().await;
    let (issuer, slot) =
        mock_upstream_for("sub-collision".into(), "collides@example.com".into()).await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer, "provision_on_first_login").await;
    seed_unlinked_local_user_with_email(&state, "Collides@Example.com").await;

    let started = start(&state, &slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=email_collision");
}
