use super::*;

#[tokio::test]
async fn k01_rotates_key_and_appends_audit_row() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let new_id = SigningKeyId::new();
    let before = audit_rows(&db).await;
    let audited = rotate_signing_key(
        &db,
        admin,
        session,
        new_id,
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
        None,
        Utc::now(),
    )
    .await
    .expect("rotate");
    audited.into_inner();

    let active = repos::signing_keys::active(&db).await.expect("active key");
    assert_eq!(active.id, new_id);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("signing_key.rotate")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("K01");
}

// ── Stage 2 items 2-3: injected-failure rollback proofs ──────────
//
// The acceptance bar (per the Stage 2 direction, §3): an injected
// failure must roll back *both* the mutation and the audit row,
// verified by observing state before and after -- not by asserting
// that the call returned `Err`. All three tests below do exactly
// that: capture "before" state, inject, assert `Err`, then assert
// "after" state is identical to "before" -- for both the domain
// table and the audit log.

#[tokio::test]
async fn k01_injected_failure_before_append_rolls_back_the_mutation() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let before_active = repos::signing_keys::active(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = rotate_signing_key(
        &db,
        admin,
        session,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
        None,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The mutation (key insert/activation) never happened: same
    // "no active key" state as before, not a half-activated new key.
    assert_eq!(
        before_active.err().map(|e| e.to_string()),
        repos::signing_keys::active(&db)
            .await
            .err()
            .map(|e| e.to_string())
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("K01");
}

#[tokio::test]
async fn k01_injected_failure_after_append_rolls_back_the_mutation_and_the_append() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_after_next_append();
    let result = rotate_signing_key(
        &db,
        admin,
        session,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
        None,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // append_within_tx really ran (it's before this injection
    // point) and really inserted a row -- proving this rolls back
    // requires that the row is gone, not merely that this test
    // never looked for it.
    assert!(
        repos::signing_keys::active(&db).await.is_err(),
        "no active key: the mutation rolled back with the audit row"
    );
    assert_eq!(
        latest_audit_action(&db).await,
        before_audit,
        "the audit row append_within_tx just wrote must not survive \
         an uncommitted transaction"
    );
}

#[tokio::test]
async fn k01_injected_commit_failure_rolls_back_the_mutation_and_the_append() {
    // Distinct from the two tests above: the closure itself returns
    // `Ok` (mutation succeeded, event built, append succeeded) --
    // this proves that a rejection at SQLite's own commit boundary,
    // which no domain code observes or controls, still prevents
    // `Audited<T>` from ever being constructed and still leaves no
    // trace in either table.
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fail_next_commit_for_test();
    let result = rotate_signing_key(
        &db,
        admin,
        session,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
        None,
        Utc::now(),
    )
    .await;
    assert!(
        result.is_err(),
        "a rejected commit must surface as Err, not a silently-empty Ok"
    );

    assert!(
        repos::signing_keys::active(&db).await.is_err(),
        "no active key: a rejected commit persists nothing"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // The hook self-disarmed after firing once: a real rotation now
    // succeeds normally, proving this test didn't leave the
    // connection unable to commit anything ever again.
    let audited = rotate_signing_key(
        &db,
        admin,
        session,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
        None,
        Utc::now(),
    )
    .await
    .expect("rotate after the injected commit failure has cleared");
    audited.into_inner();
}

// ── K03 — delete a retired signing key (RFC 094 M2b wave 4 stage 3) ──────

#[tokio::test]
async fn a_k03_signing_delete() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let id = SigningKeyId::new();
    repos::signing_keys::insert_with_plaintext(&db, id, "ed25519", b"retired-key", b"pub", false)
        .await
        .expect("seed retired key");
    let before = audit_rows(&db).await;

    let audited = delete_signing_key(&db, admin, id, Some("cleanup".into()))
        .await
        .expect("delete");
    audited.into_inner();

    assert!(
        matches!(
            repos::signing_keys::list_published(&db).await,
            Ok(rows) if !rows.iter().any(|r| r.id == id)
        ),
        "the key row must be gone"
    );
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("signing_key.delete")
    );
    let note = last_event(&db).await.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "reason").as_deref(),
        Some("cleanup")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("K03");
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
async fn k03_injected_failure_before_append_rolls_back_the_delete() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let id = SigningKeyId::new();
    repos::signing_keys::insert_with_plaintext(&db, id, "ed25519", b"retired-key", b"pub", false)
        .await
        .expect("seed retired key");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = delete_signing_key(&db, admin, id, None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        matches!(
            repos::signing_keys::list_published(&db).await,
            Ok(rows) if rows.iter().any(|r| r.id == id)
        ),
        "the delete must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("K03");
}

#[tokio::test]
async fn k03_against_a_nonexistent_key_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let ghost = SigningKeyId::new();
    let before = audit_rows(&db).await;

    let result = delete_signing_key(&db, admin, ghost, None).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(
        audit_rows(&db).await,
        before,
        "a delete of a nonexistent key must not write an audit record"
    );
}

#[tokio::test]
async fn k03_against_the_active_key_is_conflict_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let id = SigningKeyId::new();
    repos::signing_keys::insert_with_plaintext(&db, id, "ed25519", b"active-key", b"pub", true)
        .await
        .expect("seed active key");
    let before = audit_rows(&db).await;

    let result = delete_signing_key(&db, admin, id, None).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "expected Conflict for the active key"
    );
    assert!(
        matches!(
            repos::signing_keys::list_published(&db).await,
            Ok(rows) if rows.iter().any(|r| r.id == id)
        ),
        "the active key must still exist"
    );
    assert_eq!(
        audit_rows(&db).await,
        before,
        "a refused delete of the active key must not write an audit record"
    );
}
