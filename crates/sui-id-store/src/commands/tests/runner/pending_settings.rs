//! RFC 094 M2b stage 1 — S07 (create) and S09 (cancel), the two sealed
//! `pending_settings_change` commands. Both share the lean pattern most
//! M2a conversions use (not K01's own extra-thorough triplet, which was
//! specifically for the foundation command): one exactly-once test, one
//! injected-failure rollback test, per command.

use super::*;
use chrono::Duration;
use sui_id_shared::ids::PendingChangeId;

fn sample_row(
    admin: UserId,
    session: sui_id_shared::ids::SessionId,
) -> crate::repos::pending_settings_change::PendingSettingsChangeRow {
    crate::repos::pending_settings_change::PendingSettingsChangeRow {
        id: PendingChangeId::new(),
        session_id: session,
        actor_id: admin,
        intent: "smtp_password_update".into(),
        payload_enc: vec![0xDE, 0xAD, 0xBE, 0xEF],
        summary: "SMTP password: will be updated".into(),
        csrf_token: "test-csrf-token".into(),
        expires_at: Utc::now() + Duration::seconds(300),
        created_at: Utc::now(),
    }
}

// ── S07 — create ──────────────────────────────────────────────────────────

#[tokio::test]
async fn s07_create_inserts_the_row_and_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    let before = audit_rows(&db).await;

    let audited = crate::commands::create_pending_settings_change(&db, admin, row)
        .await
        .expect("create");
    audited.into_inner();

    let summary = crate::repos::pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect("row exists");
    assert_eq!(summary, "SMTP password: will be updated");

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.pending_change.created")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S07");
}

#[tokio::test]
async fn s07_injected_failure_before_append_rolls_back_the_insert() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::create_pending_settings_change(&db, admin, row).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The row must not exist: not inserted, not left half-committed.
    assert!(
        matches!(
            crate::repos::pending_settings_change::get_summary(&db, id, Utc::now()).await,
            Err(StoreError::NotFound)
        ),
        "the pending row must not have been inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S07");
}

// ── S09 — cancel ──────────────────────────────────────────────────────────

#[tokio::test]
async fn s09_cancel_deletes_the_row_and_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let before = audit_rows(&db).await;

    let audited = crate::commands::cancel_pending_settings_change(&db, admin, id)
        .await
        .expect("cancel");
    audited.into_inner();

    assert!(
        matches!(
            crate::repos::pending_settings_change::get_summary(&db, id, Utc::now()).await,
            Err(StoreError::NotFound)
        ),
        "the row must be gone"
    );
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.pending_change.cancelled")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S09");
}

#[tokio::test]
async fn s09_cancel_is_idempotent_on_an_already_absent_row() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let id = PendingChangeId::new(); // never inserted
    let before = audit_rows(&db).await;

    let audited = crate::commands::cancel_pending_settings_change(&db, admin, id)
        .await
        .expect("cancel on an absent row is not an error");
    audited.into_inner();

    // Still emits the event, matching the pre-stage `cancel`'s own
    // idempotent shape (dispatch §4: no behaviour change for a caller
    // that succeeds).
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.pending_change.cancelled")
    );
    assert_eq!(audit_rows(&db).await, before + 1);
}

#[tokio::test]
async fn s09_injected_failure_before_append_rolls_back_the_cancel() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::cancel_pending_settings_change(&db, admin, id).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The row must still exist: the delete did not survive the rollback.
    let summary = crate::repos::pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect("the row must not have been deleted");
    assert_eq!(summary, "SMTP password: will be updated");
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S09");
}
