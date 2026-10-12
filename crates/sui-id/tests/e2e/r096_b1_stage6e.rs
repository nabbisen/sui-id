//! RFC 096-B1 stage 6e — the `fed_error` surfacing map, driven against the
//! real `/admin/login` GET handler rather than asserted at the unit level
//! alone. `handlers::federation::tests` already proves
//! `FedErrorBucket::classify`'s own census and `fed_error_message`'s
//! non-reflection in isolation; this file proves the same two properties
//! reach the actual rendered page, through `LoginGetQuery` and
//! `render_login`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, nonce_slot};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use sui_id_store::models::{Role, UserRow, UserSource};
use tower::ServiceExt;

async fn login_body(fed_error: Option<&str>) -> String {
    let state = test_app();
    let uri = match fed_error {
        Some(v) => {
            use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
            format!(
                "/admin/login?fed_error={}",
                utf8_percent_encode(v, NON_ALPHANUMERIC)
            )
        }
        None => "/admin/login".to_owned(),
    };
    let resp = build_router(state)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(axum::http::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("login GET");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    std::str::from_utf8(&bytes).expect("utf8").to_owned()
}

#[tokio::test]
async fn no_fed_error_renders_no_flash_banner() {
    let body = login_body(None).await;
    assert!(!body.contains("class=\"flash"));
}

#[tokio::test]
async fn link_required_renders_the_link_required_message() {
    let body = login_body(Some("link_required")).await;
    assert!(body.contains("This sign-in could not be linked to a local account automatically."));
}

/// The oracle property (dispatch item 6's own security-relevant case),
/// through the real page: both values' rendered pages contain the exact
/// same message text.
#[tokio::test]
async fn email_collision_and_link_required_render_identical_pages() {
    let collision = login_body(Some("email_collision")).await;
    let link_required = login_body(Some("link_required")).await;
    let needle = "This sign-in could not be linked to a local account automatically.";
    assert!(collision.contains(needle));
    assert!(link_required.contains(needle));
}

#[tokio::test]
async fn unverified_email_renders_its_own_message() {
    let body = login_body(Some("unverified_email")).await;
    assert!(body.contains("Verify it with your identity provider"));
}

#[tokio::test]
async fn a_provider_or_account_level_value_renders_the_contact_admin_message() {
    for v in [
        "disabled",
        "config_drift",
        "discovery_origin",
        "signin_failed",
    ] {
        let body = login_body(Some(v)).await;
        assert!(
            body.contains("Contact an administrator."),
            "{v} did not render the contact-admin message"
        );
    }
}

/// Dispatch item 4's first half: an unrecognised value gets the generic
/// message, not a blank page and not an echo of the value itself.
#[tokio::test]
async fn an_unknown_value_renders_the_generic_message_not_a_blank_page() {
    let body = login_body(Some("not-a-real-fed-error-value")).await;
    assert!(body.contains("Sign-in could not be completed. Please try again."));
    assert!(!body.contains("not-a-real-fed-error-value"));
}

/// Dispatch item 4's second half: no reflection, in any form, including
/// attribute context. A payload shaped to break out of an HTML attribute
/// or inject a script tag must not appear anywhere in the rendered page --
/// not raw, not escaped differently, not fragment-by-fragment.
#[tokio::test]
async fn the_fed_error_value_is_never_reflected_into_the_page() {
    let payload = "\"><script>alert(document.cookie)</script>";
    let body = login_body(Some(payload)).await;
    assert!(!body.contains(payload));
    assert!(!body.contains("<script>alert"));
    assert!(!body.contains("document.cookie"));
    // The generic message still renders -- an unrecognised, payload-shaped
    // value is handled exactly like any other unrecognised value.
    assert!(body.contains("Sign-in could not be completed. Please try again."));
}

// ── R1 (2026-10-12 review): the handler-level oracle ─────────────────────
//
// The tests above feed `fed_error` values directly to the page and prove
// the *map* does not distinguish `email_collision` from `link_required`.
// They cannot see a second door onto the same property: whether the
// *handler*, under `provision_on_first_login`, ever reaches
// `fed_error=email_collision` for an email the caller cannot verify. Before
// this fix round, P2's collision check ran before P3's unverified-email
// check, so an attacker who could only present an address (never verify
// one they did not own) could still learn whether an *unverified* copy of
// it collided with a local account -- `email_collision` vs
// `unverified_email` differing by exactly that. This harness is duplicated
// from `r096_b1_stage5.rs` rather than shared (that file's own `claims()`
// doc comment notes the same choice relative to stage 4's version): each
// stage's fixture needs a different parameter (stage 5 varied `email`;
// this one additionally varies `email_verified`), and the module-private
// helpers there are not `pub(super)`.

/// Same throwaway RSA-2048 fixture key as the stage-4/5/6e e2e files.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-stage6e-fix-rsa";

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

/// Unlike `r096_b1_stage5.rs`'s `claims()`, `email_verified` is also a
/// parameter: R1's whole point is what happens when it is `false`.
fn claims_with_verification(
    issuer: &str,
    nonce: &str,
    sub: &str,
    email: &str,
    email_verified: bool,
) -> serde_json::Value {
    serde_json::json!({
        "sub": sub,
        "iss": issuer,
        "aud": "client",
        "nonce": nonce,
        "iat": chrono::Utc::now().timestamp(),
        "exp": 9_999_999_999i64,
        "email": email,
        "email_verified": email_verified,
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

/// A mock upstream signing a token for a caller-supplied `(sub, email,
/// email_verified)` triple.
async fn mock_upstream_for(
    sub: String,
    email: String,
    email_verified: bool,
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
                        let token = sign(&claims_with_verification(
                            &issuer,
                            &nonce,
                            &sub,
                            &email,
                            email_verified,
                        ));
                        axum::Json(serde_json::json!({ "access_token": "at", "id_token": token }))
                    }
                }),
            )
    })
    .await;
    (issuer, slot)
}

