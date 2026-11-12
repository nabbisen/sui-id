//! RFC 094 M2b wave 3 / RFC 096-B2 S2b — federation-provider
//! configuration writes: C16 (create), C23 (trust-policy replacement,
//! retiring C24), C25 (disable on removal from configuration) and C26
//! (display-name update). All four are startup provisioning, not an
//! administrator action: no authenticated actor, `system_principal:
//! permitted`, the same shape C22 uses.

use super::*;
use sui_id_shared::ids::FederationProviderId;

fn sample_row(slug: &str) -> crate::models::FederationProviderRow {
    let now = Utc::now();
    crate::models::FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: format!("https://{slug}.example"),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        // RFC 096-B2 S2a: these C24 (allowed-origins) tests only ever
        // seed through the test-only `create()`, which still writes
        // `enabled` verbatim -- `false` here, since C24 has nothing to
        // do with enablement and an enabled row with none of migration
        // 0051's required evidence would fail its own `CHECK`.
        enabled: false,
        allowed_origins: "https://a.example https://b.example".into(),
        config_version: 0,
        activation_generation: 0,
        validation_status: crate::models::ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: None,
    }
}

/// RFC 096-B2 S2a Part C: what the C16 tests below exercise now --
/// `create_federation_provider` takes a `NewFederationProvider`, which
/// has no `enabled` field at all (the type fact this stage adds).
fn sample_new_provider(slug: &str) -> crate::repos::federation_provider::NewFederationProvider {
    let now = Utc::now();
    crate::repos::federation_provider::NewFederationProvider {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: format!("https://{slug}.example"),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        allowed_origins: format!("https://{slug}.example"),
        id_token_algs: "RS256".into(),
        created_at: now,
        updated_at: now,
    }
}

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

// ── C16 — provider create ─────────────────────────────────────────────

#[tokio::test]
async fn a_c16_provider_create() {
    let db = fresh_db();
    let row = sample_new_provider("c16-happy");
    let before = audit_rows(&db).await;

    let audited =
        crate::commands::create_federation_provider(&db, row.clone(), Some("s3cr3t".into()))
            .await
            .expect("create");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(stored.slug, "c16-happy");
    assert!(
        stored.client_secret_enc.is_some(),
        "the secret must be sealed into the row"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.created");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.actor, None, "no session at startup");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "slug").as_deref(),
        Some("c16-happy")
    );
    assert_eq!(
        crate::registry::note_field(&note, "issuer").as_deref(),
        Some(row.issuer.as_str())
    );
    assert!(
        !note.contains("s3cr3t"),
        "the plaintext secret must never reach the note:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C16");
}

#[tokio::test]
async fn c16_injected_failure_before_append_rolls_back_the_create() {
    let db = fresh_db();
    let row = sample_new_provider("c16-rollback");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::create_federation_provider(&db, row.clone(), None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let fetched = repos::federation_provider::get_by_slug(&db, "c16-rollback").await;
    assert!(
        matches!(fetched, Err(StoreError::NotFound)),
        "the create must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C16");
}

// ── C23 — trust-policy replacement (retires C24) ──────────────────────

use crate::federation_policy::FederationTrustPolicy;

/// A fully enabled, `validated` provider with evidence bound to its own
/// (0, 0) counters — the same shape `create_enabled_for_test` always
/// produces — with a real sealed client secret, so the secret-change
/// tests below have something to compare against.
fn sample_enabled_row(slug: &str) -> crate::models::FederationProviderRow {
    let now = Utc::now();
    crate::models::FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: format!("https://{slug}.example"),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: crate::models::ValidationStatus::Validated,
        evidence_config_version: Some(0),
        evidence_activation_generation: Some(0),
        evidence_policy_digest: Some(vec![0xABu8; 32]),
        evidence_observed_at: Some(now),
        evidence_metadata_fingerprint: Some(vec![0xABu8; 32]),
        evidence_jwks_fingerprint: Some(vec![0xABu8; 32]),
        created_at: now,
        updated_at: now,
        id_token_algs: Some("RS256".into()),
    }
}

fn a_proposed_policy(issuer: &str) -> FederationTrustPolicy {
    FederationTrustPolicy::new(
        issuer,
        "client-xyz",
        true,
        "openid email profile",
        &["ES256"],
        &[],
        crate::models::ProvisionMode::ProvisionOnFirstLogin,
    )
    .expect("valid policy")
}

#[tokio::test]
async fn an_enabled_providers_policy_changing_disables_it_and_replaces_every_field() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-happy");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("old-secret"))
        .await
        .expect("seed enabled provider");
    let before = audit_rows(&db).await;

    let proposed = a_proposed_policy(&row.issuer);
    let audited = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        Some("new-secret".into()),
        Utc::now(),
    )
    .await
    .expect("C23 must not error")
    .expect("a policy this different must change something");
    let changed = audited.into_inner();
    for field in [
        "client_id",
        "scopes",
        "id_token_algs",
        "provision_mode",
        "client_secret",
    ] {
        assert!(
            changed.contains(&field),
            "expected {field:?} in {changed:?}"
        );
    }
    assert!(
        !changed.contains(&"issuer"),
        "issuer did not change: {changed:?}"
    );
    assert_eq!(
        changed,
        {
            let mut v = changed.clone();
            v.sort_unstable();
            v
        },
        "changed_fields must be sorted"
    );

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(!stored.enabled, "must be disabled");
    assert_eq!(
        stored.validation_status,
        crate::models::ValidationStatus::RequiresReview
    );
    assert!(
        stored.evidence_policy_digest.is_none(),
        "evidence must be cleared"
    );
    assert!(stored.evidence_observed_at.is_none());
    assert_eq!(stored.config_version, 1);
    assert_eq!(stored.activation_generation, 1);
    assert_eq!(stored.client_id, "client-xyz");
    assert_eq!(stored.scopes, "openid email profile");
    assert_eq!(stored.id_token_algs.as_deref(), Some("ES256"));
    assert_eq!(
        stored.provision_mode,
        crate::models::ProvisionMode::ProvisionOnFirstLogin
    );
    let opened = crate::crypto::open(
        db.key(),
        stored.client_secret_enc.as_ref().expect("secret present"),
        crate::repos::federation_provider::AAD,
    )
    .expect("must decrypt");
    assert_eq!(opened, b"new-secret");

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.policy_updated");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.actor, None, "no session at startup");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "old_config_version").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_config_version").as_deref(),
        Some("1")
    );
    assert_eq!(
        crate::registry::note_field(&note, "old_activation_generation").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_activation_generation").as_deref(),
        Some("1")
    );
    // Nothing was seeded to invalidate; the counts are present and zero
    // -- the shape every future caller's event will have, proven here
    // rather than assumed.
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_login_attempts").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_mfa_pending").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_ceremonies").as_deref(),
        Some("0")
    );
    assert!(
        !note.contains("new-secret") && !note.contains("old-secret"),
        "the plaintext secret must never reach the note:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C23");
}

