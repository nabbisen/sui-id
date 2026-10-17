//! RFC 096-B1 stage 6b — the converted `emit_must_attempt` site, driven
//! end to end. RFC 094 `:292-298` / A6 (`:120`): a failed audit emission
//! must never suppress the primary denial, and the failure must still be
//! logged. `repos::audit::tests` proves `emit_must_attempt` itself
//! returns `Failed` and never reaches the database on a genuine append
//! failure; this file proves the *caller* — `federated_callback`'s P2
//! collision block — still redirects correctly when that happens, and
//! that the structured telemetry really reaches the log on the live
//! path, not just inside the function's own unit test.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, nonce_slot};
use axum::body::Body;
use axum::http::{Method, Request, header};
use base64ct::{Base64, Encoding};
use jsonwebtoken::{EncodingKey, Header as JwtHeader, encode};
use std::io::Write;
use std::sync::{Arc, Mutex};
use sui_id::{AppState, build_router};
use sui_id_store::models::{Role, UserRow, UserSource};
use tower::ServiceExt;

/// Same throwaway RSA-2048 fixture key as the other stage-4/5/6a e2e files.
const RSA_DER_B64: &str = "MIIEowIBAAKCAQEA7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg+R86BmwZ+3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6+bvgQsosJM9EHpPgcebr/nbD/OVlA/VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp/iujUqwbe1ntsnEFhEBMBGKgHAWp/DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD+lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQIDAQABAoIBAGOUa5wTkoKfQSpRyx6M2g0tinI5wKR7eF1zgnvycV1b9jJzHF1eEOvKxnbvUYVa08l96Wi2geHnlQ+Y4y9n4VayBZgbo82dZ7NoBSzgFS4bUafK3UPAmAo3oz6vVG6h0e1pL6Jttw606MoSBqHg+0s6B/IhBATaC8y8gzW2xuSNJ6HEgSw601s62it2rZ97NHxkwh84h29mUmVp91tbskqxdo6V6oXLqMq2Ua2Y+MJEf4268pqJIg3ZlPtS3H8zgbsJYanCseHUD54acbV4JIne+6dcOp7YmsTjbzXzgoB4jCOSaZMYl/aJuUzp/OqnbrI+Kl5OcZllEAYZGrvCgqsCgYEA+U/c0ZON56s5ktA8vCe4OHkUyhLiQQW4fNvuW6uhWJ2pm4nCyof/Gc26CKm7wHSzZtiXThkMD/Z668A9hXHdSmb0kNwxVzxdGbCmHUndwQ2JNfktvwis7u6Gd/PmracNDVn4n7S1w9PCHiLK6xY2uynI8FIQkk7q84HK6DyYmD8CgYEA813uXSUEkbBq7/5S7DEh/vI9WlQsxyymA8uzkgZFHW4Ac5EBy+HC6MAsi5aNccV0mUj2HjHnMxlMlafOKzYO61VKOEGO4NFP2n1dRb+R/YBCRwfemAf4xfk+kVPz2HlhEUwHTjV81bJ/pJPNHaJxH09G4/bN/qVVz6Bv/COaoysCgYEAvSZESJUEYrHbunFWwwH3mJD0nuN42RA4CjLqQo6SmSL1HVaFfRd1CeS1sgDku31O50aIdO434pyEYfy2MFpVJC+8eXM11BOuJuGJBkuWfPOCGHr2pCs22QgK6VMYvsMw+eI66SA3j11Ht4l6HqX53EI1e28nt3k8dIcSpOPkeg0CgYA5iS1/a+8GmpTNpGzqVjtZUN/caSYk+JNPNmt/zGeuq4ED0XaBQyCXckeVwMQz76C/VJaLUPT+Ca8neoKtiJxCWumvHyCuWg3s89KHWOEk85u3u06O1uOjumdmaFiwBxJByp23icG3q/mtaRwHM45W/qEd6A2PdHszGRUgoTI//QKBgGdwL/X9/tYbnMYbybQ40KaJY5D3iuxSR47OnA8U1yyY/hUtYOihuqSawH6v2ISOScDiOcQgc+Cny7JQYI6DLzhYw7DhOiSEeQru6hfmSrumkObpztRDTAANpt+CiqLD8k22ljVB6yyFH9DDgeW9sENEfEVeY+N0k32Q0+suXZWW";
const RSA_N: &str = "7QJIwRyNWtEBgb7B0dIy5t7ucLXgsuoLIx5b6k8oh2DBXVwNq9Gg-R86BmwZ-3m99zrkOLFgGmrXghksq6veRof0OyjYjzk9jxWR6-bvgQsosJM9EHpPgcebr_nbD_OVlA_VV2QoTClUhjhpdx2Ip7tXtKDNx9F7wZUOGeRr0kjtmAteekgaUHlfHReeeZ0ez4znk4ANtGHNFp_iujUqwbe1ntsnEFhEBMBGKgHAWp_DkmtKt1ex0VUAQBlpMJcGUhGi2vkjFrUKsuWoXUytAXlfOoyzPxHbKt5gWyzNgBCryeVxfMD-lcQjBxmdhcV0NhDTNxik7ptDlXM3iXevlQ";
const RSA_E: &str = "AQAB";
const KID: &str = "e2e-stage6b-rsa";

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

