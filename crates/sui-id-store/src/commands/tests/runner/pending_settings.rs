//! RFC 094 M2b stages 1 and 2 — S07 (create), S09 (cancel), S08 (apply),
//! the three sealed `pending_settings_change` commands. S07/S09 share
//! the lean pattern most M2a conversions use (not K01's own
//! extra-thorough triplet, which was specifically for the foundation
//! command): one exactly-once test, one injected-failure rollback test,
//! per command. S08 adds a third branch per command's own shape
//! (`BindingFailed`, Decision B) with its own two tests, since neither
//! the exactly-once nor the rollback test exercises it: both describe
//! the `Applied` branch's two properties, not the denial branch's own.
//! The third kind of evidence the stage 2 dispatch asks for --
//! "a validation failure leaves the row intact" -- is not here: core's
//! `pending_change::apply` rejects an unknown `tls_mode` *before* this
//! command is ever called, so nothing at this layer can exercise it; see
//! `sui-id-core/src/settings/pending_change/tests.rs`.

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

/// A minimal, valid `SmtpConfigRow` candidate -- the shape S08's caller
/// (core) would have already validated and sealed before calling in.
fn sample_smtp_config(now: chrono::DateTime<Utc>) -> crate::models::SmtpConfigRow {
    crate::models::SmtpConfigRow {
        enabled: true,
        host: "smtp.example.com".into(),
        port: 587,
        tls_mode: crate::models::SmtpTlsMode::StartTls,
        username: Some("relay-user".into()),
        password_enc: Some(vec![0xC0, 0xFF, 0xEE]),
        from_address: "noreply@example.com".into(),
        from_name: None,
        base_url: "https://sui-id.example.com".into(),
        created_at: now,
        updated_at: now,
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

// ── S08 — apply ───────────────────────────────────────────────────────────

#[tokio::test]
async fn s08_apply_writes_smtp_config_and_appends_exactly_one_audit_row() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let now = Utc::now();
    let new_config = sample_smtp_config(now);
    let before = audit_rows(&db).await;

    let audited = crate::commands::apply_pending_settings_change(
        &db,
        admin,
        id,
        session,
        row.csrf_token.clone(),
        now,
        new_config.clone(),
    )
    .await
    .expect("apply");
    assert!(matches!(
        audited.into_inner(),
        crate::commands::ApplyPendingSettingsChangeOutcome::Applied
    ));

    // The pending row is gone.
    assert!(matches!(
        crate::repos::pending_settings_change::peek(&db, id, Utc::now()).await,
        Err(StoreError::NotFound)
    ));
    // smtp_config holds the new values.
    let stored = crate::repos::smtp_config::get(&db)
        .await
        .expect("get smtp_config")
        .expect("smtp_config row exists");
    assert_eq!(stored.host, new_config.host);
    assert_eq!(stored.port, new_config.port);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("settings.pending_change.applied")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("S08");
}

#[tokio::test]
async fn s08_injected_failure_before_append_rolls_back_the_delete_and_the_smtp_write() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let now = Utc::now();
    let new_config = sample_smtp_config(now);
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::apply_pending_settings_change(
        &db,
        admin,
        id,
        session,
        row.csrf_token.clone(),
        now,
        new_config,
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The pending row must still exist: not consumed.
    let summary = crate::repos::pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect("the row must not have been deleted");
    assert_eq!(summary, "SMTP password: will be updated");
    // smtp_config must be unchanged (never configured, in this test).
    assert!(
        crate::repos::smtp_config::get(&db)
            .await
            .expect("get smtp_config")
            .is_none(),
        "smtp_config must not have been written"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("S08");
}

#[tokio::test]
async fn s08_binding_failure_leaves_the_row_and_smtp_config_intact_and_records_a_denial() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let now = Utc::now();
    let new_config = sample_smtp_config(now);
    let before = audit_rows(&db).await;

    // Wrong CSRF token -- a binding mismatch, not an absent/expired row.
    let audited = crate::commands::apply_pending_settings_change(
        &db,
        admin,
        id,
        session,
        "the-wrong-csrf-token".to_string(),
        now,
        new_config,
    )
    .await
    .expect("a binding failure is Ok, not Err");
    assert!(matches!(
        audited.into_inner(),
        crate::commands::ApplyPendingSettingsChangeOutcome::BindingFailed
    ));

    // The pending row must still exist.
    let summary = crate::repos::pending_settings_change::get_summary(&db, id, Utc::now())
        .await
        .expect("the row must not have been deleted on a binding failure");
    assert_eq!(summary, "SMTP password: will be updated");
    // smtp_config must be unchanged.
    assert!(
        crate::repos::smtp_config::get(&db)
            .await
            .expect("get smtp_config")
            .is_none(),
        "smtp_config must not have been written on a binding failure"
    );

    // Exactly one audit row, recording the denial.
    let latest = crate::repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("one audit row");
    assert_eq!(latest.action, "settings.pending_change.binding_failed");
    assert_eq!(latest.result, "failure");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
}

/// S08 is single-use, and the re-read inside `apply_within_tx` is what
/// makes it so. `peek` has already read the row by the time the command
/// runs, so the obvious future edit is to pass that row in and skip the
/// second read -- at which point a replayed apply writes `smtp_config`
/// again and deletes nothing, because the delete no longer depends on
/// the row still being there.
///
/// `consume` has `consume_is_single_use`; `apply`, which replaces it on
/// this path, had no equivalent. Added by the architect during review:
/// measured 2026-10-24, the three mutations the round ran all leave this
/// property unguarded.
#[tokio::test]
async fn s08_apply_is_single_use() {
    let db = fresh_db();
    let (admin, session) = an_admin_session(&db).await;
    let row = sample_row(admin, session);
    let id = row.id;
    crate::repos::pending_settings_change::insert(&db, &row)
        .await
        .expect("seed row");
    let now = Utc::now();

    crate::commands::apply_pending_settings_change(
        &db,
        admin,
        id,
        session,
        row.csrf_token.clone(),
        now,
        sample_smtp_config(now),
    )
    .await
    .expect("first apply succeeds");

    let after_first = audit_rows(&db).await;

    // The same pending id, replayed with the same binding.
    let second = crate::commands::apply_pending_settings_change(
        &db,
        admin,
        id,
        session,
        row.csrf_token.clone(),
        Utc::now(),
        sample_smtp_config(now),
    )
    .await;

    assert!(
        matches!(second, Err(StoreError::NotFound)),
        "a replayed apply must be NotFound, not a second application"
    );
    assert_eq!(
        audit_rows(&db).await,
        after_first,
        "a replayed apply must write no audit row at all"
    );
}
