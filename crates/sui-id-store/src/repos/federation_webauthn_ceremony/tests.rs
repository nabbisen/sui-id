use super::*;
use crate::crypto::MasterKey;
use crate::models::{
    FederationLinkRow, FederationMfaPendingRow, FederationMfaPendingStatus, ProvisionMode, UserRow,
};
use crate::repos::{federation_link, federation_mfa_pending, federation_provider, users};
use crate::{Database, models::FederationProviderRow};
use chrono::Duration;

// This module's own fixtures, not shared with
// `federation_mfa_pending::tests` (a sibling private module) — the same
// provider/user/link/pending-row shape, built fresh here because stage
// 8's own raw-SQL suite (`tests_rfc096_b1_stage8.rs`) already proves
// every schema-level invariant (`UNIQUE(pending_id)`, the composite
// `FOREIGN KEY`, the `expires_at <= pending_expires_at` `CHECK`, cascade
// on delete/unlink) directly against the migration. What is left for
// this module's own tests to prove is narrower: that
// [`insert_or_replace_within_tx`]/[`find_by_pending_id_within_tx`]/
// [`consume_within_tx`]/[`delete_by_pending_id_within_tx`] round-trip
// and behave correctly as *functions*, not that the table enforces its
// own shape (already proven).

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
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
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

async fn seed_user(db: &Database) -> UserId {
    let id = UserId::new();
    let now = chrono::Utc::now();
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
    let now = chrono::Utc::now();
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

/// Mirrors `federation_mfa_pending::tests::seed_pending_row` (a sibling
/// private module, not reusable from here) — just enough of a parent
/// row for the composite `FOREIGN KEY` to accept a ceremony against it.
async fn seed_pending_row(
    db: &Database,
    provider_id: FederationProviderId,
    user_id: UserId,
    now: chrono::DateTime<chrono::Utc>,
) -> FederationMfaPendingRow {
    let id = FederationMfaPendingId::new();
    let aad = federation_mfa_pending::build_aad(id, provider_id, 0, 0, user_id);
    let continuation_sealed = crate::crypto::seal(db.key(), &[], &aad).unwrap();
    let row = FederationMfaPendingRow {
        id,
        provider_id,
        provider_config_version: 0,
        provider_activation_generation: 0,
        user_id,
        continuation_sealed: Some(continuation_sealed),
        next_path: None,
        browser_binding_sha256: [1u8; 32],
        csrf_binding_sha256: [2u8; 32],
        failure_count: 0,
        status: FederationMfaPendingStatus::Pending,
        created_at: now,
        expires_at: now + Duration::seconds(federation_mfa_pending::PENDING_LIFETIME_SECS),
    };
    db.with_conn({
        let row = row.clone();
        move |conn| federation_mfa_pending::insert_within_tx(conn, &row)
    })
    .await
    .unwrap();
    row
}

fn ceremony_for(
    pending: &FederationMfaPendingRow,
    now: chrono::DateTime<chrono::Utc>,
) -> FederationWebauthnCeremonyRow {
    FederationWebauthnCeremonyRow {
        id: FederationWebauthnCeremonyId::new(),
        pending_id: pending.id,
        provider_id: pending.provider_id,
        provider_config_version: pending.provider_config_version,
        provider_activation_generation: pending.provider_activation_generation,
        user_id: pending.user_id,
        ceremony_state_sealed: b"opaque-sealed-bytes".to_vec(),
        rp_id: "idp.example".to_owned(),
        origin: "https://idp.example".to_owned(),
        created_at: now,
        pending_expires_at: pending.expires_at,
        expires_at: now + Duration::seconds(60),
    }
}

async fn full_fixture(db: &Database) -> FederationMfaPendingRow {
    let provider_id = seed_provider(db).await;
    let user_id = seed_user(db).await;
    seed_link(db, user_id, provider_id).await;
    seed_pending_row(
        db,
        provider_id,
        user_id,
        "2026-10-15T12:00:00Z".parse().unwrap(),
    )
    .await
}

#[tokio::test]
async fn a_valid_ceremony_is_stored_and_found_by_pending_id() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let ceremony = ceremony_for(&pending, pending.created_at);
    let ceremony_id = ceremony.id;

    db.with_conn({
        let ceremony = ceremony.clone();
        move |conn| insert_or_replace_within_tx(conn, &ceremony)
    })
    .await
    .expect("valid ceremony");

    let found = find_by_pending_id(&db, pending.id)
        .await
        .expect("read")
        .expect("ceremony exists");
    assert_eq!(found.id, ceremony_id);
    assert_eq!(found.pending_id, pending.id);
    assert_eq!(found.provider_id, pending.provider_id);
    assert_eq!(found.user_id, pending.user_id);
    assert_eq!(found.ceremony_state_sealed, ceremony.ceremony_state_sealed);
    assert_eq!(found.rp_id, ceremony.rp_id);
    assert_eq!(found.origin, ceremony.origin);
}

