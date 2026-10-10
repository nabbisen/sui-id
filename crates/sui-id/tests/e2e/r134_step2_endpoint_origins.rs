//! RFC 134 step 2 (D3) — discovery is untrusted input.
//!
//! `ValidatedDiscovery`'s own unit tests (`crates/sui-id/src/http/
//! discovery.rs`) cover every specific rule (each endpoint independently,
//! `http://` rejected, port equivalence, the empty-set default) against
//! the pure validation function — no network needed, since `validate()`
//! performs none. What a unit test on that function *cannot* prove is
//! that the real HTTP handlers actually reach it before making any
//! request with the rejected endpoint. That is what this file proves,
//! against the real router, over real (self-signed) TLS.

use super::common::{complete_setup_and_login, extract_set_cookie, test_app};
use super::federation_fail_closed::{add_federation_provider_config, federated_signin, nonce_slot};
use super::tls_mock::{federation_test_client, serve_https};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_id::AppState;
use sui_id::build_router;
use tower::ServiceExt;

const DISCOVERY_ORIGIN_REJECTED: &str = "/admin/login?fed_error=discovery_origin";

/// Same throwaway RSA-2048 fixture key as `federation_fail_closed.rs`
/// (`crates/sui-id/src/http/id_token/tests.rs`'s own literal) -- this file
/// needs its own signed token because its mock upstream is hand-built
/// (split across two origins, the point under test), not
/// `federation_fail_closed::mock_upstream`'s.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-r134-rsa";

fn der(b64_std: &str) -> Vec<u8> {
    Base64::decode_vec(b64_std).expect("valid base64 DER")
}

fn jwks_json() -> serde_json::Value {
    serde_json::json!({ "keys": [{"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E}] })
}

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
        "exp": 9_999_999_999i64,
    });
    encode(&header, &claims, &key).expect("sign RS256")
}

async fn exec(state: &AppState, sql: String) {
    state
        .db
        .with_conn(move |c| Ok(c.execute_batch(&sql)?))
        .await
        .expect("exec");
}

/// Seed a `federation_provider` row naming `issuer`, with the slug
/// `federated_signin` expects ("up"). `allowed_origins` is the raw,
/// space-separated column value — empty means "the issuer's origin
/// alone".
async fn seed_provider(state: &AppState, issuer: &str, allowed_origins: &str) {
    let provider = uuid::Uuid::new_v4();
    exec(
        state,
        format!(
            "INSERT INTO federation_provider \
             (id, slug, display_name, issuer, client_id, enabled, allowed_origins, \
              created_at, updated_at) \
             VALUES ('{provider}', 'up', 'Up', '{issuer}', 'client', 1, '{allowed_origins}', \
             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');"
        ),
    )
    .await;
}

/// **The central test.** A discovery document whose `token_endpoint`
/// names an origin outside the allowed set (here: outside the default
/// empty-set, which is the issuer's own origin alone) must result in no
/// request being made to it — not merely a failed sign-in, which could
/// also be true after the secret was already POSTed to the wrong place.
///
/// `ValidatedDiscovery::validate` checks all three endpoints
/// unconditionally, so a bad `token_endpoint` fails the whole document
/// even though `/start` itself only reads `authorization_endpoint` — the
/// rejection happens at `/start`, before any upstream redirect and
/// before a state cookie exists, which is earlier than this property
/// strictly requires but is the stronger, correct behavior: a provider
/// whose discovery document is wrong in any way is not used at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_endpoint_outside_the_allowed_set_receives_no_request() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let token_hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = token_hits.clone();
    let rogue_token_endpoint = serve_https(move |_base| {
        axum::Router::new().route(
            "/token",
            axum::routing::post(move || {
                let hits = hits_for_route.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    axum::Json(serde_json::json!({ "access_token": "at" }))
                }
            }),
        )
    })
    .await;

    // The discovery server IS the issuer (so authorization_endpoint is
    // in-set); its token_endpoint points at the separate, out-of-set
    // rogue server above.
    let discovery_issuer = serve_https(move |base| {
        let token_endpoint = format!("{rogue_token_endpoint}/token");
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let base = base.clone();
                let token_endpoint = token_endpoint.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "issuer": base,
                        "authorization_endpoint": format!("{base}/authorize"),
                        "token_endpoint": token_endpoint,
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
    })
    .await;
    seed_provider(&state, &discovery_issuer, "").await;

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

    assert_eq!(
        token_hits.load(Ordering::SeqCst),
        0,
        "the client_secret must never have been POSTed to the rogue token_endpoint"
    );
    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, DISCOVERY_ORIGIN_REJECTED);
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_fed_state").is_none(),
        "no state cookie -- the upstream redirect never happened either"
    );
}

