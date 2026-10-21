//! RFC 095 M3 stage 4a — persist the three fields, exercised through the
//! real `/oauth2/register` and `/oauth2/token` endpoints.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::RegistrationTokenId;
use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

use super::common::*;
use tower::ServiceExt;

fn sha256_hex(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seed_token(state: &AppState) -> String {
    let plaintext = random_registration_token_plaintext();
    let row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&plaintext),
        max_uses: 10,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    sui_id_store::repos::client_registration_token::create(&state.db, &row)
        .await
        .expect("seed registration token");
    plaintext
}

struct RegResult {
    status: StatusCode,
    json: serde_json::Value,
}

async fn register_raw(state: &AppState, token: &str, body: &str) -> RegResult {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("register");
    let status = resp.status();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    RegResult { status, json }
}

/// §4: stage 2 already validated and echoed these three; this stage adds
/// storage behind the same values. Checked against the row itself, not
/// just the response — the same shape stage 3 used for `redirect_uris`.
#[tokio::test]
async fn the_registration_response_matches_what_was_stored() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body = r#"{"client_name":"Example","scope":"openid offline_access",
                    "token_endpoint_auth_method":"client_secret_post",
                    "grant_types":["authorization_code","refresh_token"],
                    "response_types":["code"],
                    "redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);

    let client_id: sui_id_shared::ids::ClientId = r.json["client_id"]
        .as_str()
        .expect("client_id")
        .parse()
        .expect("valid client id");
    let row = sui_id_store::repos::clients::get(&state.db, client_id)
        .await
        .expect("client row exists");

    assert_eq!(
        row.token_endpoint_auth_method,
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretPost)
    );
    assert_eq!(r.json["token_endpoint_auth_method"], "client_secret_post");

    assert_eq!(
        row.grant_types,
        Some(vec![
            "authorization_code".to_string(),
            "refresh_token".to_string()
        ])
    );
    assert_eq!(
        r.json["grant_types"],
        serde_json::json!(["authorization_code", "refresh_token"])
    );

    assert_eq!(row.response_types, Some(vec!["code".to_string()]));
    assert_eq!(r.json["response_types"], serde_json::json!(["code"]));
}

/// §4: a registration that omits all three still gets stage 2's defaults
/// echoed, and now also stored — not left `NULL` the way an admin-created
/// or pre-stage-4a client is. Absence of the member in the request must
/// not read as "unrecorded".
#[tokio::test]
async fn omitting_the_three_fields_still_stores_stage_twos_defaults() {
    let state = test_app();
    let token = seed_token(&state).await;
    let body =
        r#"{"client_name":"Example","scope":"openid","redirect_uris":["https://rp.test/cb"]}"#;
    let r = register_raw(&state, &token, body).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.json);

    let client_id: sui_id_shared::ids::ClientId = r.json["client_id"]
        .as_str()
        .expect("client_id")
        .parse()
        .expect("valid client id");
    let row = sui_id_store::repos::clients::get(&state.db, client_id)
        .await
        .expect("client row exists");

    assert_eq!(
        row.token_endpoint_auth_method,
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretBasic),
        "the default must be stored, not left unrecorded"
    );
    assert_eq!(
        row.grant_types,
        Some(vec!["authorization_code".to_string()])
    );
    assert_eq!(row.response_types, Some(vec!["code".to_string()]));
}

/// §6, the behavioural half: "4a changes no authentication behaviour."
/// `create_client` (admin path) now stores `None` for all three fields —
/// exactly the class a pre-0049 row is left in after the migration, since
/// nobody asked either of them. This is the faithful, reachable proxy for
/// "a database created before 0049, migrated, still serves /oauth2/token
/// exactly as before": the actual migrated-file scenario needs
/// `sui_id_store::migrations::run_up_to`, `pub(crate)` to that crate only,
/// so it is not reachable from this one. What is asserted here is the
/// property that matters — an unrecorded auth method authenticates
/// unchanged — not a literal replay of the migration.
#[tokio::test]
async fn a_client_with_no_recorded_auth_method_still_completes_the_token_endpoint() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, client_secret) = create_client(&state, &session).await;

    let row: sui_id_shared::ids::ClientId = client_id.parse().expect("client id");
    let row = sui_id_store::repos::clients::get(&state.db, row)
        .await
        .expect("client row exists");
    assert_eq!(
        row.token_endpoint_auth_method, None,
        "an admin-created client must be unrecorded, same as a pre-0049 row"
    );

    let (verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&code_challenge={challenge}&code_challenge_method=S256"
    );
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(resp.status().is_redirection(), "expected a code redirect");
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    let code = url::Url::parse(&location)
        .expect("absolute redirect")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect");

    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &client_id={client_id}&client_secret={client_secret}&code_verifier={verifier}"
    );
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
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "an unrecorded-auth-method confidential client must still authenticate \
         at the token endpoint -- 4a adds no enforcement"
    );
}
