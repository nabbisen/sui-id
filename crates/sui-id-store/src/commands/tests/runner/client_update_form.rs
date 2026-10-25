//! RFC 094 M2b wave 4 stage 2 — C03 (the client-update form): allowed
//! scopes, post-logout redirect URIs and consent policy commit together
//! in one transaction, as one audit record. C04 and C09 are this
//! command's sibling inventory rows (see `commands.rs`'s module-level
//! comment on `C03`), not separate commands -- these tests use the
//! three rows' pre-existing `test_id` names, one test per field, all
//! exercising the same single sealed command.

use super::*;

async fn seed_client(db: &Database) -> ClientId {
    let client = a_client();
    repos::clients::create(db, &client)
        .await
        .expect("create client");
    client.id
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
async fn a_c03_scopes() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_client_form(
        &db,
        admin,
        client_id,
        "openid profile email".into(),
        vec!["https://rp.test/bye".into()],
        crate::models::ConsentPolicy::Always,
        Utc::now(),
    )
    .await
    .expect("update client form");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(client.allowed_scopes, "openid profile email");

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.update_form");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "allowed_scopes_count").as_deref(),
        Some("3")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C03");
}

#[tokio::test]
async fn a_c04_logout_uris() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_client_form(
        &db,
        admin,
        client_id,
        "openid".into(),
        vec![
            "https://rp.test/bye".into(),
            "https://rp.test/also-bye".into(),
        ],
        crate::models::ConsentPolicy::None,
        Utc::now(),
    )
    .await
    .expect("update client form");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        client.post_logout_redirect_uris,
        vec![
            "https://rp.test/bye".to_string(),
            "https://rp.test/also-bye".to_string()
        ]
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.update_form");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "post_logout_redirect_uris_count").as_deref(),
        Some("2")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
}

#[tokio::test]
async fn a_c09_consent_policy() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client(&db).await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_client_form(
        &db,
        admin,
        client_id,
        "openid".into(),
        vec![],
        crate::models::ConsentPolicy::FirstTime,
        Utc::now(),
    )
    .await
    .expect("update client form");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        client.consent_policy,
        crate::models::ConsentPolicy::FirstTime
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.update_form");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "consent_policy").as_deref(),
        Some("first_time")
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
}

#[tokio::test]
async fn c03_injected_failure_before_append_rolls_back_all_three_writes() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client(&db).await;
    // Seed known starting values for all three fields, distinct from
    // what the attempted update below asks for, so "unchanged" is
    // unambiguous.
    crate::repos::clients::set_allowed_scopes(&db, client_id, "openid")
        .await
        .expect("seed scopes");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::update_client_form(
        &db,
        admin,
        client_id,
        "openid profile email offline_access".into(),
        vec!["https://rp.test/new-bye".into()],
        crate::models::ConsentPolicy::Always,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        client.allowed_scopes, "openid",
        "the scopes write (first) must not have survived rollback"
    );
    assert!(
        client.post_logout_redirect_uris.is_empty(),
        "the post-logout URI write (second) must not have survived rollback"
    );
    assert_eq!(
        client.consent_policy,
        crate::models::ConsentPolicy::None,
        "the consent-policy write (third) must not have survived rollback -- \
         this is the defect this command closes: a failure must not leave \
         some fields changed and others not"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C03");
}
