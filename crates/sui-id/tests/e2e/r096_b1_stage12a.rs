//! RFC 096-B1 stage 12a, §2: the sealed `sui_id_fed_state` cookie's own
//! `ct_eq` comparison is gone; the callback now finds its attempt row by
//! hashing the *query's* `state` value directly. The property that must
//! survive the collapse is RFC 096 `:714-715`'s own CSRF guarantee -- a
//! `state` an attacker supplies (wrong, replayed, or simply never issued)
//! must find no row, exactly as a `ct_eq` mismatch used to refuse before
//! any row was ever looked up.
//!
//! Also item 3 of the dispatch's own "what to return": a source-scope
//! check that the legacy decode and the sealed-state cookie cannot come
//! back without this file's own test noticing.

use super::common::*;
use super::federation_fail_closed::{add_federation_provider_config, linked_user, mock_upstream};
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::{AppState, build_router};
use tower::ServiceExt;

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

/// Drives `/callback` with an explicit `state` query value, independent of
/// whatever `start` actually returned -- this is the one thing a test in
/// this file needs to control that `federation_fail_closed::federated_signin`
/// does not let a caller override.
async fn callback_with_state(
    state: &AppState,
    presented_state: &str,
    browser_binding_cookie: &str,
) -> axum::http::Response<Body> {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/federated/callback?code=the-code&state={presented_state}"
                ))
                .header(
                    header::COOKIE,
                    format!("sui_id_fed_browser_binding={browser_binding_cookie}"),
                )
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("callback")
}

async fn attempt_status(state: &AppState, state_param: &str) -> String {
    let decoded = percent_encoding::percent_decode_str(state_param)
        .decode_utf8()
        .expect("state is valid UTF-8")
        .into_owned();
    state
        .db
        .with_read(move |read| {
            Ok(read
                .prepare(
                    "SELECT status FROM federation_login_attempt \
                     WHERE state_sha256 = ?1",
                )?
                .query_row([sha2::Sha256::digest(decoded.as_bytes()).to_vec()], |r| {
                    r.get::<_, String>(0)
                })?)
        })
        .await
        .expect("attempt row for this state")
}

use sha2::Digest;

async fn base_state() -> AppState {
    let mut state = test_app();
    complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    ));
    state
}

/// A `state` value this harness never issued, hashed to something the
/// attempt row's own `state_sha256` cannot possibly equal (RFC 096
/// `:587-588`'s raw SHA-256 column makes a collision on a real request
/// astronomically unlikely regardless, but this value is also simply never
/// the real one, independent of hash-collision odds).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_state_finds_no_attempt_row_and_the_real_one_stays_pending() {
    let (issuer, nonce_slot) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;

    let started = start(&state, &nonce_slot).await;

    let resp = callback_with_state(
        &state,
        "a-state-value-this-harness-never-issued",
        &started.browser_binding_cookie,
    )
    .await;

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login?fed_error=state_mismatch");

    // The real attempt -- found by its own, real `state_param` -- was
    // never claimed. The wrong `state` found no row to touch, rather than
    // finding and disturbing the right one under a wrong key.
    assert_eq!(
        attempt_status(&state, &started.state_param).await,
        "pending"
    );
}

/// An empty `state` (no query parameter at all) is the same refusal, not a
/// panic or a different code path: `q.state.as_deref().unwrap_or("")`
/// hashes to a value with no row either.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_state_is_refused_the_same_way() {
    let (issuer, nonce_slot) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot).await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/federated/callback?code=the-code")
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

    assert!(resp.status().is_redirection());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(location, "/admin/login?fed_error=state_mismatch");
    assert_eq!(
        attempt_status(&state, &started.state_param).await,
        "pending"
    );
}

/// The real `state`, presented back unmodified, still finds and claims the
/// row -- the collapse did not also make the legitimate case refuse.
/// Exercised end-to-end elsewhere in this suite (every stage4/5/6c/7 test
/// that completes a sign-in), restated here as the direct contrast to the
/// two refusals above, in the same file and against the same fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_real_state_still_finds_and_claims_the_row() {
    let (issuer, nonce_slot) = mock_upstream().await;
    let mut state = base_state().await;
    add_federation_provider_config(&mut state, &issuer);
    linked_user(&state, &issuer).await;
    let started = start(&state, &nonce_slot).await;

    let resp = callback_with_state(
        &state,
        &started.state_param,
        &started.browser_binding_cookie,
    )
    .await;

    assert!(resp.status().is_redirection());
    // Past `claim`, whatever this fixture's own provider/link shape sends
    // it on to next -- not `pending`, which is the one status this test
    // must rule out to show the real state was actually found.
    assert_ne!(
        attempt_status(&state, &started.state_param).await,
        "pending"
    );
}

// ── Source-scope: the two retired paths cannot come back unnoticed ─────────

/// RFC 096-B1 stage 12a, item 3 of the dispatch's own "what to return": a
/// test that fails if either removed path reappears, in the shape of
/// `r096_b1_stage10b_f03_scope.rs`'s own source scan. `decode_id_token_
/// claims`/`IdTokenClaims` (the trust-on-TLS decode, §1) and `seal_state`/
/// `unseal_state`/`FedState` (the sealed-state cookie, §2) are each
/// forbidden names: none of the four should still be reachable from
/// `crates/sui-id/src/`, since the verified pipeline and the durable
/// attempt row are now the sole source of what each one used to carry.
/// `federation_state` (the module `seal_state`/`unseal_state`/`FedState`
/// lived in) is checked too: the module itself is deleted, so even a
/// `mod federation_state;` declaration coming back is the regression this
/// test exists to catch, not only a call to one of its functions.
#[test]
fn neither_retired_path_is_reachable_from_handler_source() {
    // Word-boundary matched (not a bare substring search): `IdTokenClaims`
    // bare would also match inside `VerifiedIdTokenClaims`, the type that
    // *replaced* it and that most of 096-A's own modules legitimately
    // name throughout. A non-identifier character (or the string's own
    // edge) on both sides is what tells the two apart.
    const FORBIDDEN_NAMES: &[&str] = &[
        "decode_id_token_claims",
        "IdTokenClaims",
        "seal_state",
        "unseal_state",
        "FedState",
        "federation_state",
    ];
    fn is_ident_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }
    fn names_as_identifier(text: &str, name: &str) -> bool {
        let bytes = text.as_bytes();
        let mut start = 0;
        while let Some(rel) = text[start..].find(name) {
            let idx = start + rel;
            let before_ok = idx == 0 || !is_ident_byte(bytes[idx - 1]);
            let after = idx + name.len();
            let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
            if before_ok && after_ok {
                return true;
            }
            start = idx + 1;
        }
        false
    }

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .expect("crates/sui-id/src directory");
    let mut files = Vec::new();
    rs_files(&dir, &mut files);
    assert!(
        !files.is_empty(),
        "expected to scan at least one .rs file under sui-id/src"
    );

    let mut offenders = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("read .rs file");
        for name in FORBIDDEN_NAMES {
            if names_as_identifier(&text, name) {
                offenders.push(format!("{} (names `{name}`)", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "RFC 096-B1 stage 12a retired the trust-on-TLS decode and the sealed-state \
         cookie; the verified pipeline (`verify_id_token`/`identity_resolution`) and \
         the durable `federation_login_attempt` row are now the sole source of \
         identity and CSRF state. None of {FORBIDDEN_NAMES:?} should be reachable \
         from production source any longer. Found: {offenders:#?}"
    );
}

fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