/// §B's own two-provider isolation proof, exercised through the real
/// command rather than the shared invalidation function directly: a
/// pending login attempt for a second, untouched provider must survive
/// a policy replacement on the first.
#[tokio::test]
async fn replacing_one_providers_policy_invalidates_only_that_providers_flows() {
    let db = fresh_db();
    let target = sample_enabled_row("c23-target");
    repos::federation_provider::create_enabled_for_test(&db, &target, None)
        .await
        .expect("seed target");
    let other = sample_enabled_row("c23-other");
    repos::federation_provider::create_enabled_for_test(&db, &other, None)
        .await
        .expect("seed other");

    repos::federation_login_attempt::insert(
        &db,
        target.id,
        0,
        0,
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
        b"pkce-verifier",
        "https://rp.example/cb".into(),
        None,
        Utc::now(),
    )
    .await
    .expect("seed target's attempt");
    repos::federation_login_attempt::insert(
        &db,
        other.id,
        0,
        0,
        [4u8; 32],
        [5u8; 32],
        [6u8; 32],
        b"pkce-verifier",
        "https://rp.example/cb".into(),
        None,
        Utc::now(),
    )
    .await
    .expect("seed other's attempt");

    let proposed = a_proposed_policy(&target.issuer);
    crate::commands::replace_federation_provider_policy(
        &db,
        target.id,
        proposed,
        String::new(),
        None,
        Utc::now(),
    )
    .await
    .expect("C23 must not error")
    .expect("must change");

    let target_now = repos::federation_login_attempt::find_by_state_sha256(&db, [1u8; 32])
        .await
        .expect("target attempt still exists");
    let other_now = repos::federation_login_attempt::find_by_state_sha256(&db, [4u8; 32])
        .await
        .expect("other attempt still exists");
    assert_eq!(
        target_now.status,
        crate::models::FederationLoginAttemptStatus::Failed,
        "the target provider's own pending attempt must be invalidated"
    );
    assert_eq!(
        other_now.status,
        crate::models::FederationLoginAttemptStatus::Pending,
        "the other provider's attempt must be untouched"
    );
}

#[tokio::test]
async fn an_unchanged_policy_is_a_no_op_and_writes_no_event() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-unchanged");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("same-secret"))
        .await
        .expect("seed");
    let before = audit_rows(&db).await;

    let proposed = FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        true,
        &row.scopes,
        &["RS256"],
        &[],
        row.provision_mode.clone(),
    )
    .expect("valid policy");

    let result = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        Some("same-secret".into()),
        Utc::now(),
    )
    .await
    .expect("must not error");
    assert!(result.is_none(), "nothing differs; must be a no-op");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(
        stored.enabled,
        "an unchanged policy must not disable the provider"
    );
    assert_eq!(
        stored.config_version, 0,
        "a generation must never be burned for nothing"
    );
    assert_eq!(audit_rows(&db).await, before, "no event written");
}

#[tokio::test]
async fn an_unchanged_secret_is_not_resealed() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-secret-same");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("the-secret"))
        .await
        .expect("seed");
    let before_enc = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get")
        .client_secret_enc
        .expect("secret present");

    // The secret stays the same; scopes changes, so the call is not a
    // no-op overall.
    let proposed = FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        true,
        "openid",
        &["RS256"],
        &[],
        row.provision_mode.clone(),
    )
    .expect("valid policy");
    let audited = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        Some("the-secret".into()),
        Utc::now(),
    )
    .await
    .expect("must not error")
    .expect("scopes changed");
    assert!(
        !audited.into_inner().contains(&"client_secret"),
        "the unchanged secret must not be reported as a changed field"
    );

    let after_enc = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get")
        .client_secret_enc
        .expect("secret present");
    assert_eq!(
        before_enc, after_enc,
        "an unchanged secret must not be resealed -- the stored bytes must be byte-identical, \
         not merely decrypt to the same plaintext, since a fresh seal always produces a \
         different nonce/ciphertext"
    );
}

#[tokio::test]
async fn a_changed_secret_is_resealed() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-secret-changed");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("old-secret"))
        .await
        .expect("seed");
    let before_enc = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get")
        .client_secret_enc
        .expect("secret present");

    let proposed = FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        true,
        &row.scopes,
        &["RS256"],
        &[],
        row.provision_mode.clone(),
    )
    .expect("valid policy");
    let audited = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        Some("new-secret".into()),
        Utc::now(),
    )
    .await
    .expect("must not error")
    .expect("secret changed");
    assert!(audited.into_inner().contains(&"client_secret"));

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    let after_enc = stored.client_secret_enc.expect("secret present");
    assert_ne!(before_enc, after_enc, "a changed secret must be resealed");
    let opened = crate::crypto::open(db.key(), &after_enc, crate::repos::federation_provider::AAD)
        .expect("must decrypt");
    assert_eq!(opened, b"new-secret");
}

#[tokio::test]
async fn c23_against_a_nonexistent_provider_is_notfound() {
    let db = fresh_db();
    let ghost = FederationProviderId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::replace_federation_provider_policy(
        &db,
        ghost,
        a_proposed_policy("https://ghost.example"),
        String::new(),
        None,
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(StoreError::NotFound)));
    assert_eq!(audit_rows(&db).await, before);
}

