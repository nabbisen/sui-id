//! RFC 095 M3 stage 4b — the authentication/grant matrix, exercised
//! through the real `/oauth2/authorize` and `/oauth2/token` endpoints.
//! One test per row of `metadata-validation.md`'s *Authentication/grant
//! matrix*; each asserts both `authorization_code` and `refresh_token`
//! grants, since both call the same `authorize::authenticate_client`.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use base64ct::{Base64, Encoding};
use chrono::Utc;
use sui_id::{AppState, build_router};
use sui_id_core::password::hash_password;
use sui_id_shared::ids::ClientId;
use sui_id_store::models::{ClientRow, ConsentPolicy, RegistrationSource, TokenEndpointAuthMethod};
use sui_id_store::repos::clients;

use super::common::*;
use super::r120::{REDIRECT, authorize_uri};
use tower::ServiceExt;

/// `confidential`/`secret_plain`/`auth_method`/`grant_types` are the only
/// axes this stage cares about; everything else is a fixed, uninteresting
/// value. `consent_policy: ConsentPolicy::default()` (`None`) so every
/// case reaches `/oauth2/token` without a consent screen in the way.
async fn seed_client(
    state: &AppState,
    confidential: bool,
    secret_plain: Option<&str>,
    auth_method: Option<TokenEndpointAuthMethod>,
    grant_types: Option<Vec<String>>,
) -> ClientId {
    let id = ClientId::new();
    let now = Utc::now();
    let secret_hash = match secret_plain {
        Some(s) => Some(hash_password(s).await.expect("hash secret")),
        None => None,
    };
    let row = ClientRow {
        id,
        name: "stage4b-test-client".into(),
        confidential,
        secret_hash,
        redirect_uris: vec![REDIRECT.to_string()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: ConsentPolicy::default(),
        registered_via: RegistrationSource::default(),
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        token_endpoint_auth_method: auth_method,
        grant_types,
        response_types: None,
        created_at: now,
        updated_at: now,
    };
    clients::create(&state.db, &row).await.expect("seed client");
    id
}

/// §0.2's own test fixture: writes a value outside the closed set
/// directly into the column, the same way a hand-edit or a restore from
/// a different build would. No public API can produce this -- that is
/// the point.
async fn write_unparseable_auth_method(state: &AppState, id: ClientId, raw: &str) {
    let raw = raw.to_owned();
    let id_str = id.to_string();
    state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE clients SET token_endpoint_auth_method = ?1 WHERE id = ?2",
                rusqlite::params![raw, id_str],
            )?;
            Ok(())
        })
        .await
        .expect("write unparseable auth method");
}

struct TokenResp {
    status: StatusCode,
    json: serde_json::Value,
}

/// `basic`, when given, carries *both* `client_id` and the secret, the
/// way real HTTP Basic auth does. `body` is the full `application/
/// x-www-form-urlencoded` payload, which carries `client_id` itself
/// whenever the test is not relying on the Basic header for it.
async fn post_token(state: &AppState, body: String, basic: Option<(&str, &str)>) -> TokenResp {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/oauth2/token")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some((id, secret)) = basic {
        let encoded = Base64::encode_string(format!("{id}:{secret}").as_bytes());
        builder = builder.header(header::AUTHORIZATION, format!("Basic {encoded}"));
    }
    let resp = build_router(state.clone())
        .oneshot(builder.body(Body::from(body)).expect("req"))
        .await
        .expect("token");
    let status = resp.status();
    let bytes = read_body(resp.into_body()).await;
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    TokenResp { status, json }
}

async fn authorize_get_code(state: &AppState, client_id: &str, session: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(authorize_uri(client_id, "rp-state"))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(
        resp.status().is_redirection(),
        "expected a code redirect, got {}",
        resp.status()
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("location header")
        .to_owned();
    url::Url::parse(&location)
        .expect("absolute redirect")
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .expect("code in redirect")
}

/// Mints a refresh token the "legitimate way", for a client that may be
/// confidential or public, by presenting whatever credential it actually
/// accepts (`correct_basic`/`correct_body`, mutually exclusive, both
/// `None` for a public client). Used as test setup for the refresh-grant
/// half of each row -- the channel check under test runs on the
/// *subsequent* refresh exchange, never on this minting step.
async fn mint_refresh_token(
    state: &AppState,
    session: &str,
    client_id: &str,
    correct_secret: Option<&str>,
    via_basic: bool,
) -> String {
    let code = authorize_get_code(state, client_id, session).await;
    let (verifier, _) = pkce_pair();
    let body = match (via_basic, correct_secret) {
        (true, _) | (false, None) => format!(
            "grant_type=authorization_code&code={code}&redirect_uri={}\
             &client_id={client_id}&code_verifier={verifier}",
            urlencode(REDIRECT),
        ),
        (false, Some(secret)) => format!(
            "grant_type=authorization_code&code={code}&redirect_uri={}\
             &client_id={client_id}&client_secret={secret}&code_verifier={verifier}",
            urlencode(REDIRECT),
        ),
    };
    let basic = match (via_basic, correct_secret) {
        (true, Some(secret)) => Some((client_id, secret)),
        _ => None,
    };
    let r = post_token(state, body, basic).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "minting a refresh token must succeed with correct credentials: {}",
        r.json
    );
    r.json["refresh_token"]
        .as_str()
        .expect("refresh_token in mint response")
        .to_owned()
}

