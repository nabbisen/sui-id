use super::*;
use crate::{Database, crypto::MasterKey};
use chrono::Utc;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn sample(slug: &str) -> FederationProviderRow {
    let now = Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Test IdP".into(),
        issuer: "https://idp.example.com".into(),
        client_id: "client-abc".into(),
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
        id_token_algs: None,
    }
}

#[tokio::test]
async fn create_and_get_by_slug() {
    let db = fresh_db();
    let row = sample("google");
    create(&db, &row, Some("supersecret")).await.unwrap();
    let fetched = get_by_slug(&db, "google").await.unwrap();
    assert_eq!(fetched.slug, "google");
    assert!(fetched.client_secret_enc.is_some());
    let plain = decrypt_secret(db.key(), &fetched).unwrap();
    assert_eq!(plain.as_deref(), Some("supersecret"));
}

#[tokio::test]
async fn list_enabled_filters_disabled() {
    let db = fresh_db();
    create(&db, &sample("g1"), None).await.unwrap();
    let row2 = sample("g2");
    create_enabled_for_test(&db, &row2, None).await.unwrap();
    let enabled = list_enabled(&db).await.unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].slug, "g2");
}

/// RFC 096-B2 S2b, C23 (RFC 094 clause 9): `replace_policy_within_tx`'s
/// own guarded `UPDATE` must refuse when the caller's stated old
/// `activation_generation` does not match the row's real one, even when
/// `old_config_version` alone would have matched. A single-conjunct
/// mismatch, not just "both wrong" -- the mutation this proves against
/// is removing `AND activation_generation = ?13` from the `UPDATE`'s own
/// `WHERE`, which would let this exact call through.
#[tokio::test]
async fn replace_policy_refuses_when_the_activation_generation_guard_does_not_match() {
    let db = fresh_db();
    let row = sample("c23-guard");
    create(&db, &row, None).await.unwrap(); // config_version=0, activation_generation=0

    let affected = db
        .with_tx(move |tx| {
            replace_policy_within_tx(
                tx,
                row.id,
                "https://new.example",
                "new-client",
                None,
                "openid",
                ProvisionMode::LinkOnly,
                "",
                "RS256",
                /* old_config_version */ 0,
                /* new_config_version */ 1,
                /* old_activation_generation */ 999, // wrong on purpose
                /* new_activation_generation */ 1000,
                Utc::now(),
            )
        })
        .await
        .unwrap();
    assert_eq!(
        affected, 0,
        "a mismatched activation_generation guard must refuse the write"
    );

    let fetched = get(&db, row.id).await.unwrap();
    assert_eq!(fetched.issuer, row.issuer, "nothing must have changed");
    assert_eq!(fetched.config_version, 0);
    assert_eq!(fetched.activation_generation, 0);
}

/// RFC 096-B2 S2a: `set_enabled` lost its ability to *enable* a row
/// (migration 0051's `CHECK` -- see `create_enabled_for_test`'s own doc
/// comment); this now proves the enabled-row and delete halves
/// separately instead of flipping one row's flag mid-test.
#[tokio::test]
async fn create_enabled_then_delete() {
    let db = fresh_db();
    let row = sample("test");
    let id = row.id;
    create_enabled_for_test(&db, &row, None).await.unwrap();
    let fetched = get(&db, id).await.unwrap();
    assert!(fetched.enabled);
    delete(&db, id).await.unwrap();
    assert!(matches!(get(&db, id).await, Err(StoreError::NotFound)));
}

/// `set_disabled` can still disable a provider that was created enabled
/// (through [`create_enabled_for_test`], the only way left to create
/// one) -- the half of the old `set_enabled_and_delete` test that
/// `set_disabled`'s own narrower signature still covers.
#[tokio::test]
async fn set_disabled_disables_an_enabled_provider() {
    let db = fresh_db();
    let row = sample("test-disable");
    let id = row.id;
    create_enabled_for_test(&db, &row, None).await.unwrap();
    assert!(get(&db, id).await.unwrap().enabled);
    set_disabled(&db, id, Utc::now()).await.unwrap();
    assert!(!get(&db, id).await.unwrap().enabled);
}