#[tokio::test]
async fn c23_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-rollback");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("old-secret"))
        .await
        .expect("seed");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let proposed = a_proposed_policy(&row.issuer);
    let result = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        Some("new-secret".into()),
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(stored.enabled, "the row must not have been disabled");
    assert_eq!(stored.config_version, 0);
    assert_eq!(
        stored.client_secret_enc,
        repos::federation_provider::get(&db, row.id)
            .await
            .expect("get")
            .client_secret_enc,
        "no reseal must have survived rollback either"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C23");
}

#[tokio::test]
async fn a_config_version_overflow_refuses_and_writes_nothing() {
    let db = fresh_db();
    let mut row = sample_enabled_row("c23-overflow");
    row.config_version = i64::MAX;
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed");
    let before = audit_rows(&db).await;

    let proposed = a_proposed_policy(&row.issuer);
    let result = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        None,
        Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::Integrity(_))),
        "an overflowing checked_add must refuse, not wrap"
    );
    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(stored.config_version, i64::MAX, "nothing must have changed");
    assert_eq!(audit_rows(&db).await, before);
}

// ── C25 — disable on removal from configuration (amendment A2) ────────

#[tokio::test]
async fn an_enabled_provider_removed_from_config_is_disabled_and_keeps_its_links() {
    let db = fresh_db();
    let row = sample_enabled_row("c25-happy");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let user = a_user();
    repos::users::create(&db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        &db,
        crate::models::FederationLinkRow {
            user_id: user.id,
            provider_id: row.id,
            upstream_sub: "sub-1".into(),
            upstream_email: None,
            linked_at: Utc::now(),
            last_seen_at: Utc::now(),
        },
    )
    .await
    .expect("seed link");
    let before = audit_rows(&db).await;

    let audited =
        crate::commands::remove_federation_provider_from_configuration(&db, row.id, Utc::now())
            .await
            .expect("must not error")
            .expect("an enabled provider must be disabled");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(!stored.enabled);
    assert_eq!(
        stored.validation_status,
        crate::models::ValidationStatus::RequiresReview
    );
    assert!(stored.evidence_policy_digest.is_none());
    assert_eq!(stored.activation_generation, 1);
    assert_eq!(
        stored.config_version, 0,
        "config_version is untouched by a disable"
    );
    assert!(
        repos::federation_link::any_link_exists_for_provider(&db, row.id)
            .await
            .expect("link check"),
        "the link must be kept"
    );

    let event = last_event(&db).await;
    assert_eq!(
        event.action,
        "federation.provider.removed_from_configuration"
    );
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "old_activation_generation").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_activation_generation").as_deref(),
        Some("1")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C25");
}

#[tokio::test]
async fn an_already_disabled_provider_removed_from_config_is_a_no_op() {
    let db = fresh_db();
    let row = sample_row("c25-already-disabled");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed disabled provider");
    let before = audit_rows(&db).await;

    let result =
        crate::commands::remove_federation_provider_from_configuration(&db, row.id, Utc::now())
            .await
            .expect("must not error");
    assert!(result.is_none(), "an already-disabled provider is a no-op");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(
        stored.activation_generation, 0,
        "a generation must never be burned for nothing"
    );
    assert_eq!(audit_rows(&db).await, before, "no event written");
    record_rollback_coverage("C25"); // the non-committing guarded path *is* this command's rollback case
}

#[tokio::test]
async fn c25_against_a_nonexistent_provider_is_notfound() {
    let db = fresh_db();
    let ghost = FederationProviderId::new();
    let before = audit_rows(&db).await;

    let result =
        crate::commands::remove_federation_provider_from_configuration(&db, ghost, Utc::now())
            .await;
    assert!(matches!(result, Err(StoreError::NotFound)));
    assert_eq!(audit_rows(&db).await, before);
}

// ── C26 — display-name update (no disable) ─────────────────────────────

#[tokio::test]
async fn a_changed_display_name_is_applied_without_disabling_the_provider() {
    let db = fresh_db();
    let row = sample_enabled_row("c26-happy");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_federation_provider_display_name(
        &db,
        row.id,
        "A New Label".into(),
        Utc::now(),
    )
    .await
    .expect("update");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(stored.display_name, "A New Label");
    assert!(
        stored.enabled,
        "display_name is not trust policy; must not disable"
    );
    assert_eq!(stored.config_version, 0);
    assert_eq!(stored.activation_generation, 0);

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.display_name_changed");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.note, None, "no values in this event");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C26");
}

#[tokio::test]
async fn c26_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let row = sample_enabled_row("c26-rollback");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::update_federation_provider_display_name(
        &db,
        row.id,
        "A New Label".into(),
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(
        stored.display_name, row.display_name,
        "the change must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C26");
}

// ── S2b review fold-ins ───────────────────────────────────────────────

/// A row migration 0051 copied: everything recorded except the
/// algorithms, with origins consistent with its issuer.
fn a_row_0051_copied(slug: &str) -> crate::models::FederationProviderRow {
    let mut row = sample_row(slug);
    row.allowed_origins = String::new();
    row.id_token_algs = None;
    row
}

/// The event is hash-chained: it must name the field that changed, not
/// all seven, when only the algorithms were unrecorded.
#[tokio::test]
async fn c23_on_a_row_0051_copied_records_only_the_unrecorded_field() {
    let db = fresh_db();
    let row = a_row_0051_copied("c23-legacy");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed");
    let proposed = FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        false,
        &row.scopes,
        &["RS256"],
        &[],
        row.provision_mode.clone(),
    )
    .expect("valid");

    crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        None,
        Utc::now(),
    )
    .await
    .expect("C23")
    .expect("records the algorithms");

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.policy_updated");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "changed_fields").as_deref(),
        Some("id_token_algs")
    );
    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(stored.id_token_algs.as_deref(), Some("RS256"));
}

