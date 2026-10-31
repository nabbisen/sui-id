//! RFC 094 M2b wave 4 stage 4, C22 — revoke a client-registration
//! token. CLI-only, no authenticated actor; the revocation and the
//! `client.registration_token.revoked` audit append now commit in one
//! transaction, replacing a bare `revoke` that had neither a caller nor
//! an event at all.
//!
//! Cycle I stage 3, C14 — issue a registration token. Mirrors C22
//! exactly: CLI-only, no authenticated actor, the create half of the
//! same pair. `client_registration_token::create` had no event at all.

use super::*;
use sui_id_shared::ids::RegistrationTokenId;

fn sample_row(id: RegistrationTokenId) -> repos::client_registration_token::RegistrationTokenRow {
    let now = Utc::now();
    repos::client_registration_token::RegistrationTokenRow {
        id,
        token_hash: format!("hash-{id}"),
        max_uses: 1,
        used_count: 0,
        expires_at: None,
        revoked_at: None,
        note: None,
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

#[tokio::test]
async fn a_c14_registration_issue() {
    let db = fresh_db();
    let id = RegistrationTokenId::new();
    let before = audit_rows(&db).await;

    crate::commands::issue_registration_token(&db, sample_row(id))
        .await
        .expect("issue")
        .into_inner();

    let row = repos::client_registration_token::get(&db, id)
        .await
        .expect("the token must exist");
    assert_eq!(row.id, id);

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.registration_token.created");
    assert_eq!(event.note, None, "no attributes");
    assert_eq!(event.actor, None, "CLI-only, no authenticated actor");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C14");
}

#[tokio::test]
async fn c14_injected_failure_before_append_leaves_no_token_stored() {
    let db = fresh_db();
    let id = RegistrationTokenId::new();
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::issue_registration_token(&db, sample_row(id)).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let err = repos::client_registration_token::get(&db, id)
        .await
        .expect_err("the insert rolled back: no token was stored");
    assert!(matches!(err, StoreError::NotFound));
    assert_eq!(latest_audit_action(&db).await, before_audit, "no event");
    record_rollback_coverage("C14");
}

#[tokio::test]
async fn a_c22_registration_revoke() {
    let db = fresh_db();
    let id = RegistrationTokenId::new();
    repos::client_registration_token::create(&db, &sample_row(id))
        .await
        .expect("seed token");
    let before = audit_rows(&db).await;

    let audited = crate::commands::revoke_registration_token(&db, id, Utc::now())
        .await
        .expect("revoke");
    audited.into_inner();

    let row = repos::client_registration_token::find_by_hash(&db, &format!("hash-{id}"))
        .await
        .expect("find")
        .expect("row exists");
    assert!(row.revoked_at.is_some(), "the token must be revoked");

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.registration_token.revoked");
    assert_eq!(event.note, None, "no attributes");
    assert_eq!(event.actor, None, "CLI-only, no authenticated actor");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C22");
}

#[tokio::test]
async fn c22_injected_failure_before_append_rolls_back_the_revoke() {
    let db = fresh_db();
    let id = RegistrationTokenId::new();
    repos::client_registration_token::create(&db, &sample_row(id))
        .await
        .expect("seed token");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::revoke_registration_token(&db, id, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::client_registration_token::find_by_hash(&db, &format!("hash-{id}"))
        .await
        .expect("find")
        .expect("row exists");
    assert!(
        row.revoked_at.is_none(),
        "the revoke must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C22");
}

#[tokio::test]
async fn c22_against_a_nonexistent_token_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let ghost = RegistrationTokenId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::revoke_registration_token(&db, ghost, Utc::now()).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}

/// The dispatch's required measurement, restated as a permanent test:
/// `consume` must refuse a revoked token, not merely be assumed to.
#[tokio::test]
async fn a_revoked_token_is_refused_by_consume() {
    let db = fresh_db();
    let id = RegistrationTokenId::new();
    let hash = format!("hash-{id}");
    repos::client_registration_token::create(&db, &sample_row(id))
        .await
        .expect("seed token");

    crate::commands::revoke_registration_token(&db, id, Utc::now())
        .await
        .expect("revoke");

    let ok = repos::client_registration_token::consume(&db, &hash, Utc::now())
        .await
        .expect("consume");
    assert!(!ok, "a revoked token must not be consumable");
}
