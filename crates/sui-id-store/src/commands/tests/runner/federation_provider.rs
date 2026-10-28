//! RFC 094 M2b wave 3 — federation-provider configuration writes, C16
//! and C24. Both are startup provisioning, not an administrator action:
//! no authenticated actor, `system_principal: permitted`, the same shape
//! C22 uses. 2026-10-25 ruling: RFC 096-B2's activation-generation and
//! preflight semantics are not implemented here — this seals `create`
//! and the allowed-origins write as they stand today.

use super::*;
use sui_id_shared::ids::FederationProviderId;

fn sample_row(slug: &str) -> crate::models::FederationProviderRow {
    let now = Utc::now();
    crate::models::FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Google".into(),
        issuer: format!("https://{slug}.example"),
        client_id: "client-abc".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: crate::models::ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: "https://a.example https://b.example".into(),
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

// ── C16 — provider create ─────────────────────────────────────────────

#[tokio::test]
async fn a_c16_provider_create() {
    let db = fresh_db();
    let row = sample_row("c16-happy");
    let before = audit_rows(&db).await;

    let audited =
        crate::commands::create_federation_provider(&db, row.clone(), Some("s3cr3t".into()))
            .await
            .expect("create");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(stored.slug, "c16-happy");
    assert!(
        stored.client_secret_enc.is_some(),
        "the secret must be sealed into the row"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.created");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    assert_eq!(event.actor, None, "no session at startup");
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "slug").as_deref(),
        Some("c16-happy")
    );
    assert_eq!(
        crate::registry::note_field(&note, "issuer").as_deref(),
        Some(row.issuer.as_str())
    );
    assert!(
        !note.contains("s3cr3t"),
        "the plaintext secret must never reach the note:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C16");
}

#[tokio::test]
async fn c16_injected_failure_before_append_rolls_back_the_create() {
    let db = fresh_db();
    let row = sample_row("c16-rollback");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::create_federation_provider(&db, row.clone(), None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let fetched = repos::federation_provider::get_by_slug(&db, "c16-rollback").await;
    assert!(
        matches!(fetched, Err(StoreError::NotFound)),
        "the create must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C16");
}

// ── C24 — allowed-origins change ──────────────────────────────────────

#[tokio::test]
async fn a_c24_origins_change() {
    let db = fresh_db();
    let row = sample_row("c24-happy");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let origins_before = row.allowed_origins.split_whitespace().count();
    assert_eq!(origins_before, 2);
    let before = audit_rows(&db).await;

    let audited = crate::commands::update_federation_provider_allowed_origins(
        &db,
        row.id,
        "https://a.example https://b.example https://c.example".into(),
        origins_before,
        Utc::now(),
    )
    .await
    .expect("update");
    audited.into_inner();

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(
        stored.allowed_origins,
        "https://a.example https://b.example https://c.example"
    );

    let event = last_event(&db).await;
    assert_eq!(event.action, "federation.provider.allowed_origins_changed");
    assert_eq!(event.target.as_deref(), Some(row.id.to_string().as_str()));
    let note = event.note.expect("note");
    assert_eq!(
        crate::registry::note_field(&note, "origins_before").as_deref(),
        Some("2")
    );
    assert_eq!(
        crate::registry::note_field(&note, "origins_after").as_deref(),
        Some("3")
    );
    assert!(
        !note.contains("a.example") && !note.contains("b.example") && !note.contains("c.example"),
        "only the counts, never the origin lists:\n{note}"
    );
    assert_eq!(audit_rows(&db).await, before + 1, "exactly one event");
    record_exactly_once_coverage("C24");
}

#[tokio::test]
async fn c24_injected_failure_before_append_rolls_back_the_change() {
    let db = fresh_db();
    let row = sample_row("c24-rollback");
    repos::federation_provider::create(&db, &row, None)
        .await
        .expect("seed provider");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = crate::commands::update_federation_provider_allowed_origins(
        &db,
        row.id,
        "https://changed.example".into(),
        2,
        Utc::now(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let stored = repos::federation_provider::get(&db, row.id)
        .await
        .expect("get");
    assert_eq!(
        stored.allowed_origins, row.allowed_origins,
        "the change must not have survived rollback"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
    record_rollback_coverage("C24");
}

#[tokio::test]
async fn c24_against_a_nonexistent_provider_is_notfound_and_writes_no_record() {
    let db = fresh_db();
    let ghost = FederationProviderId::new();
    let before = audit_rows(&db).await;

    let result = crate::commands::update_federation_provider_allowed_origins(
        &db,
        ghost,
        "https://x.example".into(),
        0,
        Utc::now(),
    )
    .await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "expected NotFound"
    );
    assert_eq!(audit_rows(&db).await, before);
}