/// `authorization_endpoint` outside the allowed set is rejected at
/// `/start`, before the state cookie that would carry the user to the
/// upstream is even issued.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authorization_endpoint_outside_the_allowed_set_is_rejected_at_start() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let discovery_issuer = serve_https(move |base| {
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let base = base.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "issuer": base,
                        "authorization_endpoint": "https://evil.example.com/authorize",
                        "token_endpoint": "https://evil.example.com/token",
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
    })
    .await;
    seed_provider(&state, &discovery_issuer, "").await;

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

    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, DISCOVERY_ORIGIN_REJECTED);
    assert!(
        extract_set_cookie(resp.headers(), "sui_id_fed_state").is_none(),
        "no state cookie must be issued for a rejected discovery document"
    );
}

/// A provider configured with a non-empty `allowed_origins` that includes
/// a genuine second origin: the discovery document's endpoints split
/// across both are accepted, and sign-in succeeds normally.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_configured_origin_is_accepted_end_to_end() {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = Arc::new(federation_test_client(sui_id::resolver::ValidatingResolver));

    let token_hits = Arc::new(AtomicUsize::new(0));
    let hits_for_route = token_hits.clone();
    let slot = nonce_slot();
    let slot_for_token = slot.clone();
    // `iss` must be the *discovery* issuer (`validate_identity_claims`'s
    // `expected_issuer`, from the `federation_provider` row) even though
    // this route lives on the separate `second_origin` -- that split is
    // exactly the property under test.
    let discovery_issuer_cell: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));
    let discovery_issuer_for_token = discovery_issuer_cell.clone();
    let second_origin = serve_https(move |_base| {
        axum::Router::new()
            .route(
                "/jwks",
                axum::routing::get(|| async { axum::Json(jwks_json()) }),
            )
            .route(
                "/token",
                axum::routing::post(move || {
                    let hits = hits_for_route.clone();
                    let slot = slot_for_token.clone();
                    let discovery_issuer_for_token = discovery_issuer_for_token.clone();
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        let nonce = slot
                            .lock()
                            .expect("lock")
                            .take()
                            .expect("federated_signin sets the nonce before calling back");
                        let issuer = discovery_issuer_for_token
                            .lock()
                            .expect("lock")
                            .clone()
                            .expect("discovery issuer known before the token POST arrives");
                        let id_token = sign_id_token(&issuer, &nonce, "fed-sub-second-origin");
                        axum::Json(
                            serde_json::json!({ "access_token": "at", "id_token": id_token }),
                        )
                    }
                }),
            )
    })
    .await;

    let second_origin_for_discovery = second_origin.clone();
    let discovery_issuer = serve_https(move |base| {
        let token_endpoint = format!("{second_origin_for_discovery}/token");
        let jwks_uri = format!("{second_origin_for_discovery}/jwks");
        *discovery_issuer_cell.lock().expect("lock") = Some(base.clone());
        axum::Router::new().route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || {
                let base = base.clone();
                let token_endpoint = token_endpoint.clone();
                let jwks_uri = jwks_uri.clone();
                async move {
                    axum::Json(serde_json::json!({
                        "issuer": base,
                        "authorization_endpoint": format!("{base}/authorize"),
                        "token_endpoint": token_endpoint,
                        "jwks_uri": jwks_uri,
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
    })
    .await;
    let allowed = format!("{discovery_issuer} {second_origin}");
    seed_provider(&state, &discovery_issuer, &allowed).await;
    add_federation_provider_config(&mut state, &discovery_issuer);

    let outcome = federated_signin(&state, &slot).await;

    assert_eq!(
        token_hits.load(Ordering::SeqCst),
        1,
        "the real token_endpoint, on the second configured origin, was used"
    );
    // No federation_link row was seeded for this sub, so this is an
    // unknown upstream identity under the default link_only provision
    // mode -- the link flow, not a session. The property this test is
    // for is that discovery validation *passed* (unlike the central
    // test's rejection), proven by reaching the real token_endpoint and
    // not landing on a fed_error redirect.
    assert_eq!(outcome.location, "/auth/federated/link");
    assert!(outcome.session_cookie.is_none());
}
