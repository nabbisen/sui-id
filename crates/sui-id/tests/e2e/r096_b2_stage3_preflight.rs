//! RFC 096-B2 S3 — the preflight capability, end to end against a real
//! mock upstream (self-signed HTTPS, the same harness every other
//! federation e2e test uses).
//!
//! `preflight_provider` lives in the library crate
//! (`sui_id::federation_preflight`), with no caller yet (S4 is the
//! consumer) -- these are its only tests, since its own logic needs a
//! real mock upstream the lib's unit tests cannot reach.

use super::common::test_app;
use super::federation_fail_closed::add_federation_provider_config;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_id::federation_preflight::{PreflightError, preflight_provider};
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;

fn discovery_json(
    issuer: &str,
    token_endpoint_auth_methods_supported: &[&str],
    id_token_signing_alg_values_supported: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "jwks_uri": format!("{issuer}/jwks"),
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": token_endpoint_auth_methods_supported,
        "id_token_signing_alg_values_supported": id_token_signing_alg_values_supported,
        "subject_types_supported": ["public"],
    })
}

/// A real 2,048-bit RSA key, shared with `federation_fail_closed`: the
/// preflight now requires a key sign-in's own `select_key` would accept.
fn jwks_json() -> serde_json::Value {
    use super::federation_fail_closed::{KID, RSA_E, RSA_N};
    serde_json::json!({ "keys": [{"kty": "RSA", "kid": KID, "n": RSA_N, "e": RSA_E}] })
}

/// A mock upstream whose two routes each increment `hits` and serve a
/// caller-chosen JSON body. `discovery_from_issuer` is called once,
/// synchronously, with the real issuer `serve_https` just bound --
/// the same shape every other federation mock in this harness uses to
/// put the real issuer into its own discovery document.
async fn mock_upstream(
    hits: Arc<AtomicUsize>,
    discovery_from_issuer: impl Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    jwks_body: serde_json::Value,
) -> String {
    super::tls_mock::serve_https(move |issuer| {
        let hits_d = hits.clone();
        let hits_j = hits.clone();
        let discovery_body = discovery_from_issuer(&issuer);
        axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let hits_d = hits_d.clone();
                    let body = discovery_body.clone();
                    async move {
                        hits_d.fetch_add(1, Ordering::SeqCst);
                        axum::Json(body)
                    }
                }),
            )
            .route(
                "/jwks",
                axum::routing::get(move || {
                    let hits_j = hits_j.clone();
                    let body = jwks_body.clone();
                    async move {
                        hits_j.fetch_add(1, Ordering::SeqCst);
                        axum::Json(body)
                    }
                }),
            )
    })
    .await
}

/// `enabled: false`, a recorded policy (`id_token_algs = Some("RS256")`),
/// slug `"up"` (matching [`add_federation_provider_config`]) -- the one
/// piece every test here shares, varied by the caller afterward
/// (`client_secret_enc`, `issuer`, `id_token_algs`).
fn disabled_row(issuer: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "up".into(),
        display_name: "Preflight".into(),
        issuer: issuer.into(),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: Some("RS256".into()),
    }
}