/// R1's central regression, at the handler level: an upstream identity
/// presenting an **unverified** email must land on `fed_error=
/// unverified_email` whether or not that email collides with a local
/// account. `assert_eq!` on the observed `fed_error`, mirroring the shape
/// `r096_b1_stage5.rs`'s `link_only_gives_the_same_result_whether_or_not_
/// the_email_collides` already got right for `link_only` — this is the
/// same property, for `provision_on_first_login`, for the check the page-
/// level map tests above cannot see.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unverified_email_renders_the_same_fed_error_whether_or_not_it_collides() {
    let mut state_colliding = base_state().await;
    let (issuer_a, slot_a) = mock_upstream_for(
        "sub-unverified-colliding".into(),
        "probe@example.com".into(),
        false,
    )
    .await;
    add_federation_provider_config(&mut state_colliding, &issuer_a);
    seed_provider_row(&state_colliding, &issuer_a).await;
    seed_unlinked_local_user_with_email(&state_colliding, "Probe@Example.com").await;
    let started_a = start(&state_colliding, &slot_a).await;
    let location_a = callback(&state_colliding, &started_a).await;

    let mut state_non_colliding = base_state().await;
    let (issuer_b, slot_b) = mock_upstream_for(
        "sub-unverified-non-colliding".into(),
        "never-seen@example.com".into(),
        false,
    )
    .await;
    add_federation_provider_config(&mut state_non_colliding, &issuer_b);
    seed_provider_row(&state_non_colliding, &issuer_b).await;
    let started_b = start(&state_non_colliding, &slot_b).await;
    let location_b = callback(&state_non_colliding, &started_b).await;

    assert_eq!(location_a, "/admin/login?fed_error=unverified_email");
    assert_eq!(location_b, "/admin/login?fed_error=unverified_email");
    assert_eq!(
        location_a, location_b,
        "an unverified email must not reveal whether it collides with a local account"
    );
}

/// The reorder must not disarm the collision denial for a **verified**
/// email: `:707-708`'s takeover block still fires once verification is no
/// longer the thing standing in its way.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_verified_colliding_email_still_denies_as_email_collision() {
    let mut state = base_state().await;
    let (issuer, slot) = mock_upstream_for(
        "sub-verified-colliding".into(),
        "collides@example.com".into(),
        true,
    )
    .await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer).await;
    seed_unlinked_local_user_with_email(&state, "Collides@Example.com").await;

    let started = start(&state, &slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=email_collision");
}