/// The secret's absent case, and the method derived from it.
#[tokio::test]
async fn a_removed_secret_changes_the_secret_and_the_token_auth_method() {
    let db = fresh_db();
    let row = sample_enabled_row("c23-secret-removed");
    repos::federation_provider::create_enabled_for_test(&db, &row, Some("old-secret"))
        .await
        .expect("seed");
    let proposed = FederationTrustPolicy::new(
        &row.issuer,
        &row.client_id,
        false,
        &row.scopes,
        &["RS256"],
        &[],
        row.provision_mode.clone(),
    )
    .expect("valid");

    let changed = crate::commands::replace_federation_provider_policy(
        &db,
        row.id,
        proposed,
        String::new(),
        None,
        Utc::now(),
    )
    .await
    .expect("C23")
    .expect("changed")
    .into_inner();

    assert_eq!(changed, ["client_secret", "token_auth_method"]);
    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(stored.client_secret_enc.is_none(), "the secret is cleared");
    assert!(!stored.enabled);
}

/// S2b §B's two-provider test, for all three tables: only the named
/// provider's attempt, continuation and ceremony are touched, with exact
/// counts.
#[tokio::test]
async fn the_flow_invalidation_touches_only_the_named_providers_flows() {
    use crate::tests_rfc096_b2_stage2a::{
        seed_attempt, seed_ceremony, seed_link, seed_mfa_pending, seed_user,
    };
    let db = fresh_db();
    let a = sample_row("flows-a");
    let b = sample_row("flows-b");
    repos::federation_provider::create(&db, &a, None)
        .await
        .expect("a");
    repos::federation_provider::create(&db, &b, None)
        .await
        .expect("b");
    let (aid, bid) = (a.id.to_string(), b.id.to_string());
    db.with_conn(move |c| {
        seed_user(c, "user-a");
        seed_user(c, "user-b");
        seed_link(c, "user-a", &aid);
        seed_link(c, "user-b", &bid);
        seed_attempt(c, "attempt-a", &aid, 1);
        seed_attempt(c, "attempt-b", &bid, 2);
        seed_mfa_pending(c, "pending-a", &aid, "user-a", 3);
        seed_mfa_pending(c, "pending-b", &bid, "user-b", 4);
        seed_ceremony(c, "ceremony-a", "pending-a", &aid, "user-a");
        seed_ceremony(c, "ceremony-b", "pending-b", &bid, "user-b");
        Ok(())
    })
    .await
    .expect("seed flows");

    let target = a.id;
    let counts = db
        .with_tx(move |tx| {
            crate::commands::invalidate_provider_flows_within_tx(tx, target, Utc::now())
        })
        .await
        .expect("invalidate");
    assert_eq!(
        (counts.login_attempts, counts.mfa_pending, counts.ceremonies),
        (1, 1, 1)
    );

    let state = db
        .with_conn(|c| {
            let attempt = |id: &str| -> rusqlite::Result<String> {
                c.query_row(
                    "SELECT status FROM federation_login_attempt WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
            };
            let pending = |id: &str| -> rusqlite::Result<(String, bool)> {
                c.query_row(
                    "SELECT status, continuation_sealed IS NULL FROM federation_mfa_pending \
                     WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            };
            let ceremony = |id: &str| -> rusqlite::Result<i64> {
                c.query_row(
                    "SELECT COUNT(*) FROM federation_webauthn_ceremony WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
            };
            Ok((
                attempt("attempt-a")?,
                attempt("attempt-b")?,
                pending("pending-a")?,
                pending("pending-b")?,
                ceremony("ceremony-a")?,
                ceremony("ceremony-b")?,
            ))
        })
        .await
        .expect("read back");
    assert_eq!(state.0, "failed");
    assert_eq!(
        state.1, "pending",
        "the other provider's attempt is untouched"
    );
    assert_eq!(state.2, ("invalidated".to_string(), true));
    assert_eq!(
        state.3,
        ("pending".to_string(), false),
        "the other continuation is untouched"
    );
    assert_eq!(state.4, 0, "the named provider's ceremony is gone");
    assert_eq!(state.5, 1, "the other provider's ceremony is kept");
}

// ── C17 — enable (from a preflight) / disable ──────────────────────────

use crate::commands::{
    ActivationGeneration, ConfigVersion, DisableFederationProviderError, EnableFromPreflightError,
    JwksFingerprint, MetadataFingerprint, PreflightEvidence,
};

/// A disabled provider with a *recorded, usable* trust policy -- the
/// state a provider is in after a real preflight (S3) but before C17
/// enable, unlike `sample_row`'s own bare disabled-with-no-algs shape.
fn a_disabled_row_with_recorded_policy(slug: &str) -> crate::models::FederationProviderRow {
    let now = Utc::now();
    crate::models::FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: format!("https://{slug}.example"),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: crate::models::ValidationStatus::RequiresReview,
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

/// The evidence a real preflight (S3) would have produced for `row`, as
/// it stands right now -- decodes `row`'s own policy and computes its
/// real digest, rather than a fixed placeholder, so a test that mutates
/// the row afterward (changing its generation, or its policy) produces
/// evidence that genuinely disagrees with the row C17 re-reads.
fn evidence_for(
    row: &crate::models::FederationProviderRow,
    observed_at: chrono::DateTime<chrono::Utc>,
) -> PreflightEvidence {
    let policy = crate::federation_policy::from_stored(row).expect("row must decode");
    let digest =
        crate::federation_policy::compute_policy_digest(&policy, row.client_secret_enc.as_deref());
    PreflightEvidence {
        provider_id: row.id,
        config_version: ConfigVersion(row.config_version),
        activation_generation: ActivationGeneration(row.activation_generation),
        policy_digest: digest,
        observed_at,
        metadata_fingerprint: MetadataFingerprint([0xCDu8; 32]),
        jwks_fingerprint: JwksFingerprint([0xEFu8; 32]),
    }
}

#[tokio::test]
async fn an_enable_from_preflight_succeeds_and_records_the_evidence() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-enable-happy");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let now = Utc::now();
    let evidence = evidence_for(&row, now);
    let audited = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await
    .expect("enable must succeed");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(stored.enabled);
    assert_eq!(
        stored.validation_status,
        crate::models::ValidationStatus::Validated
    );
    assert_eq!(
        stored.config_version, 0,
        "enable never changes config_version"
    );
    assert_eq!(stored.activation_generation, 1);
    assert_eq!(stored.evidence_config_version, Some(0));
    assert_eq!(stored.evidence_activation_generation, Some(1));
    assert!(stored.evidence_policy_digest.is_some());
    assert_eq!(
        stored.evidence_metadata_fingerprint,
        Some([0xCDu8; 32].to_vec())
    );
    assert_eq!(
        stored.evidence_jwks_fingerprint,
        Some([0xEFu8; 32].to_vec())
    );
    assert_eq!(stored.evidence_observed_at, Some(now));

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.enabled");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.actor, Some(admin));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "config_version").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "old_activation_generation").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_activation_generation").as_deref(),
        Some("1")
    );
    assert_eq!(
        crate::registry::note_field(&note, "metadata_fingerprint").as_deref(),
        Some("cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd")
    );
    assert_eq!(
        crate::registry::note_field(&note, "jwks_fingerprint").as_deref(),
        Some("efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef")
    );
    assert!(
        !note.contains("evidence_policy_digest")
            && !note.contains("observed_at")
            && crate::registry::note_field(&note, "observed_at").is_none(),
        "RFC 096 :436-437: never the digest, never observed_at:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C17");
}

#[tokio::test]
async fn enable_accepts_a_599_second_old_check() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-599");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;

    let observed_at = Utc::now() - chrono::Duration::seconds(599);
    db.guard_clock().set(Utc::now());
    let evidence = evidence_for(&row, observed_at);
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db,
        admin,
        session_id,
        evidence,
        Utc::now(),
    )
    .await;
    assert!(result.is_ok(), "599 seconds old must be accepted");
}

