//! RFC 095 M3 stage 6 — the legacy-candidate report's read-only query.
//! `legacy_dynamic_candidates` is the only admissible evidence path: an
//! `audit_log` row naming `client.dynamic_register`, joined to the
//! client's *current* row by the audit row's own `target`. Each test
//! states one branch of that join rather than the report's formatting.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::{LegacyDynamicCandidateReport, legacy_dynamic_candidates};
use crate::models::{AuditLogRow, ClientRow, ConsentPolicy, RegistrationSource};
use crate::repos::clients;
use crate::{Database, crypto::MasterKey};
use chrono::Utc;
use sui_id_shared::ids::ClientId;

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

fn sample_client(id: ClientId, registered_via: RegistrationSource) -> ClientRow {
    let now = Utc::now();
    ClientRow {
        id,
        name: "test-client".into(),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: ConsentPolicy::default(),
        registered_via,
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        created_at: now,
        updated_at: now,
    }
}

/// `clients::create_within_tx` leaves `registered_via` at the table's own
/// `DEFAULT 'admin'` (RFC 094 C11) — the real C15 insert path
/// (`commands.rs:4102-4103`) compensates with a same-transaction
/// `set_registered_via_within_tx` call right after it. A fixture that
/// skips that follow-up silently gets `admin` regardless of what
/// `sample_client` was asked for — this seeds a client exactly the way
/// production does, so the seeded value is the one actually read back,
/// not merely the one requested.
async fn seed_client(db: &Database, id: ClientId, registered_via: RegistrationSource) {
    let row = sample_client(id, registered_via.clone());
    let now = row.created_at;
    clients::create(db, &row).await.expect("seed client");
    clients::set_registered_via(db, id, registered_via, now)
        .await
        .expect("stamp registered_via");
}

fn dynamic_register_audit_row(target: Option<String>) -> AuditLogRow {
    AuditLogRow {
        at: Utc::now(),
        actor: None,
        action: "client.dynamic_register".into(),
        target,
        result: "ok".into(),
        note: Some("name=test-client".into()),
    }
}

/// An audit row naming a client whose row still says `admin` is a
/// candidate (RFC 095 M3 stage 6 §4.1).
#[tokio::test]
async fn a_candidate_is_found() {
    let db = fresh_db();
    let id = ClientId::new();
    seed_client(&db, id, RegistrationSource::Admin).await;
    super::append(&db, &dynamic_register_audit_row(Some(id.to_string())))
        .await
        .expect("seed audit row");

    let report = legacy_dynamic_candidates(&db).await.expect("query");
    assert_eq!(report.candidates.len(), 1, "the candidate must be found");
    assert_eq!(report.candidates[0].client_id, id);
    assert!(report.unresolved.is_empty());
}

/// Same shape as the candidate, but the client row already says
/// `dynamic` — correctly stamped, not a candidate. Without this, the
/// candidate test above would pass if the query returned every audited
/// client regardless of its current `registered_via`.
#[tokio::test]
async fn a_stamped_client_is_not_a_candidate() {
    let db = fresh_db();
    let id = ClientId::new();
    seed_client(&db, id, RegistrationSource::Dynamic).await;
    super::append(&db, &dynamic_register_audit_row(Some(id.to_string())))
        .await
        .expect("seed audit row");

    let report = legacy_dynamic_candidates(&db).await.expect("query");
    assert!(
        report.candidates.is_empty(),
        "a client already stamped `dynamic` must not be reported"
    );
    assert!(
        report.unresolved.is_empty(),
        "a correctly stamped client is not a gap either"
    );
}

/// An admin-registered client with no `client.dynamic_register` audit
/// row at all must not be reported, no matter how it looks — the
/// no-inference rule (RFC 095 `:350-360`), made executable: nothing
/// about this client's shape, timestamps, or missing application URIs
/// may stand in for the one piece of admissible evidence.
#[tokio::test]
async fn an_admin_client_with_no_audit_row_is_not_a_candidate() {
    let db = fresh_db();
    let id = ClientId::new();
    seed_client(&db, id, RegistrationSource::Admin).await;
    // Deliberately no audit::append call for this client.

    let report = legacy_dynamic_candidates(&db).await.expect("query");
    assert!(
        report.candidates.is_empty(),
        "an admin client with no audit evidence must not be inferred as a candidate"
    );
    assert!(report.unresolved.is_empty());
}

/// An audit target that resolves to no client row at all — and one that
/// does not even parse as a client id — land in the §4.3 category, not
/// in the candidate list and not nowhere. An inner join would drop both
/// silently; this asserts the report does not.
#[tokio::test]
async fn an_audit_target_with_no_client_row_is_unresolved() {
    let db = fresh_db();
    let missing_id = ClientId::new();
    super::append(
        &db,
        &dynamic_register_audit_row(Some(missing_id.to_string())),
    )
    .await
    .expect("seed audit row (missing client)");
    super::append(
        &db,
        &dynamic_register_audit_row(Some("not-a-client-id".into())),
    )
    .await
    .expect("seed audit row (unparseable target)");

    let report: LegacyDynamicCandidateReport = legacy_dynamic_candidates(&db).await.expect("query");
    assert!(
        report.candidates.is_empty(),
        "neither row resolves to a client, so neither is a candidate"
    );
    assert_eq!(
        report.unresolved.len(),
        2,
        "both the missing-client and the unparseable target must be reported, not dropped"
    );
}

/// §4.1's "do not filter" rule, made executable. Added by the architect
/// during review: the round reported `is_disabled`/`is_deleted` faithfully
/// but nothing asserted that a row carrying them is still *reported*.
///
/// The regression it guards is specific and plausible: adding
/// `AND is_deleted = 0` to `clients::get` -- an ordinary-looking tidy --
/// moves every soft-deleted candidate out of `candidates` and into
/// `unresolved`, the category this report itself calls strictly more
/// alarming than a candidate. Measured: under exactly that mutation all
/// four tests above still pass and only this one fails.
#[tokio::test]
async fn a_disabled_and_soft_deleted_candidate_is_still_reported_not_hidden() {
    let db = fresh_db();
    let id = ClientId::new();
    seed_client(&db, id, RegistrationSource::Admin).await;
    clients::set_disabled(&db, id, true).await.expect("disable");
    clients::soft_delete(&db, id).await.expect("soft delete");
    super::append(&db, &dynamic_register_audit_row(Some(id.to_string())))
        .await
        .expect("seed audit row");

    let report = legacy_dynamic_candidates(&db).await.expect("query");
    assert_eq!(
        report.candidates.len(),
        1,
        "a disabled+deleted candidate must still be reported"
    );
    assert!(report.candidates[0].is_disabled);
    assert!(report.candidates[0].is_deleted);
    assert!(
        report.unresolved.is_empty(),
        "it is a candidate, not an unresolved target"
    );
}