// ── Row 1: `none`, Basic or any secret → Reject ─────────────────────────

#[tokio::test]
async fn row1_none_method_rejects_any_presented_secret() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        false,
        None,
        Some(TokenEndpointAuthMethod::None),
        None,
    )
    .await
    .to_string();

    // Legitimate: no secret at all still works (unchanged).
    let refresh = mint_refresh_token(&state, &session, &client_id, None, false).await;

    // Code grant: a public client presenting a secret via body is refused.
    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=spurious&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "a none-method client presenting a secret must be refused"
    );
    assert_eq!(r.json["error"], "invalid_client");

    // Refresh grant: the same client, presenting a secret via Basic.
    let body = format!("grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}",);
    let r = post_token(&state, body, Some((&client_id, "spurious"))).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "a none-method client presenting a secret on refresh must be refused"
    );
    assert_eq!(r.json["error"], "invalid_client");
}

// ── Row 2: `none`, client id only, no secret → Accept (already holds) ──

#[tokio::test]
async fn row2_none_method_with_no_secret_still_accepts() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        false,
        None,
        Some(TokenEndpointAuthMethod::None),
        None,
    )
    .await
    .to_string();

    let refresh = mint_refresh_token(&state, &session, &client_id, None, false).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);

    let body = format!("grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}");
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
}

// ── Row 3: `basic`, one Basic pair → Accept (already holds) ────────────

#[tokio::test]
async fn row3_basic_method_with_basic_pair_accepts() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-row3"),
        Some(TokenEndpointAuthMethod::ClientSecretBasic),
        None,
    )
    .await
    .to_string();

    let refresh = mint_refresh_token(&state, &session, &client_id, Some("s3cret-row3"), true).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, Some((&client_id, "s3cret-row3"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);

    let body = "grant_type=refresh_token".to_string();
    let body = format!("{body}&refresh_token={refresh}");
    let r = post_token(&state, body, Some((&client_id, "s3cret-row3"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
}

// ── Row 4: `basic`, secret in body, no header → Reject (new) ───────────

#[tokio::test]
async fn row4_basic_method_with_secret_in_body_rejects() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-row4"),
        Some(TokenEndpointAuthMethod::ClientSecretBasic),
        None,
    )
    .await
    .to_string();

    let refresh = mint_refresh_token(&state, &session, &client_id, Some("s3cret-row4"), true).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=s3cret-row4&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "client_secret_basic presented in the body must be refused"
    );
    assert_eq!(r.json["error"], "invalid_client");
    assert!(
        r.json["error_description"]
            .as_str()
            .unwrap_or_default()
            .contains("client_secret_basic"),
        "the description must name the channel: {}",
        r.json
    );

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret=s3cret-row4"
    );
    let r = post_token(&state, body, None).await;
    assert_ne!(r.status, StatusCode::OK, "refresh must refuse the same way");
    assert_eq!(r.json["error"], "invalid_client");
}

// ── Row 5: `post`, id+secret in body → Accept (already holds) ──────────

#[tokio::test]
async fn row5_post_method_with_body_credentials_accepts() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-row5"),
        Some(TokenEndpointAuthMethod::ClientSecretPost),
        None,
    )
    .await
    .to_string();

    let refresh =
        mint_refresh_token(&state, &session, &client_id, Some("s3cret-row5"), false).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=s3cret-row5&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret=s3cret-row5"
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
}

// ── Row 6: `post`, Basic present → Reject (new) ─────────────────────────

#[tokio::test]
async fn row6_post_method_with_basic_header_rejects() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-row6"),
        Some(TokenEndpointAuthMethod::ClientSecretPost),
        None,
    )
    .await
    .to_string();

    let refresh =
        mint_refresh_token(&state, &session, &client_id, Some("s3cret-row6"), false).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, Some((&client_id, "s3cret-row6"))).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "client_secret_post presented via Basic must be refused"
    );
    assert_eq!(r.json["error"], "invalid_client");
    assert!(
        r.json["error_description"]
            .as_str()
            .unwrap_or_default()
            .contains("client_secret_post"),
        "the description must name the channel: {}",
        r.json
    );

    let body = format!("grant_type=refresh_token&refresh_token={refresh}");
    let r = post_token(&state, body, Some((&client_id, "s3cret-row6"))).await;
    assert_ne!(r.status, StatusCode::OK, "refresh must refuse the same way");
    assert_eq!(r.json["error"], "invalid_client");
}

// ── Row 7: legacy/unrecorded, either channel, not both → Accept (exempt) ─