#[tokio::test]
async fn enable_refuses_when_the_check_is_exactly_600_seconds_old() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-600");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;

    let guard_at = Utc::now();
    let observed_at = guard_at - chrono::Duration::seconds(600);
    db.guard_clock().set(guard_at);
    let evidence = evidence_for(&row, observed_at);
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db,
        admin,
        session_id,
        evidence,
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(EnableFromPreflightError::CheckTooOld)));
}

#[tokio::test]
async fn enable_refuses_a_check_with_a_future_observed_at() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-negative-age");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;

    let guard_at = Utc::now();
    let observed_at = guard_at + chrono::Duration::seconds(5); // in the future
    db.guard_clock().set(guard_at);
    let evidence = evidence_for(&row, observed_at);
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db,
        admin,
        session_id,
        evidence,
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(EnableFromPreflightError::CheckTooOld)));
}

#[tokio::test]
async fn enable_refuses_an_already_enabled_provider() {
    let db = fresh_db();
    let mut row = a_disabled_row_with_recorded_policy("c17-already-enabled");
    row.enabled = true;
    row.validation_status = crate::models::ValidationStatus::Validated;
    let (admin, session_id) = an_admin_session(&db).await;
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");

    let now = Utc::now();
    let evidence = evidence_for(&row, now);
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(matches!(
        result,
        Err(EnableFromPreflightError::AlreadyEnabled)
    ));
}

#[tokio::test]
async fn enable_refuses_when_the_activation_generation_changed_since_the_check() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-wrong-generation");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let now = Utc::now();
    let evidence = evidence_for(&row, now); // captures generation 0

    // Something else bumps the generation out from under this evidence.
    db.with_conn(move |c| {
        Ok(c.execute(
            "UPDATE federation_provider SET activation_generation = 5 WHERE id = ?1",
            [row.id.to_string()],
        )?)
    })
    .await
    .expect("bump generation");

    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(matches!(
        result,
        Err(EnableFromPreflightError::ChangedSinceCheck)
    ));
}

#[tokio::test]
async fn enable_refuses_when_the_policy_digest_no_longer_matches() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-wrong-policy");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let now = Utc::now();

    // Evidence computed for a *different* policy than what is stored --
    // standing in for "the row's trust policy changed after the check."
    let wrong_policy = FederationTrustPolicy::new(
        "https://a-different-issuer.example",
        "a-different-client",
        false,
        "openid",
        &["RS256"],
        &[],
        crate::models::ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let wrong_digest = crate::federation_policy::compute_policy_digest(&wrong_policy, None);
    let mut evidence = evidence_for(&row, now);
    evidence.policy_digest = wrong_digest;

    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(matches!(
        result,
        Err(EnableFromPreflightError::ChangedSinceCheck)
    ));
}

#[tokio::test]
async fn enable_refuses_a_nonexistent_provider() {
    let db = fresh_db();
    let (admin, session_id) = an_admin_session(&db).await;
    let ghost = a_disabled_row_with_recorded_policy("c17-ghost"); // never created
    let now = Utc::now();
    let evidence = evidence_for(&ghost, now);
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(matches!(result, Err(EnableFromPreflightError::NotFound)));
}

/// RFC 096-B2 S4 dispatch §A row 10: "two preflights, then two enables:
/// the second refuses as changed since the check." Also row 11's
/// "replayed" case, since the second call here submits the *same*
/// evidence the first one already consumed.
#[tokio::test]
async fn two_preflights_race_the_second_enable_refuses_as_changed_since_the_check() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-race");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let now = Utc::now();

    // Two preflights, both run against the same (generation 0) row.
    let evidence_a = evidence_for(&row, now);
    let evidence_b = evidence_for(&row, now);

    let first = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence_a, now,
    )
    .await;
    assert!(first.is_ok(), "the first enable must win");

    let second = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence_b, now,
    )
    .await;
    assert!(
        matches!(second, Err(EnableFromPreflightError::AlreadyEnabled)),
        "the second capability is now stale -- the provider is already enabled, \
         which is also a true statement of what changed since that check"
    );
}

