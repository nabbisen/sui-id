//! RFC 123 D4, dispatched 2026-10-24 for `/oauth2/token`: the three
//! pre-`authenticate_client` rejections -- unknown client id, disabled or
//! deleted client, wrong secret -- must be indistinguishable in both cost
//! and response, for both grants. Response equality is asserted here;
//! the timing evidence is a separate, throwaway, `--release` measurement
//! reported in the review package, not shipped as a test.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sui_id::{AppState, build_router};
use sui_id_core::password::hash_password;
use sui_id_shared::ids::ClientId;
use sui_id_store::models::{ClientRow, ConsentPolicy, RegistrationSource, TokenEndpointAuthMethod};
use sui_id_store::repos::clients;
use tower::ServiceExt;

use super::common::*;

/// Every axis not under test is fixed to an uninteresting, valid value.
/// `authenticate_client` runs before the code/refresh-token lookup in both
/// `exchange_code` and `exchange_refresh`, so none of these three cases
/// needs a real code or refresh token -- the request never reaches that
/// far.
async fn seed_client(
    state: &AppState,
    confidential: bool,
    secret_plain: Option<&str>,
    is_disabled: bool,
    is_deleted: bool,
) -> ClientId {
    let id = ClientId::new();
    let now = Utc::now();
    let secret_hash = match secret_plain {
        Some(s) => Some(hash_password(s).await.expect("hash secret")),
        None => None,
    };
    let row = ClientRow {
        id,
        name: "r123-d4-test-client".into(),
        confidential,
        secret_hash,
        redirect_uris: vec!["https://rp.test/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled,
        is_deleted,
        consent_policy: ConsentPolicy::default(),
        registered_via: RegistrationSource::default(),
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        token_endpoint_auth_method: Some(TokenEndpointAuthMethod::ClientSecretPost),
        grant_types: None,
        response_types: None,
        created_at: now,
        updated_at: now,
    };
    clients::create(&state.db, &row).await.expect("seed client");
    id
}

struct TokenResp {
    status: StatusCode,
    json: serde_json::Value,
}

async fn post_token(state: &AppState, body: String) -> TokenResp {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/token")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .expect("req"),
        )
        .await
        .expect("token");
    let status = resp.status();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    TokenResp { status, json }
}

/// The three bodies that drive each of the pre-authentication branches,
/// for one grant type. `code`/`refresh_token` are placeholders -- never
/// read, since authentication runs first.
fn code_grant_bodies(unknown: &str, disabled: &str, wrong_secret: &str) -> [String; 3] {
    [
        format!(
            "grant_type=authorization_code&code=x&redirect_uri={}&client_id={unknown}\
             &client_secret=whatever&code_verifier=x",
            urlencode("https://rp.test/cb"),
        ),
        format!(
            "grant_type=authorization_code&code=x&redirect_uri={}&client_id={disabled}\
             &client_secret=correct-secret&code_verifier=x",
            urlencode("https://rp.test/cb"),
        ),
        format!(
            "grant_type=authorization_code&code=x&redirect_uri={}&client_id={wrong_secret}\
             &client_secret=the-wrong-one&code_verifier=x",
            urlencode("https://rp.test/cb"),
        ),
    ]
}

fn refresh_grant_bodies(unknown: &str, disabled: &str, wrong_secret: &str) -> [String; 3] {
    [
        format!(
            "grant_type=refresh_token&refresh_token=x&client_id={unknown}\
             &client_secret=whatever"
        ),
        format!(
            "grant_type=refresh_token&refresh_token=x&client_id={disabled}\
             &client_secret=correct-secret"
        ),
        format!(
            "grant_type=refresh_token&refresh_token=x&client_id={wrong_secret}\
             &client_secret=the-wrong-one"
        ),
    ]
}

#[tokio::test]
async fn pre_authentication_failures_are_byte_identical_responses_code_grant() {
    let state = test_app();
    let unknown = ClientId::new().to_string();
    let disabled = seed_client(&state, true, Some("correct-secret"), true, false)
        .await
        .to_string();
    let wrong_secret = seed_client(&state, true, Some("correct-secret"), false, false)
        .await
        .to_string();

    let bodies = code_grant_bodies(&unknown, &disabled, &wrong_secret);
    let mut responses = Vec::new();
    for body in bodies {
        let r = post_token(&state, body).await;
        responses.push(r);
    }

    for r in &responses {
        assert_eq!(
            r.status,
            StatusCode::UNAUTHORIZED,
            "every pre-authentication failure must be the same status: {:?}",
            r.json
        );
    }
    let first = &responses[0].json;
    for (i, r) in responses.iter().enumerate() {
        assert_eq!(
            r.json["error"], first["error"],
            "case {i}: error code must match the first case"
        );
        assert_eq!(
            r.json["error_description"], first["error_description"],
            "case {i}: error_description must match the first case, byte-for-byte"
        );
    }
    assert_eq!(first["error"], "invalid_client");
}

