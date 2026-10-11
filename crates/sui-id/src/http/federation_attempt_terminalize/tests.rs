use super::*;
use chrono::Duration;
use sui_id_store::crypto::MasterKey;
use sui_id_store::models::{FederationProviderRow, ProvisionMode};
use sui_id_store::repos::{audit, federation_login_attempt, federation_provider};

fn open_test_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).unwrap()
}

fn fixed_now() -> DateTime<Utc> {
    "2026-10-11T12:00:00Z".parse().unwrap()
}

async fn seed_provider(db: &Database) -> FederationProviderId {
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "test-provider".to_owned(),
        display_name: "Test Provider".to_owned(),
        issuer: "https://idp.example".to_owned(),
        client_id: "client-1".to_owned(),
        client_secret_enc: None,
        scopes: "openid email".to_owned(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: true,
        allowed_origins: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let id = row.id;
    federation_provider::create(db, &row, None).await.unwrap();
    id
}

async fn insert_attempt(
    db: &Database,
    provider_id: FederationProviderId,
) -> FederationLoginAttemptId {
    federation_login_attempt::insert(
        db,
        provider_id,
        1,
        1,
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
        b"the-pkce-verifier",
        "https://rp.example/cb".to_owned(),
        None,
        fixed_now(),
    )
    .await
    .unwrap()
    .id
}

async fn status_of(db: &Database, id: FederationLoginAttemptId) -> String {
    let id_str = id.to_string();
    db.with_read(move |read| {
        Ok(read
            .prepare("SELECT status FROM federation_login_attempt WHERE id = ?1")?
            .query_row([id_str], |r| r.get(0))?)
    })
    .await
    .unwrap()
}

/// Each of the four reasons terminalizes the attempt and records the
/// matching event name -- the "exact Class-B sum" the dispatch names, each
/// one checked by name rather than assumed from the enum alone -- and the
/// bounded `reason` token lands exactly as `reason_token()` would produce
/// it, not merely "some string".
#[tokio::test]
async fn each_reason_terminalizes_and_records_its_own_event_name() {
    let cases = [
        (
            TerminalFailureReason::UpstreamFailure(UpstreamFailureDetail::TokenExchangeFailed),
            "auth.federation.signin.upstream_failure",
            "token_exchange_failed",
        ),
        (
            TerminalFailureReason::TakeoverBlocked,
            "auth.federation.takeover_blocked",
            "email_collision",
        ),
        (
            TerminalFailureReason::LinkRequired,
            "auth.federation.link_required",
            "unlinked_identity",
        ),
        (
            TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed),
            "auth.federation.signin.denied",
            "signin_commit_failed",
        ),
    ];
    for (reason, event_name, reason_token) in cases {
        let db = open_test_db();
        let provider_id = seed_provider(&db).await;
        let attempt_id = insert_attempt(&db, provider_id).await;

        terminalize_and_audit(&db, attempt_id, reason, fixed_now(), provider_id, None).await;

        assert_eq!(status_of(&db, attempt_id).await, "failed", "{reason:?}");
        let rows = audit::recent(&db, 10).await.unwrap();
        let row = rows
            .iter()
            .find(|r| r.action == event_name)
            .unwrap_or_else(|| panic!("no {event_name} row for {reason:?}: {rows:?}"));
        assert_eq!(row.result, "denied");
        assert_eq!(
            row.note.as_deref(),
            Some(format!("provider_id={provider_id} reason={reason_token}").as_str())
        );
    }
}

/// `TerminalFailureReason::Denied` cannot carry an `UpstreamFailureDetail`
/// -- not a runtime check, a type that does not exist to construct. This
/// is not a test so much as a comment that would fail to compile if it
/// stopped being true:
/// `TerminalFailureReason::Denied(UpstreamFailureDetail::TokenParseFailed)`
/// is not an expression this file, or any other, can write.
#[test]
fn the_per_reason_pairing_is_checked_at_compile_time() {
    let _ = TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed);
    let _ = TerminalFailureReason::UpstreamFailure(UpstreamFailureDetail::TokenParseFailed);
}

/// A4's own precedent (stage 6b review): the audit log being broken must
/// not stop the state transition. `terminalize_and_audit` runs the two
/// steps independently, and this is the test that proves it rather than
/// assuming it from reading the body.
#[tokio::test]
async fn the_state_transition_survives_a_broken_audit_log() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = insert_attempt(&db, provider_id).await;
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TRIGGER terminalize_test_reject_audit BEFORE INSERT ON audit_log \
             BEGIN SELECT RAISE(ABORT, 'test: audit_log insert rejected'); END;",
        )?;
        Ok(())
    })
    .await
    .unwrap();

    terminalize_and_audit(
        &db,
        attempt_id,
        TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed),
        fixed_now(),
        provider_id,
        None,
    )
    .await;

    assert_eq!(status_of(&db, attempt_id).await, "failed");
}

/// The mirror case: a second call against an already-terminalized attempt
/// cannot terminalize it again, but must still attempt the emission --
/// `terminalize_and_audit` does not short-circuit the audit half just
/// because the state half found nothing to do.
#[tokio::test]
async fn a_second_call_against_an_already_terminalized_attempt_still_audits() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = insert_attempt(&db, provider_id).await;
    terminalize_and_audit(
        &db,
        attempt_id,
        TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed),
        fixed_now(),
        provider_id,
        None,
    )
    .await;

    terminalize_and_audit(
        &db,
        attempt_id,
        TerminalFailureReason::Denied(DeniedDetail::UserReadDbError),
        fixed_now() + Duration::seconds(1),
        provider_id,
        None,
    )
    .await;

    let rows = audit::recent(&db, 10).await.unwrap();
    let denied_rows: Vec<_> = rows
        .iter()
        .filter(|r| r.action == "auth.federation.signin.denied")
        .collect();
    assert_eq!(
        denied_rows.len(),
        2,
        "both calls' emissions must land: {rows:?}"
    );
}

/// The actor, when given, reaches the audit row -- only
/// `TerminalFailureReason::Denied`'s own descriptor declares it as
/// anything but `None`-required.
#[tokio::test]
async fn an_actor_is_carried_through_to_the_audit_row() {
    let db = open_test_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = insert_attempt(&db, provider_id).await;
    let user_id = sui_id_shared::ids::UserId::new();

    terminalize_and_audit(
        &db,
        attempt_id,
        TerminalFailureReason::Denied(DeniedDetail::SigninCommitFailed),
        fixed_now(),
        provider_id,
        Some(user_id),
    )
    .await;

    let rows = audit::recent(&db, 1).await.unwrap();
    assert_eq!(rows[0].actor, Some(user_id));
}
