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

async fn insert_attempt(
    db: &Database,
    provider_id: FederationProviderId,
    now: DateTime<Utc>,
) -> FederationLoginAttemptRow {
    insert(
        db,
        provider_id,
        1,
        1,
        hash32(1),
        hash32(2),
        hash32(3),
        b"the-pkce-verifier",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
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

// ── Stage 3: the claim ─────────────────────────────────────────────────────

#[tokio::test]
async fn a_pending_row_is_claimed_and_becomes_exchanging() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let claim_time = now + Duration::seconds(1);
    let claimed = claim(&db, attempt.id, claim_time).await.unwrap();
    assert_eq!(claimed.status, FederationLoginAttemptStatus::Exchanging);
    assert_eq!(claimed.claimed_at, Some(claim_time));

    let (status, claimed_at, ..) = stored_row(&db, attempt.id).await;
    assert_eq!(status, "exchanging");
    assert!(!claimed_at.is_empty());
}

#[tokio::test]
async fn claiming_a_nonexistent_attempt_is_not_found() {
    let db = open_test_db();
    let result = claim(&db, FederationLoginAttemptId::new(), fixed_now()).await;
    assert!(matches!(result, Err(StoreError::NotFound)));
}

/// RFC 096-B1 stage 3, item 2: a claimed row cannot be re-claimed.
#[tokio::test]
async fn an_already_claimed_row_cannot_be_reclaimed() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;
    claim(&db, attempt.id, now + Duration::seconds(1))
        .await
        .unwrap();

    let result = claim(&db, attempt.id, now + Duration::seconds(2)).await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

/// RFC 096-B1 stage 3, item 2: a non-`pending` row cannot be claimed at all
/// -- `completed`/`failed`, not just `exchanging`, by directly setting the
/// status to something `claim` never produces itself.
#[tokio::test]
async fn a_non_pending_row_cannot_be_claimed() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;
    let id_str = attempt.id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE federation_login_attempt SET status = 'failed', claimed_at = ?1 WHERE id = ?2",
            params![now, id_str],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    let result = claim(&db, attempt.id, now + Duration::seconds(1)).await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

/// RFC 096-B1 stage 3, item 3: the clock-regression test, with its own
/// error distinct from "expired".
#[tokio::test]
async fn a_claim_before_created_at_is_refused_as_clock_regression_not_expiry() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let result = claim(&db, attempt.id, now - Duration::seconds(1)).await;
    assert!(matches!(result, Err(StoreError::ClockRegression)));

    // The row must still be claimable afterward -- the refused attempt must
    // not have been consumed by the regression check.
    let claimed = claim(&db, attempt.id, now + Duration::seconds(1)).await;
    assert!(claimed.is_ok());
}

#[tokio::test]
async fn a_claim_at_or_after_expires_at_is_refused_as_expired() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let result = claim(&db, attempt.id, attempt.expires_at).await;
    assert!(matches!(result, Err(StoreError::AttemptExpired)));
}

#[tokio::test]
async fn a_claim_just_before_expires_at_succeeds() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let result = claim(&db, attempt.id, attempt.expires_at - Duration::seconds(1)).await;
    assert!(result.is_ok());
}

/// RFC 096-B1 stage 3, item 4: the verifier open is doing the tamper check,
/// not a hand-written field comparison -- altering a bound column (here,
/// `provider_config_version`) after `insert` and before `claim` must make
/// the claim fail, via the AAD no longer matching the sealed verifier.
#[tokio::test]
async fn claim_fails_if_a_bound_column_is_altered_after_insert() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let id_str = attempt.id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE federation_login_attempt SET provider_config_version = provider_config_version + 1 \
             WHERE id = ?1",
            [id_str],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    let result = claim(&db, attempt.id, now + Duration::seconds(1)).await;
    assert!(matches!(result, Err(StoreError::Crypto)));
}

/// The read-time `status != Pending` check is not purely redundant with
/// the conditional `UPDATE`'s own `WHERE status = 'pending'` guard, even
/// though both independently refuse a non-`pending` row with the same
/// `Conflict` -- removing the read-time check as a mutation changed no
/// test outcome *until this one*: it fixes *priority* when a row is both
/// non-`pending` and would independently fail the clock-regression check.
/// Without the early check, clock-regression would be evaluated first and
/// `ClockRegression` returned instead of `Conflict` for a `failed` row.
/// This pins the priority this stage actually intends: state wins over
/// time.
#[tokio::test]
async fn a_non_pending_row_reports_conflict_even_if_also_clock_regressed() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;
    let id_str = attempt.id.to_string();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE federation_login_attempt SET status = 'failed', claimed_at = ?1 WHERE id = ?2",
            params![now, id_str],
        )?;
        Ok(())
    })
    .await
    .unwrap();

    // A claim time *before* created_at: would be ClockRegression on its
    // own, were the row still pending.
    let result = claim(&db, attempt.id, now - Duration::seconds(1)).await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

/// RFC 096-B1 stage 3, item 1: two concurrent claims on the same `pending`
/// attempt yield exactly one winner. A genuine race, not two sequential
/// calls: both futures are polled together by `tokio::join!`, and
/// `claim`'s own two-step shape (a `with_read` then a separate `with_conn`)
/// gives real schedule-dependent interleaving even though `SqliteBackend`
/// serializes the actual connection access through one `Mutex` -- the
/// window this test depends on is between the two callers' reads and
/// their writes, not within SQLite itself.
#[tokio::test]
async fn exactly_one_of_two_concurrent_claims_wins() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let attempt = insert_attempt(&db, provider_id, now).await;

    let claim_time = now + Duration::seconds(1);
    let (a, b) = tokio::join!(
        claim(&db, attempt.id, claim_time),
        claim(&db, attempt.id, claim_time),
    );
    let outcomes = [a.is_ok(), b.is_ok()];
    assert_eq!(
        outcomes.iter().filter(|ok| **ok).count(),
        1,
        "exactly one of the two concurrent claims must win: {outcomes:?}"
    );
    for result in [a, b] {
        if let Err(e) = result {
            assert!(
                matches!(e, StoreError::Conflict),
                "the loser must be Conflict, got {e:?}"
            );
        }
    }
}

// ── Stage 4: lookup by state_sha256 ────────────────────────────────────────

#[tokio::test]
async fn an_attempt_is_found_by_its_own_state_hash() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let state = hash32(42);
    let attempt = insert(
        &db,
        provider_id,
        1,
        1,
        state,
        hash32(2),
        hash32(3),
        b"v",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
    .await
    .unwrap();

    let found = find_by_state_sha256(&db, state).await.unwrap();
    assert_eq!(found.id, attempt.id);
}

#[tokio::test]
async fn an_unknown_state_hash_is_not_found() {
    let db = open_test_db();
    let result = find_by_state_sha256(&db, hash32(99)).await;
    assert!(matches!(result, Err(StoreError::NotFound)));
}

/// `state_sha256 UNIQUE` (migration 0046) is what makes this lookup
/// well-defined at all -- two attempts can never share a state hash, so
/// "found by state" never has to pick among candidates.
#[tokio::test]
async fn a_second_attempt_cannot_reuse_a_state_hash_already_in_use() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let now = fixed_now();
    let state = hash32(7);
    insert(
        &db,
        provider_id,
        1,
        1,
        state,
        hash32(2),
        hash32(3),
        b"v",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
    .await
    .unwrap();

    let result = insert(
        &db,
        provider_id,
        1,
        1,
        state,
        hash32(20),
        hash32(30),
        b"v2",
        "https://rp.example/cb".to_owned(),
        None,
        now,
    )
    .await;
    assert!(result.is_err());
}
