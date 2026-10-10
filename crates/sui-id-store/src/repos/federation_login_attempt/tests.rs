use super::*;
use crate::crypto::MasterKey;
use crate::models::FederationLoginAttemptStatus;
use crate::repos::federation_provider;
use crate::{Database, models::FederationProviderRow};

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "test-provider".to_owned(),
        display_name: "Test Provider".to_owned(),
        issuer: "https://idp.example".to_owned(),
        client_id: "client-1".to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let id = row.id;
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

fn hash32(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn fixed_now() -> DateTime<Utc> {
    "2026-10-10T12:00:00Z".parse().unwrap()
}

async fn stored_row(db: &Database, id: FederationLoginAttemptId) -> (String, String, i64, i64) {
    let id_str = id.to_string();
    db.with_read(move |read| {
        let mut stmt = read.prepare(
            "SELECT status, claimed_at, provider_config_version, provider_activation_generation \
             FROM federation_login_attempt WHERE id = ?1",
        )?;
        let row = stmt.query_row([id_str], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        Ok(row)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn a_pending_row_is_inserted_and_readable() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let row = insert(
        &db,
        provider_id,
        3,
        2,
        hash32(1),
        hash32(2),
        hash32(3),
        b"pkce-verifier-plaintext",
        "https://rp.example/cb".to_owned(),
        None,
        fixed_now(),
    )
    .await
    .unwrap();

    assert_eq!(row.status, FederationLoginAttemptStatus::Pending);
    assert_eq!(row.claimed_at, None);

    let (status, claimed_at, version, generation) = stored_row(&db, row.id).await;
    assert_eq!(status, "pending");
    assert_eq!(claimed_at, "");
    assert_eq!(version, 3);
    assert_eq!(generation, 2);
}

/// Stage 1's own table `CHECK` requires exactly `status = 'pending' AND
/// claimed_at IS NULL` for a fresh row — this is the insert side of that
/// invariant, proving `insert` actually produces a row the `CHECK` accepts
/// rather than merely trusting it compiles.
#[tokio::test]
async fn the_inserted_row_satisfies_the_pending_claimed_at_check() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    // If the row violated stage 1's CHECK, this INSERT -- not a later
    // read -- would fail with a constraint error.
    insert(
        &db,
        provider_id,
        1,
        1,
        hash32(1),
        hash32(2),
        hash32(3),
        b"v",
        "https://rp.example/cb".to_owned(),
        None,
        fixed_now(),
    )
    .await
    .expect("a freshly started attempt must satisfy stage 1's own CHECK");
}

/// RFC 096 `:588-589`: 600-second lifetime, derived from one clock sample.
#[tokio::test]
async fn expires_at_is_exactly_600_seconds_after_created_at() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let row = insert(
        &db,
        provider_id,
        1,
        1,
        hash32(1),
        hash32(2),
        hash32(3),
        b"v",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
    .await
    .unwrap();
    assert_eq!(row.created_at, now);
    assert_eq!(row.expires_at, now + Duration::seconds(600));
    assert_eq!(
        row.expires_at - row.created_at,
        Duration::seconds(ATTEMPT_LIFETIME_SECS)
    );
}

/// The dispatch's own requirement: opening the verifier fails if any one of
/// the four bound values changes. This is the test that makes the AAD
/// binding real rather than decorative.
#[tokio::test]
async fn opening_the_verifier_fails_if_any_bound_value_changes() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let other_provider_id = FederationProviderId::new();
    let plaintext = b"the-pkce-verifier";
    let row = insert(
        &db,
        provider_id,
        5,
        9,
        hash32(1),
        hash32(2),
        hash32(3),
        plaintext,
        "https://rp.example/cb".to_owned(),
        None,
        fixed_now(),
    )
    .await
    .unwrap();

    let key = db.key();
    let correct_aad = build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
    );
    let opened = crate::crypto::open(key, &row.pkce_verifier_sealed, &correct_aad).unwrap();
    assert_eq!(opened, plaintext);

    let wrong_id_aad = build_aad(
        FederationLoginAttemptId::new(),
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
    );
    assert!(crate::crypto::open(key, &row.pkce_verifier_sealed, &wrong_id_aad).is_err());

    let wrong_provider_aad = build_aad(
        row.id,
        other_provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
    );
    assert!(crate::crypto::open(key, &row.pkce_verifier_sealed, &wrong_provider_aad).is_err());

    let wrong_version_aad = build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version + 1,
        row.provider_activation_generation,
    );
    assert!(crate::crypto::open(key, &row.pkce_verifier_sealed, &wrong_version_aad).is_err());

    let wrong_generation_aad = build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation + 1,
    );
    assert!(crate::crypto::open(key, &row.pkce_verifier_sealed, &wrong_generation_aad).is_err());
}

/// `build_aad`'s safety argument (module doc) rests on neither a UUID
/// string nor a decimal integer ever containing the NUL separator. Checked
/// directly against a real generated id rather than assumed from the UUID
/// spec.
#[test]
fn id_strings_never_contain_the_aad_separator() {
    let id = FederationLoginAttemptId::new();
    let provider_id = FederationProviderId::new();
    assert!(!id.to_string().contains('\0'));
    assert!(!provider_id.to_string().contains('\0'));
    assert!(!42i64.to_string().contains('\0'));
    assert!(!(-1i64).to_string().contains('\0'));
}

/// The concrete reason the separator is load-bearing, not merely tidy:
/// without it, `version=1, generation=23` and `version=12, generation=3`
/// both concatenate to the digit string `"123"` and would produce the same
/// AAD -- two distinct attempts' verifiers would be interchangeable.
#[test]
fn distinct_version_generation_pairs_that_would_collide_unseparated_do_not() {
    let id = FederationLoginAttemptId::new();
    let provider_id = FederationProviderId::new();
    let a = build_aad(id, provider_id, 1, 23);
    let b = build_aad(id, provider_id, 12, 3);
    assert_ne!(a, b);
}
