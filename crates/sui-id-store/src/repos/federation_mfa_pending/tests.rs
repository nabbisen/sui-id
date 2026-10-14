use super::*;
use crate::crypto::MasterKey;
use crate::models::{FederationLinkRow, ProvisionMode, UserRow};
use crate::repos::{federation_link, federation_provider, users};
use crate::{Database, models::FederationProviderRow};

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn fixed_now() -> DateTime<Utc> {
    "2026-10-14T12:00:00Z".parse().unwrap()
}

fn hash32(tag: u8) -> [u8; 32] {
    [tag; 32]
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let id = FederationProviderId::new();
    let row = FederationProviderRow {
        id,
        slug: format!("test-provider-{id}"),
        display_name: "Test Provider".to_owned(),
        issuer: "https://idp.example".to_owned(),
        client_id: "client-1".to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

async fn seed_user(db: &Database) -> UserId {
    let id = UserId::new();
    let now = Utc::now();
    let row = UserRow {
        id,
        username: id.to_string(),
        display_name: None,
        is_admin: false,
        role: crate::models::Role::User,
        is_disabled: false,
        is_deleted: false,
        user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
        email: None,
        preferred_lang: None,
        email_normalized: None,
        email_verified_at: None,
        last_login_at: None,
        source: crate::models::UserSource::Local,
        external_stable_id: None,
    };
    users::create(db, &row).await.unwrap();
    id
}

async fn seed_link(db: &Database, user_id: UserId, provider_id: FederationProviderId) {
    let now = Utc::now();
    federation_link::upsert(
        db,
        FederationLinkRow {
            user_id,
            provider_id,
            upstream_sub: "fixture-sub".to_owned(),
            upstream_email: None,
            linked_at: now,
            last_seen_at: now,
        },
    )
    .await
    .unwrap();
}

/// Builds a row sealed against `db`'s own key — the AAD must be sealed
/// with the same key `insert_within_tx`'s real caller
/// (`commands::issue_federated_mfa_pending`) would use.
fn seed_pending_row(
    db: &Database,
    provider_id: FederationProviderId,
    user_id: UserId,
    now: DateTime<Utc>,
) -> FederationMfaPendingRow {
    let id = FederationMfaPendingId::new();
    let provider_config_version = 0;
    let provider_activation_generation = 0;
    let aad = build_aad(
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
        user_id,
    );
    let continuation_sealed = crate::crypto::seal(db.key(), &[], &aad).unwrap();
    FederationMfaPendingRow {
        id,
        provider_id,
        provider_config_version,
        provider_activation_generation,
        user_id,
        continuation_sealed: Some(continuation_sealed),
        next_path: None,
        browser_binding_sha256: hash32(1),
        csrf_binding_sha256: hash32(2),
        failure_count: 0,
        status: crate::models::FederationMfaPendingStatus::Pending,
        created_at: now,
        expires_at: now + Duration::seconds(PENDING_LIFETIME_SECS),
    }
}

async fn stored_pending_count(db: &Database) -> i64 {
    db.with_read(|read| {
        Ok(read
            .prepare("SELECT COUNT(*) FROM federation_mfa_pending")?
            .query_row([], |r| r.get(0))?)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn a_valid_row_is_stored() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let row = seed_pending_row(&db, provider_id, user_id, fixed_now());

    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("valid row");
    assert_eq!(stored_pending_count(&db).await, 1);
}

/// RFC 096-B1 stage 10a: `find_by_id` is the `FederatedMfaVerifier`'s
/// own read of the full row — asserted here against every field it
/// added or newly exposes (`status`, `failure_count`), not only that a
/// row comes back.
#[tokio::test]
async fn find_by_id_returns_the_full_row() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let row = seed_pending_row(&db, provider_id, user_id, fixed_now());
    let id = row.id;
    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("valid row");

    let found = find_by_id(&db, id)
        .await
        .expect("read")
        .expect("row exists");
    assert_eq!(found.id, id);
    assert_eq!(found.user_id, user_id);
    assert_eq!(found.provider_id, provider_id);
    assert_eq!(
        found.status,
        crate::models::FederationMfaPendingStatus::Pending
    );
    assert_eq!(found.failure_count, 0);
    assert_eq!(found.browser_binding_sha256, row.browser_binding_sha256);
    assert_eq!(found.csrf_binding_sha256, row.csrf_binding_sha256);
}

/// The unknown-id case `find_by_id`'s own doc names: `Ok(None)`, not an
/// error — what makes "no such row" foldable into the same refusal path
/// as every other binding mismatch.
#[tokio::test]
async fn find_by_id_returns_none_for_an_unknown_id() {
    let db = open_test_db();
    let found = find_by_id(&db, FederationMfaPendingId::new())
        .await
        .expect("read");
    assert!(found.is_none());
}

/// RFC 096-B1 stage 9 fix round, R3, **narrowed by stage 10a's own fix
/// round, R2**: this test originally protected all three of `status`,
/// `failure_count` and `primary_method` against a later table rebuild
/// silently dropping one of migration 0047's own `DEFAULT`s. Stage 10a
/// changed `insert_within_tx` to name `status` and `failure_count`
/// explicitly (`models::FederationMfaPendingRow`'s own doc explains
/// why — the verifier needs to read a non-`pending` value back), which
/// reverses the premise for those two: this test now reads back values
/// it itself supplied through `seed_pending_row`, so dropping either
/// `DEFAULT` would not make it fail. Only `primary_method` is still
/// left to the table's own `DEFAULT` and still genuinely protected
/// here. `status`/`failure_count` are still read back below as an
/// ordinary round-trip check, not as a claim that this test would
/// catch a missing `DEFAULT` on either of them.
#[tokio::test]
async fn a_valid_row_leaves_primary_method_to_its_default() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let row = seed_pending_row(&db, provider_id, user_id, fixed_now());
    let id = row.id;

    db.with_conn(move |conn| insert_within_tx(conn, &row))
        .await
        .expect("valid row");

    let (status, failure_count, primary_method): (String, i64, String) = db
        .with_read(move |read| {
            Ok(read
                .prepare(
                    "SELECT status, failure_count, primary_method \
                         FROM federation_mfa_pending WHERE id = ?1",
                )?
                .query_row([id.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)
        })
        .await
        .expect("read back");
    assert_eq!(status, "pending");
    assert_eq!(failure_count, 0);
    assert_eq!(primary_method, "fed");
}

/// Stage 9's own "active link" recheck: the `FOREIGN KEY (user_id,
/// provider_id) REFERENCES federation_link` (migration 0047's fix round)
/// refuses a row for a pair that was never linked — atomic with the
/// insert, not a separate `SELECT` this write could race past.
#[tokio::test]
async fn a_row_for_an_unlinked_pair_is_a_conflict() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    // Deliberately no seed_link.
    let row = seed_pending_row(&db, provider_id, user_id, fixed_now());

    let err = db
        .with_conn(move |conn| insert_within_tx(conn, &row))
        .await
        .expect_err("an unlinked pair must be refused");
    assert!(matches!(err, StoreError::Conflict));
}

/// `continuation_sealed`'s whole point (module doc, answer 2): a
/// promotion must open it with the row's own current values as AAD, so
/// changing any one of the five bound columns out of band makes it fail
/// to open. Checked by mutating the *stored row* directly via raw SQL
/// (bypassing this crate's own write paths, exactly the scenario the seal
/// defends against) and reconstructing the AAD from what is stored now.
#[tokio::test]
async fn opening_the_continuation_fails_if_any_bound_column_changes() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let other_provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    let other_user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    seed_link(&db, other_user_id, other_provider_id).await;
    let row = seed_pending_row(&db, provider_id, user_id, fixed_now());
    let id = row.id;

    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("valid row");

    let key = db.key();
    // `continuation_sealed` is `Option` (stage 10a fix round, R1) --
    // this row is always the freshly-built, still-`pending` one, so it
    // is always `Some`.
    let sealed = row
        .continuation_sealed
        .as_deref()
        .expect("a freshly-built row is pending and has a sealed continuation");
    let correct_aad = build_aad(
        id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
        row.user_id,
    );
    assert!(crate::crypto::open(key, sealed, &correct_aad).is_ok());

    let wrong_id_aad = build_aad(
        FederationMfaPendingId::new(),
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
        row.user_id,
    );
    assert!(crate::crypto::open(key, sealed, &wrong_id_aad).is_err());

    let wrong_provider_aad = build_aad(
        id,
        other_provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
        row.user_id,
    );
    assert!(crate::crypto::open(key, sealed, &wrong_provider_aad).is_err());

    let wrong_version_aad = build_aad(
        id,
        row.provider_id,
        row.provider_config_version + 1,
        row.provider_activation_generation,
        row.user_id,
    );
    assert!(crate::crypto::open(key, sealed, &wrong_version_aad).is_err());

    let wrong_generation_aad = build_aad(
        id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation + 1,
        row.user_id,
    );
    assert!(crate::crypto::open(key, sealed, &wrong_generation_aad).is_err());

    let wrong_user_aad = build_aad(
        id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
        other_user_id,
    );
    assert!(crate::crypto::open(key, sealed, &wrong_user_aad).is_err());
}

#[tokio::test]
async fn purge_expired_removes_a_row_older_than_24_hours_past_expiry() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let old_now = fixed_now() - Duration::hours(25);
    let row = seed_pending_row(&db, provider_id, user_id, old_now);
    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("old row");

    let purge_now = fixed_now();
    let n = purge_expired(&db, purge_now).await.expect("purge");
    assert_eq!(n, 1);
    assert_eq!(stored_pending_count(&db).await, 0);
}

/// RFC 096-B1 stage 9, item 5: "the cascade test for the ceremony table"
/// — not a manual `DELETE` (migration 0047's fix round already proves
/// the `FOREIGN KEY ... ON DELETE CASCADE` reaches a manual delete), but
/// specifically *this purge's own* `DELETE`, since that is the one no
/// other stage's test exercises. There is no repo function for
/// `federation_webauthn_ceremony` yet (stage 11's job), so the ceremony
/// row is seeded with raw SQL matching migration 0047's own column list.
#[tokio::test]
async fn purge_expired_cascades_a_ceremony_whose_parent_it_removes() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let old_now = fixed_now() - Duration::hours(25);
    let row = seed_pending_row(&db, provider_id, user_id, old_now);
    let pending_expires_at = row.expires_at;
    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("old row");

    db.with_conn(move |conn| {
        conn.execute(
            "INSERT INTO federation_webauthn_ceremony( \
                 id, pending_id, provider_id, provider_config_version, \
                 provider_activation_generation, user_id, ceremony_state_sealed, rp_id, origin, \
                 created_at, pending_expires_at, expires_at) \
             VALUES (?1,?2,?3,0,0,?4,X'63657265656d6f6e79','rp.example', \
                      'https://rp.example',?5,?6,?5)",
            params![
                FederationMfaPendingId::new().to_string(),
                row.id.to_string(),
                row.provider_id.to_string(),
                row.user_id.to_string(),
                old_now,
                pending_expires_at,
            ],
        )?;
        Ok(())
    })
    .await
    .expect("seed ceremony");

    let ceremony_count_before: i64 = db
        .with_read(|read| {
            Ok(read
                .prepare("SELECT COUNT(*) FROM federation_webauthn_ceremony")?
                .query_row([], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(ceremony_count_before, 1, "the ceremony was seeded");

    let n = purge_expired(&db, fixed_now()).await.expect("purge");
    assert_eq!(n, 1, "exactly one pending row purged");
    assert_eq!(stored_pending_count(&db).await, 0);
    let ceremony_count_after: i64 = db
        .with_read(|read| {
            Ok(read
                .prepare("SELECT COUNT(*) FROM federation_webauthn_ceremony")?
                .query_row([], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(
        ceremony_count_after, 0,
        "the ceremony cascaded with the purged parent"
    );
}

/// RFC 096-B1 stage 9 fix round, R4: the sibling table's own
/// `federation_login_attempt/tests.rs::the_24_hour_boundary_is_strict`,
/// copied here since this purge is a copy of that purge's shape — a row
/// at *exactly* 24 hours past its own expiry survives, because the
/// predicate is a strict `<`, and one second further does not. (RFC 096
/// `:734` is this table's rule — *"Terminal rows retain no usable
/// continuation and are purged after 24 hours"*; `:605`, cited here
/// before, is `federation_login_attempt`'s own sentence, and says
/// *"periodically deleted after 24 hours"* rather than anything about a
/// boundary. The RFC fixes neither side of the boundary; the strict `<`
/// is this implementation's choice, matched to the sibling's, and this
/// test is what pins it.)
#[tokio::test]
async fn the_24_hour_boundary_is_strict() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let purge_now = fixed_now();
    let cutoff = purge_now - Duration::hours(24);

    let at_boundary = seed_pending_row(
        &db,
        provider_id,
        user_id,
        cutoff - Duration::seconds(PENDING_LIFETIME_SECS),
    );
    let at_boundary_id = at_boundary.id;
    db.with_conn(move |conn| insert_within_tx(conn, &at_boundary))
        .await
        .expect("at-boundary row");

    let one_second_past = seed_pending_row(
        &db,
        provider_id,
        user_id,
        cutoff - Duration::seconds(PENDING_LIFETIME_SECS) - Duration::seconds(1),
    );
    let one_second_past_id = one_second_past.id;
    db.with_conn(move |conn| insert_within_tx(conn, &one_second_past))
        .await
        .expect("one-second-past row");

    let deleted = purge_expired(&db, purge_now).await.expect("purge");
    assert_eq!(deleted, 1);

    let row_exists = |id: FederationMfaPendingId| {
        let db = &db;
        async move {
            let n: i64 = db
                .with_read(move |read| {
                    Ok(read
                        .prepare("SELECT COUNT(*) FROM federation_mfa_pending WHERE id = ?1")?
                        .query_row([id.to_string()], |r| r.get(0))?)
                })
                .await
                .expect("count");
            n == 1
        }
    };
    assert!(
        row_exists(at_boundary_id).await,
        "exactly 24 hours past expiry is not yet \"after 24 hours\""
    );
    assert!(!row_exists(one_second_past_id).await);
}

#[tokio::test]
async fn purge_expired_leaves_a_fresh_row_alone() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let user_id = seed_user(&db).await;
    seed_link(&db, user_id, provider_id).await;
    let now = fixed_now();
    let row = seed_pending_row(&db, provider_id, user_id, now);
    db.with_conn({
        let row = row.clone();
        move |conn| insert_within_tx(conn, &row)
    })
    .await
    .expect("fresh row");

    let n = purge_expired(&db, now).await.expect("purge");
    assert_eq!(n, 0);
    assert_eq!(stored_pending_count(&db).await, 1);
}
