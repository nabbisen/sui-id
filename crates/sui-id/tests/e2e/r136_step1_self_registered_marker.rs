//! RFC 136 step 1, D2 — the self-registered marker.
//!
//! A marker on `registered_via = 'dynamic'` only; an administrator-created
//! client gets nothing. Both halves are tested together in each view, so a
//! marker rendered unconditionally would fail here even though a
//! presence-only test would pass.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::RegistrationTokenId;
use sui_id_store::repos::client_registration_token::RegistrationTokenRow;

use super::common::*;
use tower::ServiceExt;

const REDIRECT: &str = "https://rp.test/cb";
// `Locale::ALL` lists `Ja` before `En` (sui_id_i18n::lib.rs), so `test_app()`'s
// unauthenticated/default negotiation lands on Japanese -- confirmed by
// running this test and reading the actual rendered body rather than
// assuming English, matching this session's "measure, don't assume"
// convention. `status_self_registered`'s Japanese value (locale/ja.rs).
const MARKER_TEXT: &str = "自己登録";

fn sha256_hex(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seed_registration_token(state: &AppState) -> String {
    let plaintext = random_registration_token_plaintext();
    let row = RegistrationTokenRow {
        id: RegistrationTokenId::new(),
        token_hash: sha256_hex(&plaintext),
        max_uses: 1,
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

/// Register a client dynamically (RFC 7591) and return its id.
async fn register_dynamic_client(state: &AppState) -> String {
    let token = seed_registration_token(state).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/register")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"redirect_uris":["{REDIRECT}"],"client_name":"Dynamic RP"}}"#
                )))
                .expect("req"),
        )
        .await
        .expect("register");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = read_body(resp.into_body()).await;
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    body["client_id"].as_str().expect("client_id").to_owned()
}

async fn get_html(state: &AppState, session: &str, uri: String) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("get");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The list page shows the marker for the dynamic client and not for the
/// admin-created one -- both halves, in the same listing.
#[tokio::test]
async fn marker_on_the_list_distinguishes_dynamic_from_admin_created() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (_admin_client_id, _secret) = create_client(&state, &session).await;
    register_dynamic_client(&state).await;

    let html = get_html(&state, &session, "/admin/clients".to_string()).await;
    let marker_count = html.matches(MARKER_TEXT).count();
    assert_eq!(
        marker_count, 1,
        "exactly one row (the dynamic client) must carry the marker; got {marker_count} in:\n{html}"
    );
}

/// The edit/detail view shows the marker for a dynamically registered
/// client.
#[tokio::test]
async fn marker_is_present_on_the_detail_view_for_a_dynamic_client() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let client_id = register_dynamic_client(&state).await;

    let html = get_html(&state, &session, format!("/admin/clients/{client_id}/edit")).await;
    assert!(
        html.contains(MARKER_TEXT),
        "detail view for a dynamic client must show the marker"
    );
}

/// The edit/detail view shows no marker for an administrator-created
/// client -- without this test, a marker rendered unconditionally would
/// have passed the previous one alone.
#[tokio::test]
async fn marker_is_absent_on_the_detail_view_for_an_admin_created_client() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;

    let html = get_html(&state, &session, format!("/admin/clients/{client_id}/edit")).await;
    assert!(
        !html.contains(MARKER_TEXT),
        "detail view for an admin-created client must not show the marker"
    );
}