fn with_federation_client(state: &mut sui_id::AppState) {
    state.http_client = Arc::new(Some(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    )));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_successful_preflight_returns_a_capability_with_matching_fingerprints() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let jwks_body = jwks_json();
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_body.clone(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let capability = preflight_provider(&state, provider_id)
        .await
        .expect("preflight must succeed");

    assert_eq!(capability.provider_id(), provider_id);
    assert_eq!(capability.config_version(), 0);
    assert_eq!(capability.activation_generation(), 0);

    let expected_discovery = discovery_json(&issuer, &["none"], &["RS256"]);
    let expected_metadata_fingerprint: [u8; 32] =
        Sha256::digest(serde_json::to_vec(&expected_discovery).expect("serialize")).into();
    assert_eq!(
        capability.metadata_fingerprint(),
        &expected_metadata_fingerprint
    );

    let expected_jwks_fingerprint: [u8; 32] =
        Sha256::digest(serde_json::to_vec(&jwks_body).expect("serialize")).into();
    assert_eq!(capability.jwks_fingerprint(), &expected_jwks_fingerprint);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "one discovery fetch, one jwks fetch"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_enabled_provider_is_refused_before_any_network_access() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let mut row = disabled_row(&issuer);
    row.config_version = 0;
    row.activation_generation = 0;
    let provider_id = row.id;
    federation_provider::create_enabled_for_test(&state.db, &row, None)
        .await
        .expect("seed enabled provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::NotDisabled)));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the mock"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_nonexistent_provider_is_refused_before_any_network_access() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);

    let ghost = FederationProviderId::new();
    let result = preflight_provider(&state, ghost).await;
    assert!(matches!(result, Err(PreflightError::NotFound)));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the mock"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_provider_with_no_recorded_policy_is_refused_before_any_network_access() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let mut row = disabled_row(&issuer);
    row.id_token_algs = None; // every row migration 0051 copied has this
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::PolicyNotRecorded)));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the mock"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_provider_with_no_matching_config_block_is_refused_before_any_network_access() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    // Deliberately not called: `add_federation_provider_config(&mut state, &issuer);`

    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::NotInConfiguration)));
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "no request reached the mock"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_federation_http_client_refuses_before_any_network_access() {
    let mut state = test_app();
    state.http_client = Arc::new(None);

    let row = disabled_row("https://unreachable.example");
    let provider_id = row.id;
    add_federation_provider_config(&mut state, &row.issuer);
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::Transport)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invalid_discovery_document_is_refused() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    // Missing every required field -- fails deserialization/validation,
    // regardless of the issuer.
    let issuer = mock_upstream(
        hits.clone(),
        |_issuer| serde_json::json!({ "issuer": "whatever" }),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::Discovery)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invalid_jwks_document_is_refused() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        serde_json::json!({ "not_keys": [] }),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::Jwks)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unsupported_auth_method_is_refused() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    // The policy's own `client_secret_enc` is present below, so its
    // token-auth method is `client_secret_basic` -- the document names
    // only `none`.
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let mut row = disabled_row(&issuer);
    row.client_secret_enc = Some(vec![0xAB; 48]); // present -> client_secret_basic
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, Some("a-secret"))
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::AuthMethodUnsupported)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_shared_algorithm_is_refused() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["ES256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    // The row's own `id_token_algs` is `RS256` (`disabled_row`'s
    // default); the document supports only `ES256`.
    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::NoCommonAlgorithm)));
}

/// RFC 096 `:412-413`'s re-read, proven against a real race: the mock's
/// own `/jwks` route bumps the provider's `activation_generation` in the
/// database while it serves, so by the time `preflight_provider`'s final
/// re-read runs, the row has moved. **Mutation performed separately and
/// reverted** (not committed here): skipping the re-read block lets this
/// test return `Ok` instead of `Err(ChangedDuringPreflight)` — see the
/// review package.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_provider_changed_while_the_check_was_running_is_refused() {
    let mut state = test_app();
    let provider_id = FederationProviderId::new();
    let db_for_route = state.db.clone();

    let issuer = super::tls_mock::serve_https(move |issuer| {
        let discovery_body = discovery_json(&issuer, &["none"], &["RS256"]);
        let db_for_route = db_for_route.clone();
        axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let body = discovery_body.clone();
                    async move { axum::Json(body) }
                }),
            )
            .route(
                "/jwks",
                axum::routing::get(move || {
                    let db = db_for_route.clone();
                    async move {
                        db.with_conn(move |c| {
                            Ok(c.execute(
                                "UPDATE federation_provider \
                                 SET activation_generation = activation_generation + 1 \
                                 WHERE id = ?1",
                                [provider_id.to_string()],
                            )?)
                        })
                        .await
                        .expect("bump activation_generation while serving");
                        axum::Json(jwks_json())
                    }
                }),
            )
    })
    .await;

    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);

    let mut row = disabled_row(&issuer);
    row.id = provider_id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(
        result,
        Err(PreflightError::ChangedDuringPreflight)
    ));
}

// ── S3 review fold-ins ────────────────────────────────────────────────

