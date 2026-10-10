//! RFC 096-B1 stage 4 — 096-A's validators get their first production
//! callers. `federation_fail_closed.rs` proves the happy path and the
//! pre-existing account-state refusals, all still gated by the *old*
//! trust-on-TLS code (unchanged, running after this stage's new block
//! succeeds). This file proves the *new* block's own refusals: every test
//! here signs or shapes an ID token response the old code would have
//! accepted outright — the old decode does no signature check, no issuer
//! check, no `iat` check, and the old nonce check reads the same cookie
//! value this file's forged tokens still carry correctly — and shows the
//! new pipeline refuses it first, before the old code ever runs.

use super::common::*;
use super::federation_fail_closed::{
    add_federation_provider_config, linked_user, mock_upstream, nonce_slot,
};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

/// Same throwaway RSA-2048 fixture key as `federation_fail_closed.rs`.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-stage4-rsa";
const SUB: &str = "fed-sub-1";

fn der(b64_std: &str) -> Vec<u8> {
    Base64::decode_vec(b64_std).expect("valid base64 DER")
}

fn jwks_json() -> serde_json::Value {
    serde_json::json!({ "keys": [{"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E}] })
}

/// Signs `claims` with the key the mock's own JWKS serves, under its own
/// `kid` — callers that want a refusal vary `claims` (wrong issuer, stale
/// `iat`, wrong nonce, ...) rather than the key, so the structural
/// signature stays genuinely valid and only the one property under test
/// is wrong. [`forged_signature_is_refused`] is the one exception: it
/// corrupts a correctly-signed token's signature bytes afterward instead.
fn sign(claims: &serde_json::Value) -> String {
    let key = EncodingKey::from_rsa_der(&der(RSA_DER_B64));
    let mut header = JwtHeader::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(KID.to_string());
    encode(&header, claims, &key).expect("sign RS256")
}

fn claims(issuer: &str, nonce: &str) -> serde_json::Value {
    serde_json::json!({
        "sub": SUB,
        "iss": issuer,
        "aud": "client",
        "nonce": nonce,
        "iat": chrono::Utc::now().timestamp(),
        "exp": 9_999_999_999i64,
    })
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

/// A mock upstream whose `/token` response is entirely up to the caller:
/// every test in this file needs to send something the *correct* mock in
/// `federation_fail_closed::mock_upstream` cannot (a bad signature, a
/// wrong claim, a response with no `id_token` at all). `body` is given
/// the issuer and the real nonce `/start` generated (already
/// percent-decoded) and returns the full JSON the mock `/token` route
/// replies with.
async fn custom_mock_upstream(
    body: impl Fn(&str, &str) -> serde_json::Value + Send + Sync + 'static,
) -> (String, super::federation_fail_closed::NonceSlot) {
    let slot = nonce_slot();
    let slot_for_token = slot.clone();
    let body = std::sync::Arc::new(body);
    let issuer = super::tls_mock::serve_https(move |issuer| {
        let issuer_for_token = issuer.clone();
        let body = body.clone();
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
                    let body = body.clone();
                    async move {
                        let nonce =
                            slot.lock().expect("lock").take().expect(
                                "federated_signin/start sets the nonce before the callback",
                            );
                        axum::Json(body(&issuer, &nonce))
                    }
                }),
            )
    })
    .await;
    (issuer, slot)
}

struct Started {
    fed_state_cookie: String,
    state_param: String,
}

/// Drives `/start` only — the piece every test in this file shares before
/// it diverges on exactly how `/token`'s response is wrong.
async fn start(state: &AppState, nonce_slot: &super::federation_fail_closed::NonceSlot) -> Started {
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
    *nonce_slot.lock().expect("lock") = Some(nonce_param);
    Started {
        fed_state_cookie,
        state_param,
    }
}

/// Drives `/callback` for an already-started attempt and returns its
/// redirect `Location`.
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

/// **Not a demonstration of `fed_error=superseded`.** The obvious way to
/// manufacture a row whose recorded provider trust generation disagrees
/// with "current" — tamper the row's own `provider_config_version` after
/// `/start` — does not reach the superseded check at all: that column is
/// AAD-bound by stage 2's seal (`insert`/`claim`, RFC 096 `:583`'s PKCE
/// binding), so altering it is caught by the *earlier* tamper check
/// instead. This test pins that finding directly, so it is recorded as a
/// property rather than left as a surprise the next reader has to
/// rediscover: with today's placeholder (every provider's version/
/// generation fixed at `(0, 0)`, RFC 096-B2/M2b not yet landed), the
/// superseded check is wired into `federated_callback` but not reachable
/// end-to-end by any input, tampered or genuine — `attempt_is_superseded`
/// is unit-tested directly instead (`handlers::federation::tests`), and
/// *that* is what proves the comparison itself is correct.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tampering_the_rows_own_provider_generation_is_caught_as_tampering_not_as_superseded() {
    let (issuer, nonce_slot) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    exec(
        &state,
        "UPDATE federation_login_attempt SET provider_config_version = 1".into(),
    )
    .await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=tamper_detected");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_signature_byte_flipped_after_signing_is_refused() {
    let (issuer, nonce_slot) = custom_mock_upstream(|issuer, nonce| {
        let mut token = sign(&claims(issuer, nonce));
        let last = token.pop().expect("token has a final character");
        token.push(if last == 'A' { 'B' } else { 'A' });
        serde_json::json!({ "access_token": "at", "id_token": token })
    })
    .await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=signature_invalid");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_issuer_claim_is_refused() {
    let (issuer, nonce_slot) = custom_mock_upstream(|_issuer, nonce| {
        let token = sign(&claims("https://not-the-real-issuer.invalid", nonce));
        serde_json::json!({ "access_token": "at", "id_token": token })
    })
    .await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=identity_claims_invalid");
}

/// `validate_iat`'s lower bound is the *attempt's own* `created_at`
/// (RFC 096 `:676`), not some fixed skew from "now" — a token minted
/// before this attempt even existed is refused as a substitution attempt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_iat_from_before_the_attempt_existed_is_refused() {
    let (issuer, nonce_slot) = custom_mock_upstream(|issuer, nonce| {
        let mut c = claims(issuer, nonce);
        c["iat"] = serde_json::json!((chrono::Utc::now() - chrono::Duration::hours(1)).timestamp());
        let token = sign(&c);
        serde_json::json!({ "access_token": "at", "id_token": token })
    })
    .await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=time_claims_invalid");
}