#[tokio::test]
async fn find_by_pending_id_returns_none_when_no_ceremony_exists() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    assert!(find_by_pending_id(&db, pending.id).await.unwrap().is_none());
}

/// RFC 096 `:736`: "atomically replace at most one ... ceremony." A
/// second [`insert_or_replace_within_tx`] call for the same `pending_id`
/// must leave exactly one row — the *second* one, by identity — not
/// two, and not a conflict error (unlike every other guarded insert in
/// this crate's federation tables, this one is specified to replace,
/// not refuse).
#[tokio::test]
async fn a_second_insert_for_the_same_pending_id_replaces_the_first() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let first = ceremony_for(&pending, pending.created_at);
    let second = ceremony_for(&pending, pending.created_at);
    assert_ne!(first.id, second.id);

    db.with_conn({
        let first = first.clone();
        move |conn| insert_or_replace_within_tx(conn, &first)
    })
    .await
    .unwrap();
    db.with_conn({
        let second = second.clone();
        move |conn| insert_or_replace_within_tx(conn, &second)
    })
    .await
    .unwrap();

    let found = find_by_pending_id(&db, pending.id).await.unwrap().unwrap();
    assert_eq!(found.id, second.id);

    let count: i64 = db
        .with_read(|read| {
            Ok(read
                .prepare("SELECT COUNT(*) FROM federation_webauthn_ceremony")?
                .query_row([], |r| r.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn consume_within_tx_deletes_the_named_ceremony() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let ceremony = ceremony_for(&pending, pending.created_at);
    let ceremony_id = ceremony.id;
    db.with_conn({
        let ceremony = ceremony.clone();
        move |conn| insert_or_replace_within_tx(conn, &ceremony)
    })
    .await
    .unwrap();

    let consumed = db
        .with_conn(move |conn| consume_within_tx(conn, ceremony_id))
        .await
        .unwrap();
    assert!(consumed);
    assert!(find_by_pending_id(&db, pending.id).await.unwrap().is_none());
}

/// [`consume_within_tx`] is guarded on `id`, not `pending_id` — a stale
/// id (one already replaced away by a concurrent F06 call) must not
/// delete whatever ceremony is *currently* bound to the row.
#[tokio::test]
async fn consume_within_tx_does_not_delete_a_different_ceremony_for_the_same_pending_row() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let stale = ceremony_for(&pending, pending.created_at);
    let stale_id = stale.id;
    db.with_conn({
        let stale = stale.clone();
        move |conn| insert_or_replace_within_tx(conn, &stale)
    })
    .await
    .unwrap();
    let fresh = ceremony_for(&pending, pending.created_at);
    let fresh_id = fresh.id;
    db.with_conn({
        let fresh = fresh.clone();
        move |conn| insert_or_replace_within_tx(conn, &fresh)
    })
    .await
    .unwrap();

    let consumed = db
        .with_conn(move |conn| consume_within_tx(conn, stale_id))
        .await
        .unwrap();
    assert!(!consumed, "the stale id must not report a consumption");
    let found = find_by_pending_id(&db, pending.id).await.unwrap().unwrap();
    assert_eq!(found.id, fresh_id, "the current ceremony must survive");
}

#[tokio::test]
async fn delete_by_pending_id_within_tx_is_idempotent_when_nothing_exists() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let deleted = db
        .with_conn(move |conn| delete_by_pending_id_within_tx(conn, pending.id))
        .await
        .unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn delete_by_pending_id_within_tx_deletes_whichever_ceremony_is_bound() {
    let db = open_test_db();
    let pending = full_fixture(&db).await;
    let ceremony = ceremony_for(&pending, pending.created_at);
    db.with_conn({
        let ceremony = ceremony.clone();
        move |conn| insert_or_replace_within_tx(conn, &ceremony)
    })
    .await
    .unwrap();

    let deleted = db
        .with_conn(move |conn| delete_by_pending_id_within_tx(conn, pending.id))
        .await
        .unwrap();
    assert!(deleted);
    assert!(find_by_pending_id(&db, pending.id).await.unwrap().is_none());
}

#[tokio::test]
async fn build_aad_differs_when_any_bound_column_differs() {
    let provider_a = FederationProviderId::new();
    let provider_b = FederationProviderId::new();
    let user_a = UserId::new();
    let pending_a = FederationMfaPendingId::new();
    let pending_b = FederationMfaPendingId::new();
    let base = build_aad(pending_a, provider_a, 0, 0, user_a);
    assert_ne!(base, build_aad(pending_b, provider_a, 0, 0, user_a));
    assert_ne!(base, build_aad(pending_a, provider_b, 0, 0, user_a));
    assert_ne!(base, build_aad(pending_a, provider_a, 1, 0, user_a));
    assert_ne!(base, build_aad(pending_a, provider_a, 0, 1, user_a));
    assert_ne!(base, build_aad(pending_a, provider_a, 0, 0, UserId::new()));
}
