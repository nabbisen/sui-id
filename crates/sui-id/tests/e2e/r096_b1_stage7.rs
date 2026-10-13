//! RFC 096-B1 stage 7 — F04: first federated provisioning, in one Class-A
//! transaction. `commands::tests::runner::federation` already proves the
//! transaction's own properties (rollback, exactly-once, the three
//! rechecks) directly against the store; this file proves the real
//! `/auth/federated/<slug>/start` + `/callback` pair reaches F04 and comes
//! out the other side signed in — the property a store-level test cannot
//! see, since it never drives the handler that resolves the username,
//! builds the rows, and turns a successful commit into a session cookie.
//!
//! Harness duplicated from `r096_b1_stage5.rs` rather than shared (that
//! file's own `claims()` doc comment notes the same choice relative to
//! stage 4's version): this one fixes `email_verified: true` and a
//! non-colliding email, which stage 5's own tests do not need.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, nonce_slot};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-stage7-rsa";

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
        "preferred_username": "new-fed-user",
    })
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

async fn seed_provider_row(state: &AppState, issuer: &str) {
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, provision_mode, enabled, \
              created_at, updated_at) \
             VALUES ('{}', 'up', 'Up', '{issuer}', 'client', 'provision_on_first_login', 1, \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
}

struct Started {
    fed_state_cookie: String,
    browser_binding_cookie: String,
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
    let browser_binding_cookie =
        extract_set_cookie(resp.headers(), "sui_id_fed_browser_binding").expect("binding cookie");
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
        browser_binding_cookie,
        state_param,
    }
}

async fn callback(
    state: &AppState,
    started: &Started,
) -> (axum::http::StatusCode, axum::http::HeaderMap) {
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
                    format!(
                        "sui_id_fed_state={}; sui_id_fed_browser_binding={}",
                        started.fed_state_cookie, started.browser_binding_cookie
                    ),
                )
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback");
    (resp.status(), resp.headers().clone())
}

async fn base_state() -> AppState {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    state
}

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

/// The happy path F04 exists for: an unknown upstream identity, verified
/// email, `provision_on_first_login` — a new user is created, linked,
/// signed in, and lands on `/admin` with a session cookie, through the
/// real router rather than the store directly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_first_time_identity_is_provisioned_and_signed_in() {
    let mut state = base_state().await;
    let (issuer, slot) =
        mock_upstream_for("sub-new-fed-user".into(), "newuser@example.com".into()).await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer).await;

    let started = start(&state, &slot).await;
    let (status, headers) = callback(&state, &started).await;

    assert!(status.is_redirection(), "callback: {status}");
    let location = headers
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("redirect location");
    assert_eq!(location, "/admin");
    assert!(
        extract_set_cookie(&headers, "sui_id_session").is_some(),
        "a session cookie was issued"
    );

    let user_count: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM users WHERE email_normalized = 'newuser@example.com'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count users");
    assert_eq!(user_count, 1, "exactly one user was created");

    let link_count: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM federation_link WHERE upstream_sub = 'sub-new-fed-user'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count links");
    assert_eq!(link_count, 1, "exactly one link was created");

    let audit_count: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.federation.provisioned'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count audit rows");
    assert_eq!(audit_count, 1, "exactly one provisioned event");
}

/// A second callback for the exact same upstream identity, replayed after
/// the first already provisioned it, must not provision a second time:
/// the real "known user" branch (an existing link) is what it takes
/// instead, not F04 again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_returning_identity_does_not_provision_twice() {
    let mut state = base_state().await;
    let (issuer, slot) =
        mock_upstream_for("sub-returning-user".into(), "returning@example.com".into()).await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer).await;

    let first = start(&state, &slot).await;
    let (status, _) = callback(&state, &first).await;
    assert!(status.is_redirection());

    let second = start(&state, &slot).await;
    let (status, headers) = callback(&state, &second).await;
    assert!(status.is_redirection(), "second callback: {status}");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/admin")
    );

    let user_count: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM users WHERE email_normalized = 'returning@example.com'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count users");
    assert_eq!(user_count, 1, "still exactly one user");

    let provisioned_count: i64 = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.federation.provisioned'",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count provisioned events");
    assert_eq!(
        provisioned_count, 1,
        "the second sign-in did not provision again"
    );
}
