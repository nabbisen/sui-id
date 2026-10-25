//! RFC 094 M2b wave 4 stage 1 — C05 (disable client), C21 (enable client),
//! C06 (soft-delete client). C05 and C06 each do two writes in one
//! transaction: the client-row mutation and
//! `refresh_tokens::revoke_all_for_client_within_tx`; C21 does one write
//! only and must not revoke anything.

use super::*;

async fn seed_client_with_tokens(db: &Database, n: usize, initially_disabled: bool) -> ClientId {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    let mut client = a_client();
    client.is_disabled = initially_disabled;
    repos::clients::create(db, &client)
        .await
        .expect("create client");
    for _ in 0..n {
        let id = sui_id_shared::RefreshTokenId::generate();
        let row = crate::models::RefreshTokenRow {
            id: id.clone(),
            user_id: user.id,
            client_id: client.id,
            scope: "openid".into(),
            expires_at: Utc::now() + TimeDelta::hours(1),
            revoked_at: None,
            created_at: Utc::now(),
            auth_methods: vec![],
            family_id: sui_id_shared::FamilyId::root_of(&id),
        };
        let token =
            sui_id_shared::RawRefreshToken::from_untrusted(uuid::Uuid::new_v4().to_string());
        repos::refresh_tokens::insert(db, &row, &token)
            .await
            .expect("seed refresh token");
    }
    client.id
}

async fn active_token_count(db: &Database, client_id: ClientId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM refresh_tokens WHERE client_id = ?1 AND revoked_at IS NULL",
            [client_id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

async fn last_event(db: &Database) -> crate::models::AuditLogRow {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
}

// ── C05 — disable client ─────────────────────────────────────────────────

#[tokio::test]
async fn a_c05_disable() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 2, false).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::disable_client(
        &db,
        admin,
        client_id,
        Some("spam reports".into()),
        Utc::now(),
    )
    .await
    .expect("disable client");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(client.is_disabled, "client must be disabled");
    assert_eq!(
        active_token_count(&db, client_id).await,
        0,
        "all refresh tokens must be revoked"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.disable");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "refresh_tokens_revoked").as_deref(),
        Some("2")
    );
    assert_eq!(
        crate::registry::note_field(&note, "reason").as_deref(),
        Some("spam reports")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C05");
}

#[tokio::test]
async fn c05_injected_failure_before_append_rolls_back_the_disable_and_the_revoke() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 2, false).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::disable_client(&db, admin, client_id, None, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(
        !client.is_disabled,
        "the disable must not have survived rollback"
    );
    assert_eq!(
        active_token_count(&db, client_id).await,
        2,
        "the revoke must not have survived rollback either -- this is the defect \
         this command closes: the client row and its tokens must move together"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C05");
}

// ── C21 — enable client ──────────────────────────────────────────────────

#[tokio::test]
async fn a_c21_enable() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 2, true).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::enable_client(&db, admin, client_id, Utc::now())
        .await
        .expect("enable client");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(!client.is_disabled, "client must be enabled");
    assert_eq!(
        active_token_count(&db, client_id).await,
        2,
        "enabling must revoke nothing"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.enable");
    assert_eq!(
        event.note, None,
        "enable carries no attributes -- in particular no refresh_tokens_revoked"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C21");
}

#[tokio::test]
async fn c21_injected_failure_before_append_rolls_back_the_enable() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 1, true).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::enable_client(&db, admin, client_id, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(
        client.is_disabled,
        "the enable must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C21");
}

// ── C06 — soft-delete client ─────────────────────────────────────────────

#[tokio::test]
async fn a_c06_delete() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 3, false).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::delete_client(
        &db,
        admin,
        client_id,
        Some("owner requested".into()),
        Utc::now(),
    )
    .await
    .expect("delete client");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(client.is_deleted, "client must be soft-deleted");
    assert!(
        client.is_disabled,
        "a deleted client must also read as disabled"
    );
    assert_eq!(
        active_token_count(&db, client_id).await,
        0,
        "all refresh tokens must be revoked"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.delete");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "refresh_tokens_revoked").as_deref(),
        Some("3")
    );
    assert_eq!(
        crate::registry::note_field(&note, "reason").as_deref(),
        Some("owner requested")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C06");
}

#[tokio::test]
async fn c06_injected_failure_before_append_rolls_back_the_delete_and_the_revoke() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_tokens(&db, 2, false).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::delete_client(&db, admin, client_id, None, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert!(
        !client.is_deleted,
        "the delete must not have survived rollback"
    );
    assert_eq!(
        active_token_count(&db, client_id).await,
        2,
        "the revoke must not have survived rollback either -- this is the defect \
         this command closes: the client row and its tokens must move together"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C06");
}