#[tokio::test]
async fn enable_refuses_at_generation_overflow_and_writes_nothing() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-overflow");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    // `create()`'s own `INSERT` leaves `activation_generation` to the
    // schema `DEFAULT` (0); set it to the boundary value directly, then
    // re-read the row so the evidence below is built from what is
    // actually stored, not from `row`'s own (unpersisted) field.
    db.with_conn({
        let id = row.id;
        move |c| {
            Ok(c.execute(
                "UPDATE federation_provider SET activation_generation = ?1 WHERE id = ?2",
                rusqlite::params![i64::MAX, id.to_string()],
            )?)
        }
    })
    .await
    .expect("set activation_generation to i64::MAX");
    let row = repos::federation_provider::get(&db, row.id)
        .await
        .expect("re-read");
    let (admin, session_id) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;
    let now = Utc::now();
    let evidence = evidence_for(&row, now);

    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(matches!(result, Err(EnableFromPreflightError::Storage)));
    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(!stored.enabled, "nothing must have changed");
    assert_eq!(stored.activation_generation, i64::MAX);
    assert_eq!(audit_rows(&db).await, before);
}

#[tokio::test]
async fn c17_enable_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-enable-rollback");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;
    let now = Utc::now();
    let evidence = evidence_for(&row, now);

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::enable_federation_provider_from_preflight(
        &db, admin, session_id, evidence, now,
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(
        !stored.enabled,
        "the enable must not have survived rollback"
    );
    assert_eq!(stored.activation_generation, 0);
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C17");
}

#[tokio::test]
async fn c17_disable_disables_an_enabled_provider_and_invalidates_its_flows() {
    let db = fresh_db();
    let row = sample_enabled_row("c17-disable-happy");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let (admin, session_id) = an_admin_session(&db).await;
    repos::federation_login_attempt::insert(
        &db,
        row.id,
        0,
        0,
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
        b"pkce-verifier",
        "https://rp.example/cb".into(),
        None,
        Utc::now(),
    )
    .await
    .expect("seed a pending attempt");
    let before = audit_rows(&db).await;
    let now = Utc::now();

    let audited = crate::commands::disable_federation_provider(&db, admin, session_id, row.id, now)
        .await
        .expect("must not error")
        .expect("an enabled provider must be disabled");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(!stored.enabled);
    assert_eq!(
        stored.validation_status,
        crate::models::ValidationStatus::Validated,
        "validation_status is left exactly as it was"
    );
    assert!(stored.evidence_policy_digest.is_none());
    assert_eq!(stored.activation_generation, 1);
    assert_eq!(stored.config_version, 0);

    let target_attempt = repos::federation_login_attempt::find_by_state_sha256(&db, [1u8; 32])
        .await
        .expect("still exists");
    assert_eq!(
        target_attempt.status,
        crate::models::FederationLoginAttemptStatus::Failed
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.disabled");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "old_activation_generation").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_activation_generation").as_deref(),
        Some("1")
    );
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_login_attempts").as_deref(),
        Some("1")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C17");
}

#[tokio::test]
async fn c17_disable_is_a_no_op_for_an_already_disabled_provider() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c17-disable-noop");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed disabled provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let before = audit_rows(&db).await;

    let result =
        crate::commands::disable_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await
            .expect("must not error");
    assert!(result.is_none(), "an already-disabled provider is a no-op");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(
        stored.activation_generation, 0,
        "a generation must never be burned for nothing"
    );
    assert_eq!(audit_rows(&db).await, before, "no event written");
    record_rollback_coverage("C17"); // the non-committing guarded path is this command's rollback case for disable
}

#[tokio::test]
async fn c17_disable_refuses_a_nonexistent_provider() {
    let db = fresh_db();
    let (admin, session_id) = an_admin_session(&db).await;
    let ghost = FederationProviderId::new();
    let result =
        crate::commands::disable_federation_provider(&db, admin, session_id, ghost, Utc::now())
            .await;
    assert!(matches!(
        result,
        Err(DisableFederationProviderError::NotFound)
    ));
}

#[tokio::test]
async fn c17_disable_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let row = sample_enabled_row("c17-disable-rollback");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::disable_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(
        stored.enabled,
        "the disable must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C17");
}

/// S4 review: C17's store-side refusals give a reason and a next step,
/// the rule every operator-facing refusal in this cycle follows.
#[test]
fn c17_store_refusals_each_name_a_next_step() {
    use crate::commands::{DisableFederationProviderError as D, EnableFromPreflightError as E};
    let enable = [
        (E::ChangedSinceCheck.to_string(), "run it again"),
        (E::CheckTooOld.to_string(), "run it again"),
        (E::AlreadyEnabled.to_string(), "disable it first"),
        (E::NotFound.to_string(), "reload the provider list"),
        (E::StepUpRequired.to_string(), "confirm it again"),
        (E::Storage.to_string(), "see the server log"),
    ];
    let disable = [
        (D::NotFound.to_string(), "reload the provider list"),
        (D::StepUpRequired.to_string(), "confirm it again"),
        (D::Storage.to_string(), "see the server log"),
    ];
    for (text, next_step) in enable.iter().chain(disable.iter()) {
        assert!(text.contains(next_step), "{text:?} lacks {next_step:?}");
    }
}

/// S4 review: the chooser, not only the choice. The backend test proves
/// `with_tx_immediate` waits; this proves **C17** runs through it.
/// Another connection holds the write lock for 2 s while C17 runs on a
/// check that is 599.5 s old.
/// - `IMMEDIATE` (correct): C17's `BEGIN` waits for the lock, so `guard_at`
///   is read after the wait, the check is over 600 s old, and C17 refuses
///   it as stale.
/// - `DEFERRED`: `guard_at` would be read before any wait (C17's first
///   statements are reads, which WAL never blocks), the check would pass at
///   599 s, and the `UPDATE` would then wait out the lock and enable the
///   provider.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c17_reads_guard_at_only_after_it_holds_the_write_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("c17-immediate.db");
    let db = Database::open(&path, crate::crypto::MasterKey::generate()).expect("open file db");
    let row = a_disabled_row_with_recorded_policy("c17-lock");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let evidence = evidence_for(&row, Utc::now() - chrono::Duration::milliseconds(599_500));

    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let lock_path = path.clone();
    let holder = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(&lock_path).expect("second connection");
        conn.execute_batch("BEGIN IMMEDIATE;")
            .expect("take the write lock");
        ready_tx.send(()).expect("signal");
        std::thread::sleep(std::time::Duration::from_secs(2));
        conn.execute_batch("ROLLBACK;").expect("release it");
    });
    ready_rx.recv().expect("the lock is held");

    let result = crate::commands::enable_federation_provider_from_preflight(
        &db,
        admin,
        session_id,
        evidence,
        Utc::now(),
    )
    .await;
    holder.join().expect("lock holder");

    assert!(
        matches!(result, Err(EnableFromPreflightError::CheckTooOld)),
        "guard_at must be read after the write lock is held"
    );
    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert!(!stored.enabled, "nothing was enabled");
}

