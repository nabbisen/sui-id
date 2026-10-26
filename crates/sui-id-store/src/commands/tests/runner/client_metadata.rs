//! RFC 094 M2b wave 4 stage 3 — C01 (create client), C02 (update basic
//! info), C07 (rotate secret), C08 (dev-mode secret patch). All one-write
//! commands; each sealed command now carries its own audit event in the
//! same transaction as its write.

use super::*;

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

// ── C01 — create client ──────────────────────────────────────────────────

#[tokio::test]
async fn a_c01_create() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let row = a_client();
    let client_id = row.id;
    let before = audit_rows(&db).await;

    let audited = crate::commands::create_client(&db, admin, row)
        .await
        .expect("create client");
    audited.into_inner();

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(stored.id, client_id);

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.create");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C01");
}

#[tokio::test]
async fn c01_injected_failure_before_append_rolls_back_the_create() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let row = a_client();
    let client_id = row.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::create_client(&db, admin, row).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        matches!(
            repos::clients::get(&db, client_id).await,
            Err(StoreError::NotFound)
        ),
        "the create must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C01");
}

// ── C02 — update client basic info ───────────────────────────────────────

#[tokio::test]
async fn a_c02_basic() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let row = a_client();
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_client_basic(
        &db,
        admin,
        client_id,
        "renamed-rp".into(),
        vec!["https://rp.test/new-cb".into()],
        Utc::now(),
    )
    .await
    .expect("update basic");
    audited.into_inner();

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(stored.name, "renamed-rp");
    assert_eq!(
        stored.redirect_uris,
        vec!["https://rp.test/new-cb".to_string()]
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.update");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C02");
}

#[tokio::test]
async fn c02_injected_failure_before_append_rolls_back_the_update() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let row = a_client();
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::update_client_basic(
        &db,
        admin,
        client_id,
        "renamed-rp".into(),
        vec!["https://rp.test/new-cb".into()],
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        stored.name, row.name,
        "the rename must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C02");
}

#[tokio::test]
async fn c02_against_a_nonexistent_client_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let ghost = ClientId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::update_client_basic(
        &db,
        admin,
        ghost,
        "x".into(),
        vec!["https://rp.test/cb".into()],
        Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}

// ── C07 — rotate client secret ───────────────────────────────────────────

#[tokio::test]
async fn a_c07_secret() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let mut row = a_client();
    row.confidential = true;
    row.secret_hash = Some("old-hash".into());
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before = audit_rows(&db).await;

    let audited = crate::commands::rotate_client_secret(
        &db,
        admin,
        client_id,
        "new-hash".into(),
        Some("scheduled".into()),
        Utc::now(),
    )
    .await
    .expect("rotate secret");
    audited.into_inner();

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(stored.secret_hash.as_deref(), Some("new-hash"));

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.rotate_secret");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "reason").as_deref(),
        Some("scheduled")
    );
    assert!(
        !note.contains("new-hash") && !note.contains("old-hash"),
        "the note must never carry the secret hash"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C07");
}

#[tokio::test]
async fn c07_injected_failure_before_append_rolls_back_the_rotation() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let mut row = a_client();
    row.confidential = true;
    row.secret_hash = Some("old-hash".into());
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::rotate_client_secret(
        &db,
        admin,
        client_id,
        "new-hash".into(),
        None,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        stored.secret_hash.as_deref(),
        Some("old-hash"),
        "the rotation must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C07");
}

#[tokio::test]
async fn c07_against_a_nonexistent_client_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let ghost = ClientId::new();
    let before = audit_rows(&db).await;

    let result =
        crate::commands::rotate_client_secret(&db, admin, ghost, "h".into(), None, Utc::now())
            .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}

// ── C08 — patch a dev-mode client's secret hash ──────────────────────────

#[tokio::test]
async fn a_c08_dev_secret() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let mut row = a_client();
    row.confidential = true;
    row.secret_hash = Some("generated-hash".into());
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before = audit_rows(&db).await;

    let audited = crate::commands::set_dev_client_secret(&db, admin, client_id, "dev-hash".into())
        .await
        .expect("set dev secret");
    audited.into_inner();

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(stored.secret_hash.as_deref(), Some("dev-hash"));

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.dev_secret_set");
    assert_eq!(event.note, None, "no attributes, in particular no hash");
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C08");
}

#[tokio::test]
async fn c08_injected_failure_before_append_rolls_back_the_patch() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let mut row = a_client();
    row.confidential = true;
    row.secret_hash = Some("original-hash".into());
    let client_id = row.id;
    repos::clients::create(&db, &row)
        .await
        .expect("seed client");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result =
        crate::commands::set_dev_client_secret(&db, admin, client_id, "dev-hash".into()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        stored.secret_hash.as_deref(),
        Some("original-hash"),
        "the patch must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C08");
}

#[tokio::test]
async fn c08_against_a_nonexistent_client_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let ghost = ClientId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::set_dev_client_secret(&db, admin, ghost, "h".into()).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}