/// RFC 096 `:593-594`: the ID token is mandatory. The pre-existing
/// userinfo-fallback branch, for a response with no `id_token` at all, is
/// unreachable from this stage on — this is the test that proves it, not
/// merely documents it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_token_response_without_an_id_token_is_refused() {
    let (issuer, nonce_slot) =
        custom_mock_upstream(|_issuer, _nonce| serde_json::json!({ "access_token": "at" })).await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=missing_id_token");
}

/// The nonce check this stage wires is `validate_nonce` via
/// `claim_and_consume_nonce` — distinct from the pre-existing cookie-based
/// nonce check a few lines below in the old code, which never runs here
/// because the new check refuses first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_nonce_is_refused_by_the_new_pipeline() {
    let (issuer, nonce_slot) = custom_mock_upstream(|issuer, _nonce| {
        let token = sign(&claims(issuer, "not-the-real-nonce"));
        serde_json::json!({ "access_token": "at", "id_token": token })
    })
    .await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=nonce_mismatch");
}

/// `validate_optional_claims` has no expected values to compare against
/// (unlike the other four) and no e2e scenario above happens to send any
/// optional claim at all, so none of them exercise its own failure path.
/// A malformed `email` (no `@`, same bound `optional_claims::tests`
/// pins at the unit level) is this file's only way to show it is a real,
/// routed check and not dead code once it reaches the real callback.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_optional_claim_is_refused() {
    let (issuer, nonce_slot) = custom_mock_upstream(|issuer, nonce| {
        let mut c = claims(issuer, nonce);
        c["email"] = serde_json::json!("not-an-email-address");
        let token = sign(&c);
        serde_json::json!({ "access_token": "at", "id_token": token })
    })
    .await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=optional_claims_invalid");
}