// ── C18 — federation provider delete ────────────────────────────────────

use crate::commands::{DeleteFederationProviderError, UnlinkFederationProviderError};

async fn seed_link(db: &Database, user_id: UserId, provider_id: FederationProviderId) {
    let now = Utc::now();
    repos::federation_link::upsert(
        db,
        crate::models::FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: format!("sub-{user_id}"),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .expect("seed link");
}

/// Migration 0048's own partial `UNIQUE (user_id) WHERE status IN
/// ('pending', 'exhausted')` index means a user can have at most one row
/// in either of those two statuses at a time -- a test wanting more than
/// one row for the same user must give every row after the first a
/// different, non-competing status.
async fn seed_mfa_pending_for_with_status(
    db: &Database,
    user_id: UserId,
    provider: &crate::models::FederationProviderRow,
    status: crate::models::FederationMfaPendingStatus,
) {
    let now = Utc::now();
    let provider_id = provider.id;
    let provider_config_version = provider.config_version;
    let provider_activation_generation = provider.activation_generation;
    let continuation_sealed =
        (status == crate::models::FederationMfaPendingStatus::Pending).then(|| vec![0u8; 16]);
    db.with_tx(move |tx| {
        crate::repos::federation_mfa_pending::insert_within_tx(
            tx,
            &crate::models::FederationMfaPendingRow {
                id: sui_id_shared::ids::FederationMfaPendingId::new(),
                provider_id,
                provider_config_version,
                provider_activation_generation,
                user_id,
                continuation_sealed,
                next_path: None,
                browser_binding_sha256: [0u8; 32],
                csrf_binding_sha256: [0u8; 32],
                failure_count: 0,
                status,
                created_at: now,
                expires_at: now + chrono::Duration::seconds(300),
            },
        )
    })
    .await
    .expect("seed mfa pending");
}

async fn seed_mfa_pending_for(
    db: &Database,
    user_id: UserId,
    provider: &crate::models::FederationProviderRow,
) {
    seed_mfa_pending_for_with_status(
        db,
        user_id,
        provider,
        crate::models::FederationMfaPendingStatus::Pending,
    )
    .await;
}

async fn an_ordinary_user(db: &Database) -> UserId {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    user.id
}

async fn link_exists(db: &Database, user_id: UserId, provider_id: FederationProviderId) -> bool {
    db.with_conn(move |conn| repos::federation_link::exists_within_tx(conn, user_id, provider_id))
        .await
        .expect("exists check")
}

#[tokio::test]
async fn delete_refuses_an_enabled_provider() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c18-enabled");
    repos::federation_provider::create_enabled_for_test(&db, &row, None)
        .await
        .expect("seed enabled provider");
    let (admin, session_id) = an_admin_session(&db).await;

    let result =
        crate::commands::delete_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await;
    assert!(matches!(
        result,
        Err(DeleteFederationProviderError::StillEnabled)
    ));
    assert!(
        repos::federation_provider::get(&db, row.id).await.is_ok(),
        "nothing was deleted"
    );
}

#[tokio::test]
async fn delete_refuses_a_nonexistent_provider() {
    let db = fresh_db();
    let (admin, session_id) = an_admin_session(&db).await;
    let ghost = FederationProviderId::new();

    let result =
        crate::commands::delete_federation_provider(&db, admin, session_id, ghost, Utc::now())
            .await;
    assert!(matches!(
        result,
        Err(DeleteFederationProviderError::NotFound)
    ));
}

#[tokio::test]
async fn delete_succeeds_and_cascades_links_and_continuations_but_keeps_the_users() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c18-happy");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed disabled provider");
    let (admin, session_id) = an_admin_session(&db).await;

    let user_a = an_ordinary_user(&db).await;
    let user_b = an_ordinary_user(&db).await;
    let user_c = an_ordinary_user(&db).await;
    for u in [user_a, user_b, user_c] {
        seed_link(&db, u, row.id).await;
    }
    seed_mfa_pending_for(&db, user_a, &row).await;
    seed_mfa_pending_for(&db, user_b, &row).await;

    let before = audit_rows(&db).await;
    let audited =
        crate::commands::delete_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await
            .expect("must succeed");
    audited.into_inner();

    assert!(
        matches!(
            repos::federation_provider::get(&db, row.id).await,
            Err(StoreError::NotFound)
        ),
        "the provider row is gone"
    );
    for u in [user_a, user_b, user_c] {
        assert!(
            !link_exists(&db, u, row.id).await,
            "the link for {u} must be gone"
        );
        assert!(
            repos::users::get(&db, u).await.is_ok(),
            "the user {u} must still exist -- only the link is deleted"
        );
    }
    let remaining_pending: i64 = db
        .with_conn(|c| {
            Ok(
                c.query_row("SELECT COUNT(*) FROM federation_mfa_pending", [], |r| {
                    r.get(0)
                })?,
            )
        })
        .await
        .expect("count");
    assert_eq!(remaining_pending, 0, "both pending rows cascaded away");

    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.deleted");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.actor, Some(admin));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "old_activation_generation").as_deref(),
        Some("0")
    );
    assert_eq!(
        crate::registry::note_field(&note, "new_activation_generation").as_deref(),
        Some("1")
    );
    assert_eq!(
        crate::registry::note_field(&note, "deleted_links").as_deref(),
        Some("3"),
        "the exact count A3 asks the event to record"
    );
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_mfa_pending").as_deref(),
        Some("2")
    );
    record_exactly_once_coverage("C18");
}

