use super::*;
use crate::crypto::MasterKey;
use crate::models::UserRow;

// No `#[cfg(test)] mod tests;` existed for this repo module before RFC
// 096-B1 stage 11 — every function below this line was previously
// exercised only indirectly, through `authn::webauthn`'s own
// integration tests (`sui-id-core`). The three new functions
// ([`current_passkey_enc_within_tx`], [`has_any_within_tx`],
// [`swap_passkey_within_tx`]) are store-internal guards F03/F06 call
// durably inside a transaction and have no handler to exercise them
// through yet (stage 11's own "no handler" constraint) — this module
// is where they get a direct test instead.

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
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
    crate::repos::users::create(db, &row).await.unwrap();
    id
}

fn credential_row(user_id: UserId, tag: u8) -> UserWebauthnCredentialRow {
    UserWebauthnCredentialRow {
        id: WebauthnCredentialId::new(),
        user_id,
        credential_id: vec![tag; 16],
        passkey_enc: vec![0xAA, tag],
        nickname: "Passkey".to_owned(),
        created_at: Utc::now(),
        last_used_at: None,
    }
}

#[tokio::test]
async fn current_passkey_enc_within_tx_returns_the_stored_blob() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    let row = credential_row(user_id, 1);
    let id = row.id;
    create(&db, &row, b"plaintext").await.unwrap();
    let expected = find_by_credential_id(&db, &row.credential_id)
        .await
        .unwrap()
        .unwrap()
        .passkey_enc;

    let found = db
        .with_conn(move |conn| current_passkey_enc_within_tx(conn, id))
        .await
        .unwrap();
    assert_eq!(found, Some(expected));
}

#[tokio::test]
async fn current_passkey_enc_within_tx_returns_none_for_an_unknown_id() {
    let db = open_test_db();
    let found = db
        .with_conn(move |conn| current_passkey_enc_within_tx(conn, WebauthnCredentialId::new()))
        .await
        .unwrap();
    assert!(found.is_none());
}

/// The WebAuthn analogue of TOTP's `FactorChanged`: once the credential
/// is deleted, [`current_passkey_enc_within_tx`] must flip to `None`,
/// not keep reporting a blob that no longer has a row.
#[tokio::test]
async fn current_passkey_enc_within_tx_returns_none_after_the_credential_is_deleted() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    let row = credential_row(user_id, 2);
    let id = row.id;
    create(&db, &row, b"plaintext").await.unwrap();
    delete(&db, id, user_id).await.unwrap();

    let found = db
        .with_conn(move |conn| current_passkey_enc_within_tx(conn, id))
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn has_any_within_tx_is_false_with_no_credentials() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    let has_any = db
        .with_conn(move |conn| has_any_within_tx(conn, user_id))
        .await
        .unwrap();
    assert!(!has_any);
}

#[tokio::test]
async fn has_any_within_tx_is_true_with_one_credential() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    create(&db, &credential_row(user_id, 3), b"plaintext")
        .await
        .unwrap();
    let has_any = db
        .with_conn(move |conn| has_any_within_tx(conn, user_id))
        .await
        .unwrap();
    assert!(has_any);
}

/// `has_any_within_tx` is per-user — a credential belonging to a
/// different user must not count.
#[tokio::test]
async fn has_any_within_tx_does_not_count_another_users_credential() {
    let db = open_test_db();
    let owner = seed_user(&db).await;
    let other = seed_user(&db).await;
    create(&db, &credential_row(owner, 4), b"plaintext")
        .await
        .unwrap();
    let has_any = db
        .with_conn(move |conn| has_any_within_tx(conn, other))
        .await
        .unwrap();
    assert!(!has_any);
}

#[tokio::test]
async fn swap_passkey_within_tx_replaces_the_blob_when_expected_matches() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    let row = credential_row(user_id, 5);
    let id = row.id;
    // `create` seals `passkey_json_plain` itself (`AAD`, `db.key()`) and
    // stores *that*, ignoring `row.passkey_enc` entirely — the real
    // stored blob has to be read back, not assumed from the fixture.
    create(&db, &row, b"plaintext").await.unwrap();
    let expected = find_by_credential_id(&db, &row.credential_id)
        .await
        .unwrap()
        .expect("credential exists")
        .passkey_enc;

    let new_sealed = vec![0xBB, 5];
    let now = Utc::now();
    db.with_conn({
        let expected = expected.clone();
        let new_sealed = new_sealed.clone();
        move |conn| swap_passkey_within_tx(conn, id, &expected, &new_sealed, now)
    })
    .await
    .expect("expected blob matches, swap succeeds");

    let found = find_by_credential_id(&db, &row.credential_id)
        .await
        .unwrap()
        .expect("credential still exists");
    assert_eq!(found.passkey_enc, new_sealed);
    assert_eq!(found.last_used_at, Some(now));
}

/// The compare-and-swap guard: a stale `expected_sealed` (the blob
/// already moved since it was read) must refuse rather than overwrite
/// silently — the same shape
/// `user_totp::swap_recovery_codes_within_tx`'s own guard already
/// takes for the recovery-code blob.
#[tokio::test]
async fn swap_passkey_within_tx_refuses_when_expected_is_stale() {
    let db = open_test_db();
    let user_id = seed_user(&db).await;
    let row = credential_row(user_id, 6);
    let id = row.id;
    create(&db, &row, b"plaintext").await.unwrap();
    let original = find_by_credential_id(&db, &row.credential_id)
        .await
        .unwrap()
        .expect("credential exists")
        .passkey_enc;

    let wrong_expected = vec![0xFF, 0xFF];
    let new_sealed = vec![0xBB, 6];
    let err = db
        .with_conn(move |conn| {
            swap_passkey_within_tx(conn, id, &wrong_expected, &new_sealed, Utc::now())
        })
        .await
        .expect_err("stale expected blob must not swap");
    assert!(matches!(err, StoreError::NotFound));

    let found = find_by_credential_id(&db, &row.credential_id)
        .await
        .unwrap()
        .expect("credential still exists");
    assert_eq!(
        found.passkey_enc, original,
        "the blob must be unchanged after a refused swap"
    );
}

#[tokio::test]
async fn swap_passkey_within_tx_refuses_when_the_credential_is_gone() {
    let db = open_test_db();
    let err = db
        .with_conn(move |conn| {
            swap_passkey_within_tx(
                conn,
                WebauthnCredentialId::new(),
                b"expected",
                b"new",
                Utc::now(),
            )
        })
        .await
        .expect_err("an unknown credential id must refuse");
    assert!(matches!(err, StoreError::NotFound));
}
