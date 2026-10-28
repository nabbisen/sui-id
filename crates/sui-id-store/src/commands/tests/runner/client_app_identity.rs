//! RFC 140 stage 1 — C10: a client's application-identity links
//! (`homepage_uri`, `privacy_policy_uri`, `tos_uri`) commit atomically
//! with their audit record. `logo_uri` is read fresh inside the same
//! transaction and passed through unchanged — stage 1 does not manage
//! it, RFC 140 §5 still undecided.

use super::*;

async fn seed_client_with_logo(db: &Database, logo_uri: &str) -> ClientId {
    let mut client = a_client();
    client.logo_uri = Some(logo_uri.into());
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
async fn a_c10_app_identity_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_logo(&db, "https://logo.example/l.png").await;
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_client_app_identity(
        &db,
        admin,
        client_id,
        Some("https://home.example".into()),
        Some("https://privacy.example".into()),
        Some("https://tos.example".into()),
        Utc::now(),
    )
    .await
    .expect("update app identity");
    audited.into_inner();

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(client.homepage_uri.as_deref(), Some("https://home.example"));
    assert_eq!(
        client.privacy_policy_uri.as_deref(),
        Some("https://privacy.example")
    );
    assert_eq!(client.tos_uri.as_deref(), Some("https://tos.example"));
    assert_eq!(
        client.logo_uri.as_deref(),
        Some("https://logo.example/l.png"),
        "logo_uri must survive unchanged -- stage 1 does not manage it"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "client.app_identity_changed");
    assert_eq!(
        event.target.as_deref(),
        Some(client_id.to_string().as_str())
    );
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "homepage_uri_changed").as_deref(),
        Some("true")
    );
    assert_eq!(
        crate::registry::note_field(&note, "privacy_policy_uri_changed").as_deref(),
        Some("true")
    );
    assert_eq!(
        crate::registry::note_field(&note, "tos_uri_changed").as_deref(),
        Some("true")
    );
    assert!(
        !note.contains("home.example")
            && !note.contains("privacy.example")
            && !note.contains("tos.example"),
        "only booleans, never the URLs themselves:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C10");
}

/// A field left unchanged (same value resubmitted) must report `false`,
/// not `true` -- the attribute is a comparison against the row's own
/// prior state, read fresh inside this transaction, not "was this
/// field present in the request."
#[tokio::test]
async fn a_c10_app_identity_change_reports_false_for_an_unchanged_field() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_logo(&db, "https://logo.example/l.png").await;

    crate::commands::update_client_app_identity(
        &db,
        admin,
        client_id,
        Some("https://home.example".into()),
        None,
        None,
        Utc::now(),
    )
    .await
    .expect("first update");

    let audited = crate::commands::update_client_app_identity(
        &db,
        admin,
        client_id,
        Some("https://home.example".into()), // same value resubmitted
        Some("https://privacy.example".into()), // genuinely new
        None,
        Utc::now(),
    )
    .await
    .expect("second update");
    audited.into_inner();

    let note = last_event(&db).await.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "homepage_uri_changed").as_deref(),
        Some("false"),
        "resubmitting the same value must not report a change"
    );
    assert_eq!(
        crate::registry::note_field(&note, "privacy_policy_uri_changed").as_deref(),
        Some("true")
    );
}

#[tokio::test]
async fn c10_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let client_id = seed_client_with_logo(&db, "https://logo.example/l.png").await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::update_client_app_identity(
        &db,
        admin,
        client_id,
        Some("https://home.example".into()),
        None,
        None,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let client = repos::clients::get(&db, client_id)
        .await
        .expect("get client");
    assert_eq!(
        client.homepage_uri, None,
        "the change must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C10");
}

#[tokio::test]
async fn c10_against_a_nonexistent_client_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let (admin, _session) = an_admin_session(&db).await;
    let ghost = ClientId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::update_client_app_identity(
        &db,
        admin,
        ghost,
        Some("https://home.example".into()),
        None,
        None,
        Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}