#[tokio::test]
async fn deleting_the_same_provider_twice_is_notfound_the_second_time() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c18-twice");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed disabled provider");
    let (admin, session_id) = an_admin_session(&db).await;

    crate::commands::delete_federation_provider(&db, admin, session_id, row.id, Utc::now())
        .await
        .expect("first delete succeeds");
    let second =
        crate::commands::delete_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await;
    assert!(
        matches!(second, Err(DeleteFederationProviderError::NotFound)),
        "a capability cannot be replayed against a row that no longer exists"
    );
}

#[tokio::test]
async fn c18_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c18-rollback");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed disabled provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let user = an_ordinary_user(&db).await;
    seed_link(&db, user, row.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::delete_federation_provider(&db, admin, session_id, row.id, Utc::now())
            .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::federation_provider::get(&db, row.id).await.is_ok(),
        "the provider must not have survived rollback as deleted"
    );
    assert!(
        link_exists(&db, user, row.id).await,
        "the link must not have survived rollback as deleted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C18");
}

// ── C20 — federation link delete (unlink) ───────────────────────────────

#[tokio::test]
async fn unlink_refuses_a_nonexistent_link() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c20-no-link");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let user = an_ordinary_user(&db).await;

    let result = crate::commands::unlink_federation_provider(
        &db,
        admin,
        session_id,
        user,
        row.id,
        Utc::now(),
    )
    .await;
    assert!(matches!(
        result,
        Err(UnlinkFederationProviderError::NotFound)
    ));
}

#[tokio::test]
async fn unlink_succeeds_and_cascades_continuations_but_keeps_the_user_and_other_links() {
    let db = fresh_db();
    let row_a = a_disabled_row_with_recorded_policy("c20-happy-a");
    let row_b = a_disabled_row_with_recorded_policy("c20-happy-b");
    repos::federation_provider::create(&db, &row_a, None)
        .await
        .expect("seed provider a");
    repos::federation_provider::create(&db, &row_b, None)
        .await
        .expect("seed provider b");
    let (admin, session_id) = an_admin_session(&db).await;
    let user = an_ordinary_user(&db).await;
    seed_link(&db, user, row_a.id).await;
    seed_link(&db, user, row_b.id).await;
    seed_mfa_pending_for(&db, user, &row_a).await;
    seed_mfa_pending_for_with_status(
        &db,
        user,
        &row_a,
        crate::models::FederationMfaPendingStatus::Completed,
    )
    .await;

    let before = audit_rows(&db).await;
    let audited = crate::commands::unlink_federation_provider(
        &db,
        admin,
        session_id,
        user,
        row_a.id,
        Utc::now(),
    )
    .await
    .expect("must succeed");
    audited.into_inner();

    assert!(
        !link_exists(&db, user, row_a.id).await,
        "the link to provider a is gone"
    );
    assert!(
        link_exists(&db, user, row_b.id).await,
        "the link to provider b must remain -- only one pair was unlinked"
    );
    assert!(
        repos::users::get(&db, user).await.is_ok(),
        "the user must still exist"
    );
    let remaining_pending: i64 = db
        .with_conn(|c| {
            Ok(
                c.query_row("SELECT COUNT(*) FROM federation_mfa_pending", [], |r| {
                    r.get(0)
                })?,
            )
        })
        .await
        .expect("count");
    assert_eq!(
        remaining_pending, 0,
        "both continuations for the pair cascaded away"
    );

    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.link.deleted");
    assert_eq!(event.target.as_deref(), Some(user.to_string().as_str()));
    assert_eq!(event.actor, Some(admin));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "provider_id").as_deref(),
        Some(row_a.id.to_string().as_str())
    );
    assert_eq!(
        crate::registry::note_field(&note, "invalidated_continuations").as_deref(),
        Some("2")
    );
    record_exactly_once_coverage("C20");
}

#[tokio::test]
async fn unlinking_the_same_pair_twice_is_notfound_the_second_time() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c20-twice");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let user = an_ordinary_user(&db).await;
    seed_link(&db, user, row.id).await;

    crate::commands::unlink_federation_provider(&db, admin, session_id, user, row.id, Utc::now())
        .await
        .expect("first unlink succeeds");
    let second = crate::commands::unlink_federation_provider(
        &db,
        admin,
        session_id,
        user,
        row.id,
        Utc::now(),
    )
    .await;
    assert!(matches!(
        second,
        Err(UnlinkFederationProviderError::NotFound)
    ));
}

#[tokio::test]
async fn c20_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let row = a_disabled_row_with_recorded_policy("c20-rollback");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let (admin, session_id) = an_admin_session(&db).await;
    let user = an_ordinary_user(&db).await;
    seed_link(&db, user, row.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::unlink_federation_provider(
        &db,
        admin,
        session_id,
        user,
        row.id,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        link_exists(&db, user, row.id).await,
        "the link must not have survived rollback as deleted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C20");
}

/// S6 review-anticipating check: C18's and C20's store-side refusals
/// each give a reason and a next step.
#[test]
fn c18_and_c20_store_refusals_each_name_a_next_step() {
    let c18 = [
        (
            DeleteFederationProviderError::NotFound.to_string(),
            "reload the provider list",
        ),
        (
            DeleteFederationProviderError::StillEnabled.to_string(),
            "disable it first",
        ),
        (
            DeleteFederationProviderError::StepUpRequired.to_string(),
            "confirm it again",
        ),
        (
            DeleteFederationProviderError::Storage.to_string(),
            "try again",
        ),
    ];
    let c20 = [
        (
            UnlinkFederationProviderError::NotFound.to_string(),
            "reload the page",
        ),
        (
            UnlinkFederationProviderError::StepUpRequired.to_string(),
            "confirm it again",
        ),
        (
            UnlinkFederationProviderError::Storage.to_string(),
            "try again",
        ),
    ];
    for (text, next_step) in c18.iter().chain(c20.iter()) {
        assert!(text.contains(next_step), "{text:?} lacks {next_step:?}");
    }
}