#[tokio::test]
async fn pre_authentication_failures_are_byte_identical_responses_refresh_grant() {
    let state = test_app();
    let unknown = ClientId::new().to_string();
    let disabled = seed_client(&state, true, Some("correct-secret"), true, false)
        .await
        .to_string();
    let wrong_secret = seed_client(&state, true, Some("correct-secret"), false, false)
        .await
        .to_string();

    let bodies = refresh_grant_bodies(&unknown, &disabled, &wrong_secret);
    let mut responses = Vec::new();
    for body in bodies {
        let r = post_token(&state, body).await;
        responses.push(r);
    }

    for r in &responses {
        assert_eq!(
            r.status,
            StatusCode::UNAUTHORIZED,
            "every pre-authentication failure must be the same status: {:?}",
            r.json
        );
    }
    let first = &responses[0].json;
    for (i, r) in responses.iter().enumerate() {
        assert_eq!(
            r.json["error"], first["error"],
            "case {i}: error code must match the first case"
        );
        assert_eq!(
            r.json["error_description"], first["error_description"],
            "case {i}: error_description must match the first case, byte-for-byte"
        );
    }
    assert_eq!(first["error"], "invalid_client");
}

/// A deleted client is a distinct real-world state from a disabled one;
/// confirm it collapses into the same response as the other two, not
/// just that "disabled" does.
#[tokio::test]
async fn a_deleted_client_also_gets_the_identical_response() {
    let state = test_app();
    let deleted = seed_client(&state, true, Some("correct-secret"), false, true)
        .await
        .to_string();
    let wrong_secret = seed_client(&state, true, Some("correct-secret"), false, false)
        .await
        .to_string();

    let deleted_body = format!(
        "grant_type=authorization_code&code=x&redirect_uri={}&client_id={deleted}\
         &client_secret=correct-secret&code_verifier=x",
        urlencode("https://rp.test/cb"),
    );
    let wrong_secret_body = format!(
        "grant_type=authorization_code&code=x&redirect_uri={}&client_id={wrong_secret}\
         &client_secret=the-wrong-one&code_verifier=x",
        urlencode("https://rp.test/cb"),
    );

    let r_deleted = post_token(&state, deleted_body).await;
    let r_wrong = post_token(&state, wrong_secret_body).await;

    assert_eq!(r_deleted.status, r_wrong.status);
    assert_eq!(r_deleted.json["error"], r_wrong.json["error"]);
    assert_eq!(
        r_deleted.json["error_description"],
        r_wrong.json["error_description"]
    );
    assert_eq!(r_deleted.json["error"], "invalid_client");
}

/// RFC 123 D4's equalisation has no shipped guard — a timing property
/// cannot be asserted precisely without flakiness, and both this round's
/// benchmark and RFC 123's own were throwaway. So a future edit removing
/// the dummy from any one branch is caught by nothing: measured
/// 2026-10-24, deleting it from `exchange_refresh`'s unknown-client arm
/// left all 595 e2e tests and every lib suite green.
///
/// **A floor is assertable where a precise cost is not.** The equalised
/// branches run one real Argon2id verification — ~37 ms in release,
/// roughly twenty times that in the debug profile `cargo test` uses — and
/// a branch that skipped it returned in ~150 µs. A floor of 10 ms sits
/// more than three times under the release cost and sixty times over the
/// unequalised one, and is one-sided: a slow or loaded runner makes this
/// pass, never fail. It is a tripwire for a removed dummy, not a
/// measurement.
///
/// Added by the architect during review. Both grants, because the round's
/// own mutation covered only the code grant and the gap was on the other
/// side.
#[tokio::test]
async fn every_pre_authentication_failure_still_costs_a_verification() {
    const FLOOR: std::time::Duration = std::time::Duration::from_millis(10);

    for (grant, body_for) in [
        (
            "authorization_code",
            code_grant_bodies as fn(&str, &str, &str) -> [String; 3],
        ),
        (
            "refresh_token",
            refresh_grant_bodies as fn(&str, &str, &str) -> [String; 3],
        ),
    ] {
        let state = test_app();
        let unknown = ClientId::new().to_string();
        let disabled = seed_client(&state, true, Some("correct-secret"), true, false)
            .await
            .to_string();
        let wrong_secret = seed_client(&state, true, Some("correct-secret"), false, false)
            .await
            .to_string();

        for (case, body) in ["unknown", "disabled", "wrong secret"].iter().zip(body_for(
            &unknown,
            &disabled,
            &wrong_secret,
        )) {
            let started = std::time::Instant::now();
            let r = post_token(&state, body).await;
            let elapsed = started.elapsed();
            assert_eq!(
                r.status,
                StatusCode::UNAUTHORIZED,
                "{grant}/{case}: must be a pre-authentication rejection, not a rate limit"
            );
            assert!(
                elapsed >= FLOOR,
                "{grant}/{case}: returned in {elapsed:?}, under the {FLOOR:?} floor -- \
                 this branch is not running the dummy verification RFC 123 D4 requires"
            );
        }
    }
}