/// Same mechanism as `repos::audit::tests::break_audit_log` (the store
/// crate's own unit test) and `tests/e2e/r102_stage4.rs`'s helper of the
/// same name: a real trigger makes the `INSERT` fail, rather than a
/// simulated flag — `emit_must_attempt`'s call to `audit::append` has
/// nothing to distinguish this from a genuine database fault.
async fn break_audit_log(state: &AppState) {
    exec(
        state,
        "CREATE TRIGGER stage6b_reject_audit BEFORE INSERT ON audit_log \
         BEGIN SELECT RAISE(ABORT, 'stage6b test: audit_log insert rejected'); END;"
            .into(),
    )
    .await;
}

struct Started {
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
    // RFC 096-B1 stage 6c: round-tripped from `/start`'s own response,
    // or every callback here refuses as `fed_error=binding_mismatch`.
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
                        "sui_id_fed_browser_binding={}",
                        started.browser_binding_cookie
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

// ── Log capture — thread-local, so the test runs on the default,
// current-thread `#[tokio::test]` runtime (no `flavor = "multi_thread"`):
// the handler must run on *this* thread for the capturing subscriber to
// see its log line. Same reasoning and same shape as
// `tests/e2e/r102_stage4.rs`'s own `capture()`. ──────────────────────────

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("lock").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn capture() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (captured, guard)
}

fn logged(captured: &Captured) -> String {
    String::from_utf8_lossy(&captured.0.lock().expect("lock")).into_owned()
}

/// RFC 094 A6 (`:120`), driven end to end: the audit log is broken before
/// the request, so `emit_must_attempt`'s own `audit::append` call fails
/// for real. The denial must still happen — the point of building this on
/// Class-B rather than Class-A — and the failure must still be logged,
/// which is `emit_must_attempt`'s own unconditional telemetry, not
/// anything this handler does to compensate.
#[tokio::test]
async fn the_denial_still_happens_and_the_failure_is_logged_when_the_emission_fails() {
    let mut state = base_state().await;
    let (issuer, slot) =
        mock_upstream_for("sub-collision".into(), "collides@example.com".into()).await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer, "provision_on_first_login").await;
    seed_unlinked_local_user_with_email(&state, "Collides@Example.com").await;
    break_audit_log(&state).await;
    let (captured, _guard) = capture();

    let started = start(&state, &slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(
        location, "/admin/login?fed_error=email_collision",
        "A6: a failed audit emission must never suppress the denial"
    );
    let log = logged(&captured);
    assert!(
        log.contains("must-attempt emission: audit append failed"),
        "no telemetry line in captured log:\n{log}"
    );
    assert!(
        log.contains("auth.federation.takeover_blocked"),
        "telemetry line present but doesn't name the event:\n{log}"
    );
}

/// The ordinary path, for contrast: nothing broken, the same denial, and
/// the audit row genuinely lands — proven by reading it back, not merely
/// by the redirect (which would look identical either way).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_denial_happens_and_the_event_is_recorded_when_nothing_is_broken() {
    let mut state = base_state().await;
    let (issuer, slot) =
        mock_upstream_for("sub-collision-2".into(), "collides2@example.com".into()).await;
    add_federation_provider_config(&mut state, &issuer);
    seed_provider_row(&state, &issuer, "provision_on_first_login").await;
    seed_unlinked_local_user_with_email(&state, "Collides2@Example.com").await;

    let started = start(&state, &slot).await;
    let location = callback(&state, &started).await;

    assert_eq!(location, "/admin/login?fed_error=email_collision");

    let rows = sui_id_store::repos::audit::recent(&state.db, 10)
        .await
        .expect("read audit rows");
    let row = rows
        .iter()
        .find(|r| r.action == "auth.federation.takeover_blocked")
        .expect("the event was recorded");
    let note = row.note.as_deref().unwrap_or_default();
    assert!(
        note.starts_with("provider_id=") && note.ends_with(" reason=email_collision"),
        "unexpected note shape: {note:?}"
    );
}