/// A JWKS that parses but has no key sign-in could use: the only key's
/// RSA modulus is one byte. Before the review the preflight accepted it,
/// so the provider would have been enabled and then failed every sign-in.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_jwks_with_no_usable_key_is_refused() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        serde_json::json!({ "keys": [{"kty": "RSA", "kid": "short", "n": "AQAB", "e": "AQAB"}] }),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);
    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(result, Err(PreflightError::Jwks)));
}

/// The re-read's digest comparison, on its own: the policy changes while
/// the check runs, but the version and generation do not, so only the
/// digest can notice.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_policy_changed_while_the_check_was_running_is_refused_by_its_digest() {
    let mut state = test_app();
    let provider_id = FederationProviderId::new();
    let db_for_route = state.db.clone();

    let issuer = super::tls_mock::serve_https(move |issuer| {
        let discovery_body = discovery_json(&issuer, &["none"], &["RS256"]);
        let db_for_route = db_for_route.clone();
        axum::Router::new()
            .route(
                "/.well-known/openid-configuration",
                axum::routing::get(move || {
                    let body = discovery_body.clone();
                    async move { axum::Json(body) }
                }),
            )
            .route(
                "/jwks",
                axum::routing::get(move || {
                    let db = db_for_route.clone();
                    async move {
                        db.with_conn(move |c| {
                            Ok(c.execute(
                                "UPDATE federation_provider SET client_id = 'changed' \
                                 WHERE id = ?1",
                                [provider_id.to_string()],
                            )?)
                        })
                        .await
                        .expect("change the policy, not the counters");
                        axum::Json(jwks_json())
                    }
                }),
            )
    })
    .await;

    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);
    let mut row = disabled_row(&issuer);
    row.id = provider_id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let result = preflight_provider(&state, provider_id).await;
    assert!(matches!(
        result,
        Err(PreflightError::ChangedDuringPreflight)
    ));
}

async fn audit_count(state: &sui_id::AppState) -> i64 {
    state
        .db
        .with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))?))
        .await
        .expect("count")
}

/// Dispatch §B clause 8: the preflight writes nothing, on success or on
/// failure. Counted, not argued: the audit log and the provider row are
/// compared before and after.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_preflight_writes_nothing_on_success_or_failure() {
    let mut state = test_app();
    let hits = Arc::new(AtomicUsize::new(0));
    let issuer = mock_upstream(
        hits.clone(),
        |issuer| discovery_json(issuer, &["none"], &["RS256"]),
        jwks_json(),
    )
    .await;
    with_federation_client(&mut state);
    add_federation_provider_config(&mut state, &issuer);
    let row = disabled_row(&issuer);
    let provider_id = row.id;
    federation_provider::create(&state.db, &row, None)
        .await
        .expect("seed provider");

    let snapshot = |r: &FederationProviderRow| {
        (
            r.enabled,
            r.config_version,
            r.activation_generation,
            r.validation_status,
            r.evidence_policy_digest.clone(),
            r.updated_at,
        )
    };
    let before_row = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    let before_audit = audit_count(&state).await;

    let ok = preflight_provider(&state, provider_id).await;
    assert!(ok.is_ok(), "the success case must succeed");
    let after_ok = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert_eq!(
        snapshot(&after_ok),
        snapshot(&before_row),
        "success wrote to the row"
    );
    assert_eq!(
        audit_count(&state).await,
        before_audit,
        "success wrote an audit row"
    );

    // A failure: a configured algorithm the upstream does not offer.
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE federation_provider SET id_token_algs = 'ES256' WHERE id = ?1",
                [provider_id.to_string()],
            )?)
        })
        .await
        .expect("make the check fail");
    let before_fail = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    let failed = preflight_provider(&state, provider_id).await;
    assert!(matches!(failed, Err(PreflightError::NoCommonAlgorithm)));
    let after_fail = federation_provider::get(&state.db, provider_id)
        .await
        .expect("get");
    assert_eq!(
        snapshot(&after_fail),
        snapshot(&before_fail),
        "failure wrote to the row"
    );
    assert_eq!(
        audit_count(&state).await,
        before_audit,
        "failure wrote an audit row"
    );
}
