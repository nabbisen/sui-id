//! RFC 094 M2a, C15 — dynamic client registration, store-level: the
//! injected-failure rollback the HTTP-level e2e tests cannot reach (the
//! fault injector is `pub(crate)` to this crate).

use super::*;
use crate::repos::client_registration_token::RegistrationTokenRow;
use sui_id_shared::ids::RegistrationTokenId;

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(input.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

async fn seed_token(db: &Database, max_uses: i64) -> (RegistrationTokenId, String) {
    let plaintext = format!("regtoken-{}", uuid::Uuid::new_v4());
    let id = RegistrationTokenId::new();
    let row = RegistrationTokenRow {
        id,
        token_hash: sha256_hex(&plaintext),
        max_uses,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    repos::client_registration_token::create(db, &row)
        .await
        .expect("seed token");
    (id, plaintext)
}

async fn used_count(db: &Database, id: RegistrationTokenId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT used_count FROM client_registration_token WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("read used_count")
}

async fn client_row_count(db: &Database) -> i64 {
    db.with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM clients", [], |r| r.get(0))?))
        .await
        .expect("count clients")
}

#[tokio::test]
async fn c15_registers_consumes_the_token_once_and_stamps_registered_via() {
    let db = fresh_db();
    let (token_id, plaintext) = seed_token(&db, 1).await;
    let mut row = a_client();
    row.registered_via = crate::models::RegistrationSource::Dynamic;
    let client_id = row.id;
    let before = audit_rows(&db).await;

    let hash = sha256_hex(&plaintext);
    let result = crate::commands::register_client_dynamically(&db, hash, row, Utc::now()).await;
    result.expect("registration succeeds");

    assert_eq!(used_count(&db, token_id).await, 1);
    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("row exists");
    assert_eq!(
        stored.registered_via,
        crate::models::RegistrationSource::Dynamic
    );
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("client.dynamic_register")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C15");
}

#[tokio::test]
async fn c15_an_invalid_token_rolls_back_and_writes_nothing() {
    let db = fresh_db();
    let row = a_client();

    // No token was ever seeded: any hash is "invalid, expired, revoked, or
    // exhausted" from `consume_within_tx`'s point of view.
    let result = crate::commands::register_client_dynamically(
        &db,
        "no-such-hash".to_owned(),
        row,
        Utc::now(),
    )
    .await;

    assert!(matches!(result, Err(StoreError::NotFound)));
    assert_eq!(client_row_count(&db).await, 0, "no client row was created");
    assert_eq!(
        repos::audit::recent(&db, 1)
            .await
            .expect("audit tail")
            .len(),
        0,
        "no audit row was written"
    );
}

/// The clause M2a states for every converted row: an injected failure in
/// the audit append rolls back the whole transaction, including the token
/// spend — not merely the client row.
#[tokio::test]
async fn c15_injected_append_failure_leaves_the_token_unspent_and_no_client_row() {
    let db = fresh_db();
    let (token_id, plaintext) = seed_token(&db, 1).await;
    let row = a_client();

    db.fault_injector().fail_before_next_append();
    let hash = sha256_hex(&plaintext);
    let result = crate::commands::register_client_dynamically(&db, hash, row, Utc::now()).await;

    assert!(result.is_err(), "the injected failure surfaces");
    assert_eq!(
        used_count(&db, token_id).await,
        0,
        "the token was not spent -- the whole transaction rolled back"
    );
    assert_eq!(client_row_count(&db).await, 0, "no client row was created");
    assert_eq!(
        repos::audit::recent(&db, 1)
            .await
            .expect("audit tail")
            .len(),
        0,
        "no audit row was written"
    );
    record_rollback_coverage("C15");
}