#[tokio::test]
async fn row7_unrecorded_method_accepts_either_channel() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(&state, true, Some("s3cret-row7"), None, None)
        .await
        .to_string();

    let refresh = mint_refresh_token(&state, &session, &client_id, Some("s3cret-row7"), true).await;

    // Via body.
    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=s3cret-row7&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "unrecorded + body: {}", r.json);

    // Via Basic.
    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, Some((&client_id, "s3cret-row7"))).await;
    assert_eq!(r.status, StatusCode::OK, "unrecorded + basic: {}", r.json);

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret=s3cret-row7"
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
}

// ── Row 8: any method, Basic and body both present → Reject (new) ──────

#[tokio::test]
async fn row8_both_channels_present_rejects_regardless_of_method() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-row8"),
        Some(TokenEndpointAuthMethod::ClientSecretBasic),
        None,
    )
    .await
    .to_string();

    let refresh = mint_refresh_token(&state, &session, &client_id, Some("s3cret-row8"), true).await;

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=s3cret-row8&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, Some((&client_id, "s3cret-row8"))).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "both channels present must be refused regardless of the registered method"
    );
    assert_eq!(r.json["error"], "invalid_client");

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret=s3cret-row8"
    );
    let r = post_token(&state, body, Some((&client_id, "s3cret-row8"))).await;
    assert_ne!(r.status, StatusCode::OK, "refresh must refuse the same way");
    assert_eq!(r.json["error"], "invalid_client");
}

// ── §0.2: the unparseable case fails closed, not exempt ─────────────────

#[tokio::test]
async fn an_unparseable_stored_auth_method_refuses_rather_than_exempts() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-unknown"),
        Some(TokenEndpointAuthMethod::ClientSecretBasic),
        None,
    )
    .await;
    // Mint before corrupting the column, with a channel that will be
    // wrong (body) once the method reads as unparseable rather than
    // "basic" -- the point is that it refuses either way afterward.
    let client_id_str = client_id.to_string();
    let refresh = mint_refresh_token(
        &state,
        &session,
        &client_id_str,
        Some("s3cret-unknown"),
        true,
    )
    .await;
    write_unparseable_auth_method(&state, client_id, "CLIENT_SECRET_BASIC").await;

    let code = authorize_get_code(&state, &client_id_str, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, Some((&client_id_str, "s3cret-unknown"))).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "an unparseable stored method must refuse, not silently exempt: {}",
        r.json
    );
    assert_eq!(r.json["error"], "invalid_client");

    let body = format!("grant_type=refresh_token&refresh_token={refresh}");
    let r = post_token(&state, body, Some((&client_id_str, "s3cret-unknown"))).await;
    assert_ne!(r.status, StatusCode::OK, "refresh must refuse the same way");
    assert_eq!(r.json["error"], "invalid_client");
}

// ── C1's exemption, re-verified under the new code ──────────────────────

#[tokio::test]
async fn an_unrecorded_stored_auth_method_authenticates_exactly_as_before() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(&state, true, Some("s3cret-exempt"), None, None)
        .await
        .to_string();

    let code = authorize_get_code(&state, &client_id, &session).await;
    let (verifier, _) = pkce_pair();
    let body = format!(
        "grant_type=authorization_code&code={code}&redirect_uri={}\
         &client_id={client_id}&client_secret=s3cret-exempt&code_verifier={verifier}",
        urlencode(REDIRECT),
    );
    let r = post_token(&state, body, None).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.json);
}

// ── §6: the refresh half, both gaps, independently ──────────────────────

#[tokio::test]
async fn refresh_is_refused_when_the_clients_grant_types_omit_refresh_token() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-grant"),
        Some(TokenEndpointAuthMethod::ClientSecretPost),
        Some(vec!["authorization_code".to_string()]),
    )
    .await
    .to_string();

    let refresh =
        mint_refresh_token(&state, &session, &client_id, Some("s3cret-grant"), false).await;

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id}&client_secret=s3cret-grant"
    );
    let r = post_token(&state, body, None).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "a client not registered for refresh_token must be refused: {}",
        r.json
    );
    assert_eq!(r.json["error"], "unauthorized_client");
}

#[tokio::test]
async fn refresh_is_refused_after_the_clients_scope_policy_is_narrowed() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = seed_client(
        &state,
        true,
        Some("s3cret-scope"),
        Some(TokenEndpointAuthMethod::ClientSecretPost),
        Some(vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ]),
    )
    .await;
    let client_id_str = client_id.to_string();

    let refresh = mint_refresh_token(
        &state,
        &session,
        &client_id_str,
        Some("s3cret-scope"),
        false,
    )
    .await;

    // Narrow the policy after the refresh token was issued -- directly,
    // the same repo function the admin settings form uses.
    clients::set_allowed_scopes(&state.db, client_id, "profile")
        .await
        .expect("narrow scope policy");

    let body = format!(
        "grant_type=refresh_token&refresh_token={refresh}&client_id={client_id_str}&client_secret=s3cret-scope"
    );
    let r = post_token(&state, body, None).await;
    assert_ne!(
        r.status,
        StatusCode::OK,
        "a refresh token outside the client's current scope policy must be refused: {}",
        r.json
    );
    assert_eq!(r.json["error"], "invalid_scope");
}
