//! RFC 096-B1 stage 7 — F04: first federated provisioning, in one Class-A
//! transaction. RFC 094 M2a's own criteria, applied to the newest
//! command: injected append failures roll back every write (criterion 2),
//! and one successful provisioning writes precisely one audit record
//! (criterion 3) -- plus the three rechecks the manifest itself names:
//! absent link, verified unique email, username uniqueness, each proven
//! by losing the race deliberately.

use super::*;
use crate::models::UserWebauthnCredentialRow;
use crate::models::{FederationLinkRow, FederationProviderRow, ProvisionMode};
use crate::repos::federation_login_attempt;
use crate::repos::federation_mfa_pending::{InvalidatedReason, MfaRejectionReason};
use crate::repos::user_totp;
use sui_id_shared::ids::{
    FederationLoginAttemptId, FederationMfaPendingId, FederationProviderId,
    FederationWebauthnCeremonyId, SessionId, WebauthnCredentialId,
};

async fn seed_provider(db: &Database) -> FederationProviderId {
    let now = Utc::now();
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "f04-provider".into(),
        display_name: "F04 Provider".into(),
        issuer: "https://idp.example".into(),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::ProvisionOnFirstLogin,
        enabled: true,
        allowed_origins: String::new(),
        created_at: now,
        updated_at: now,
    };
    let id = row.id;
    repos::federation_provider::create(db, &row, None)
        .await
        .expect("seed provider");
    id
}

/// A freshly inserted, claimed (`exchanging`) attempt — F04's own
/// precondition. `hash` makes each attempt's `state_sha256` distinct
/// (`UNIQUE`), so a test seeding more than one attempt does not collide
/// with itself.
async fn an_exchanging_attempt(
    db: &Database,
    provider_id: FederationProviderId,
    hash: u8,
) -> FederationLoginAttemptId {
    let now = Utc::now();
    let row = federation_login_attempt::insert(
        db,
        provider_id,
        1,
        1,
        [hash; 32],
        [hash.wrapping_add(1); 32],
        [hash.wrapping_add(2); 32],
        b"verifier",
        "https://example.com/cb".into(),
        None,
        now,
    )
    .await
    .expect("insert attempt");
    let claimed = federation_login_attempt::claim(
        db,
        row.id,
        now,
        provider_id,
        1,
        1,
        [hash.wrapping_add(2); 32],
    )
    .await
    .expect("claim attempt");
    claimed.id
}

fn a_federated_user(username: &str, email: Option<&str>) -> UserRow {
    let now = Utc::now();
    UserRow {
        id: UserId::new(),
        username: username.to_owned(),
        display_name: None,
        is_admin: false,
        role: crate::models::Role::User,
        is_disabled: false,
        is_deleted: false,
        user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
        email: email.map(str::to_owned),
        preferred_lang: None,
        email_normalized: None,
        email_verified_at: None,
        last_login_at: None,
        source: crate::models::UserSource::Local,
        external_stable_id: None,
    }
}

fn a_federated_link(
    user_id: UserId,
    provider_id: FederationProviderId,
    upstream_sub: &str,
) -> FederationLinkRow {
    let now = Utc::now();
    FederationLinkRow {
        user_id,
        provider_id,
        upstream_sub: upstream_sub.to_owned(),
        upstream_email: None,
        linked_at: now,
        last_seen_at: now,
    }
}

fn a_fed_session(user_id: UserId) -> SessionRow {
    SessionRow {
        id: SessionId::new(),
        user_id,
        expires_at: Utc::now() + TimeDelta::hours(24),
        created_at: Utc::now(),
        revoked_at: None,
        auth_methods: vec![sui_id_shared::AuthMethod::Fed],
        last_step_up_at: None,
        last_used_at: None,
    }
}

async fn attempt_status(db: &Database, id: FederationLoginAttemptId) -> String {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT status FROM federation_login_attempt WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("status")
}

// ── Happy path, rollback (criterion 2) and exactly-once (criterion 3) ───

#[tokio::test]
async fn f04_provisions_a_new_user_link_session_and_completes_the_attempt() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 1).await;
    let user = a_federated_user("f04-happy", Some("happy@example.com"));
    let user_id = user.id;
    let link = a_federated_link(user_id, provider_id, "sub-happy");
    let session = a_fed_session(user_id);
    let session_id = session.id;
    let before_count = audit_rows(&db).await;

    provision_federated_user(&db, attempt_id, provider_id, user, link, session)
        .await
        .expect("provision succeeds");

    let row = repos::users::get(&db, user_id).await.expect("user exists");
    assert_eq!(row.username, "f04-happy");
    assert!(!row.is_admin);
    assert!(
        repos::federation_link::find_by_sub(&db, provider_id, "sub-happy")
            .await
            .expect("link lookup")
            .is_some(),
        "the link was created"
    );
    assert!(
        repos::sessions::get(&db, session_id).await.is_ok(),
        "the session was created"
    );
    assert_eq!(attempt_status(&db, attempt_id).await, "completed");
    assert_eq!(audit_rows(&db).await, before_count + 1, "exactly one event");
    assert_eq!(
        latest_audit_action(&db).await,
        Some("auth.federation.provisioned".to_owned())
    );
    record_exactly_once_coverage("F04");
}

#[tokio::test]
async fn f04_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 2).await;
    let user = a_federated_user("f04-rollback", Some("rollback@example.com"));
    let user_id = user.id;
    let link = a_federated_link(user_id, provider_id, "sub-rollback");
    let session = a_fed_session(user_id);
    let session_id = session.id;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = provision_federated_user(&db, attempt_id, provider_id, user, link, session).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::users::get(&db, user_id).await.is_err(),
        "no user row was created"
    );
    assert!(
        repos::federation_link::find_by_sub(&db, provider_id, "sub-rollback")
            .await
            .expect("link lookup")
            .is_none(),
        "no link row was created"
    );
    assert!(
        repos::sessions::get(&db, session_id).await.is_err(),
        "no session row was created"
    );
    assert_eq!(
        attempt_status(&db, attempt_id).await,
        "exchanging",
        "the attempt was not completed either"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // Control: the same call, with no failure injected, really does create
    // everything -- without this, the assertions above would pass just as
    // well if the mutation never ran at all.
    let control_user =
        a_federated_user("f04-rollback-control", Some("rollback-control@example.com"));
    let control_user_id = control_user.id;
    let control_link = a_federated_link(control_user_id, provider_id, "sub-rollback-control");
    let control_session = a_fed_session(control_user_id);
    provision_federated_user(
        &db,
        attempt_id,
        provider_id,
        control_user,
        control_link,
        control_session,
    )
    .await
    .expect("the control succeeds");
    assert!(repos::users::get(&db, control_user_id).await.is_ok());
    assert_eq!(attempt_status(&db, attempt_id).await, "completed");
    record_rollback_coverage("F04");
}

// ── Dispatch item 2: the three rechecks, each losing the race ──────────

/// A link to the same upstream identity, created *after* the handler's own
/// check but *before* this transaction -- the race F04's own recheck
/// exists to catch.
#[tokio::test]
async fn f04_refuses_when_a_link_for_the_same_upstream_identity_already_exists() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 3).await;

    let winner = a_federated_user("f04-race-link-winner", None);
    repos::users::create(&db, &winner)
        .await
        .expect("seed winning user");
    repos::federation_link::upsert(
        &db,
        a_federated_link(winner.id, provider_id, "sub-link-race"),
    )
    .await
    .expect("seed winning link");

    let loser = a_federated_user("f04-race-link-loser", None);
    let loser_id = loser.id;
    let link = a_federated_link(loser_id, provider_id, "sub-link-race");
    let session = a_fed_session(loser_id);

    let result = provision_federated_user(&db, attempt_id, provider_id, loser, link, session).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "a concurrent link for the same upstream identity must refuse, not absorb"
    );
    assert!(
        repos::users::get(&db, loser_id).await.is_err(),
        "the loser's user row did not survive the rollback"
    );
}

/// A different user already holding the same normalized email, created
/// after the handler's own collision check but before this transaction.
#[tokio::test]
async fn f04_refuses_when_the_email_has_already_been_taken() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 4).await;

    let existing = a_federated_user("f04-race-email-existing", Some("race@example.com"));
    repos::users::create(&db, &existing)
        .await
        .expect("seed existing user");

    let loser = a_federated_user("f04-race-email-loser", Some("race@example.com"));
    let loser_id = loser.id;
    let link = a_federated_link(loser_id, provider_id, "sub-email-race");
    let session = a_fed_session(loser_id);

    let result = provision_federated_user(&db, attempt_id, provider_id, loser, link, session).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "a concurrently taken email must refuse, not create a second user with it"
    );
    assert!(repos::users::get(&db, loser_id).await.is_err());
    assert!(
        repos::federation_link::find_by_sub(&db, provider_id, "sub-email-race")
            .await
            .expect("link lookup")
            .is_none(),
        "the link insert rolled back with everything else"
    );
}

/// A different user already holding the exact username `resolve_shadow_
/// username` resolved to, created after the handler's own resolution but
/// before this transaction.
#[tokio::test]
async fn f04_refuses_when_the_username_has_already_been_taken() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 5).await;

    let existing = a_federated_user("f04-race-username", None);
    repos::users::create(&db, &existing)
        .await
        .expect("seed existing user");

    let loser = a_federated_user("f04-race-username", None);
    let loser_id = loser.id;
    let link = a_federated_link(loser_id, provider_id, "sub-username-race");
    let session = a_fed_session(loser_id);

    let result = provision_federated_user(&db, attempt_id, provider_id, loser, link, session).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "a concurrently taken username must refuse, not create a second user with it"
    );
    assert!(
        repos::federation_link::find_by_sub(&db, provider_id, "sub-username-race")
            .await
            .expect("link lookup")
            .is_none(),
        "the link insert rolled back with everything else"
    );
}

/// Dispatch item 1: an attempt that is no longer `exchanging` (lost a race
/// against F05, or already completed by a concurrent commit) refuses
/// before any of the other three writes, rather than creating a user for
/// an attempt that can no longer be completed.
#[tokio::test]
async fn f04_refuses_when_the_attempt_is_no_longer_exchanging() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 6).await;
    federation_login_attempt::terminalize_failed(&db, attempt_id, Utc::now())
        .await
        .expect("F05 wins the race first");

    let user = a_federated_user("f04-attempt-lost-race", None);
    let user_id = user.id;
    let link = a_federated_link(user_id, provider_id, "sub-attempt-race");
    let session = a_fed_session(user_id);

    let result = provision_federated_user(&db, attempt_id, provider_id, user, link, session).await;
    assert!(matches!(result, Err(StoreError::Conflict)));
    assert!(repos::users::get(&db, user_id).await.is_err());
    assert_eq!(attempt_status(&db, attempt_id).await, "failed");
}

// ── RFC 096-B1 stage 9 — F02: existing link, local MFA required ────────

/// `provider_config_version`/`provider_activation_generation` fixed at 1/1
/// to match `an_exchanging_attempt`'s own hardcoded values — the row's
/// identity columns must agree with the attempt they belong to, the same
/// way F04's `link`/`session` already have to agree with `user`.
/// `issue_federated_mfa_pending` builds and seals the row itself, so every
/// call site here just supplies the raw pieces.
#[allow(clippy::too_many_arguments)]
async fn call_f02(
    db: &Database,
    attempt_id: FederationLoginAttemptId,
    provider_id: FederationProviderId,
    user_id: UserId,
    now: DateTime<Utc>,
) -> StoreResult<FederationMfaPendingId> {
    issue_federated_mfa_pending(
        db,
        attempt_id,
        provider_id,
        1,
        1,
        user_id,
        None,
        [9; 32],
        [10; 32],
        now,
    )
    .await
}

async fn pending_count(db: &Database) -> i64 {
    db.with_conn(|c| {
        Ok(
            c.query_row("SELECT COUNT(*) FROM federation_mfa_pending", [], |r| {
                r.get(0)
            })?,
        )
    })
    .await
    .expect("count")
}

#[tokio::test]
async fn f02_completes_the_attempt_and_creates_exactly_one_pending_row() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 10).await;
    let user = a_federated_user("f02-happy", None);
    let user_id = user.id;
    repos::users::create(&db, &user).await.expect("seed user");
    repos::federation_link::upsert(&db, a_federated_link(user_id, provider_id, "sub-f02-happy"))
        .await
        .expect("seed link");

    let pending_id = call_f02(&db, attempt_id, provider_id, user_id, Utc::now())
        .await
        .expect("F02 succeeds");

    assert_eq!(attempt_status(&db, attempt_id).await, "completed");
    assert_eq!(pending_count(&db).await, 1);
    let stored_id: String = db
        .with_conn(|c| Ok(c.query_row("SELECT id FROM federation_mfa_pending", [], |r| r.get(0))?))
        .await
        .expect("stored id");
    assert_eq!(stored_id, pending_id.to_string());
    record_exactly_once_coverage("F02");
}

/// RFC 094 M2a criterion 2, by hand (dispatch item 4: G20 does not cover
/// Class-P commands, so no gate asks for this). `fail_next_commit_for_test`
/// rejects the real SQLite commit regardless of what ran inside it —
/// unlike `fail_before_next_append`, which only fires on an audit append
/// F02 never makes (Class P has none by construction) — so this is the
/// mechanism that actually reaches a Protocol transaction's own commit.
#[tokio::test]
async fn f02_injected_commit_failure_rolls_back_everything() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 11).await;
    let user = a_federated_user("f02-rollback", None);
    let user_id = user.id;
    repos::users::create(&db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        &db,
        a_federated_link(user_id, provider_id, "sub-f02-rollback"),
    )
    .await
    .expect("seed link");

    db.fail_next_commit_for_test();
    let result = call_f02(&db, attempt_id, provider_id, user_id, Utc::now()).await;
    assert!(
        result.is_err(),
        "a rejected commit must surface as Err, not a silently-empty Ok"
    );
    assert_eq!(
        attempt_status(&db, attempt_id).await,
        "exchanging",
        "the attempt must not have been completed either"
    );
    assert_eq!(pending_count(&db).await, 0, "no pending row was created");

    // Control: the same call, no failure injected, for a second untouched
    // identity, really does write both rows -- without this, the
    // assertions above would pass just as well if the injection never ran.
    let control_attempt_id = an_exchanging_attempt(&db, provider_id, 12).await;
    let control_user = a_federated_user("f02-rollback-control", None);
    let control_user_id = control_user.id;
    repos::users::create(&db, &control_user)
        .await
        .expect("seed control user");
    repos::federation_link::upsert(
        &db,
        a_federated_link(control_user_id, provider_id, "sub-f02-rollback-control"),
    )
    .await
    .expect("seed control link");
    call_f02(
        &db,
        control_attempt_id,
        provider_id,
        control_user_id,
        Utc::now(),
    )
    .await
    .expect("the control succeeds");
    assert_eq!(attempt_status(&db, control_attempt_id).await, "completed");
    assert_eq!(pending_count(&db).await, 1);
    record_rollback_coverage("F02");
}

#[tokio::test]
async fn f02_refuses_when_the_attempt_is_no_longer_exchanging() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 13).await;
    federation_login_attempt::terminalize_failed(&db, attempt_id, Utc::now())
        .await
        .expect("F05 wins the race first");
    let user = a_federated_user("f02-attempt-lost-race", None);
    let user_id = user.id;
    repos::users::create(&db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        &db,
        a_federated_link(user_id, provider_id, "sub-f02-attempt-race"),
    )
    .await
    .expect("seed link");

    let result = call_f02(&db, attempt_id, provider_id, user_id, Utc::now()).await;
    assert!(matches!(result, Err(StoreError::Conflict)));
    assert_eq!(pending_count(&db).await, 0);
    assert_eq!(attempt_status(&db, attempt_id).await, "failed");
}

/// Stage 9's own "active link" recheck, exercised through the command
/// (not only the repo-level test in `repos::federation_mfa_pending::
/// tests`): a link that existed when the caller resolved `user_id`, but is
/// gone by the time this transaction runs, refuses rather than creating a
/// pending row for an identity no longer linked.
#[tokio::test]
async fn f02_refuses_when_the_link_has_been_removed() {
    let db = fresh_db();
    let provider_id = seed_provider(&db).await;
    let attempt_id = an_exchanging_attempt(&db, provider_id, 14).await;
    let user = a_federated_user("f02-unlinked-race", None);
    let user_id = user.id;
    repos::users::create(&db, &user).await.expect("seed user");
    // Deliberately no federation_link row.

    let result = call_f02(&db, attempt_id, provider_id, user_id, Utc::now()).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "a pending row for an unlinked pair must refuse"
    );
    assert_eq!(pending_count(&db).await, 0);
    assert_eq!(
        attempt_status(&db, attempt_id).await,
        "exchanging",
        "the attempt completion rolled back with the refused insert"
    );
}

// ── RFC 096-B1 stage 11b, RFC 096 `:756`: a restart preserves count/
//    status ──────────────────────────────────────────────────────────

async fn failure_count_and_status(
    db: &Database,
    id: FederationMfaPendingId,
) -> (i64, crate::models::FederationMfaPendingStatus) {
    let row = repos::federation_mfa_pending::find_by_id(db, id)
        .await
        .unwrap()
        .expect("row exists");
    (row.failure_count, row.status)
}

/// Direct SQL, deliberately: these tests are about F02's own restart
/// behavior, not F03's counting mechanism (already exercised in full
/// above/elsewhere) — setting the count this way needs no TOTP
/// enrolment and no real wrong proofs, just a row sitting at a given
/// count when the restart arrives.
async fn set_failure_count_and_status(
    db: &Database,
    id: FederationMfaPendingId,
    failure_count: i64,
    status: &str,
) {
    let id_str = id.to_string();
    let status = status.to_owned();
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE federation_mfa_pending \
             SET failure_count = ?1, status = ?2, \
                 continuation_sealed = CASE WHEN ?2 = 'pending' THEN continuation_sealed ELSE NULL END \
             WHERE id = ?3",
            rusqlite::params![failure_count, status, id_str],
        )?;
        Ok(())
    })
    .await
    .unwrap();
}

async fn active_row_count_for_user(db: &Database, user_id: UserId) -> i64 {
    let user_id = user_id.to_string();
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM federation_mfa_pending \
             WHERE user_id = ?1 AND status IN ('pending', 'exhausted')",
            [user_id],
            |r| r.get(0),
        )?)
    })
    .await
    .unwrap()
}

struct RestartFixture {
    provider_id: FederationProviderId,
    user_id: UserId,
}

async fn restart_fixture(db: &Database, tag: u8) -> RestartFixture {
    let provider_id = seed_provider(db).await;
    let user = a_federated_user(&format!("restart-{tag}"), None);
    let user_id = user.id;
    repos::users::create(db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        db,
        a_federated_link(user_id, provider_id, &format!("sub-restart-{tag}")),
    )
    .await
    .expect("seed link");
    RestartFixture {
        provider_id,
        user_id,
    }
}

/// The dispatch's own worked example, exactly: a restart while a
/// continuation sits at `failure_count = 4` must not produce a second
/// row at 0. "Written so it fails if the reset comes back" is this
/// test's own mutation, run below it.
#[tokio::test]
async fn f02_restart_at_count_four_does_not_grant_a_fresh_budget() {
    let db = fresh_db();
    let f = restart_fixture(&db, 1).await;
    let attempt_1 = an_exchanging_attempt(&db, f.provider_id, 1).await;
    let first_id = issue_federated_mfa_pending(
        &db,
        attempt_1,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [1; 32],
        [2; 32],
        Utc::now(),
    )
    .await
    .expect("F02 succeeds");
    set_failure_count_and_status(&db, first_id, 4, "pending").await;

    let attempt_2 = an_exchanging_attempt(&db, f.provider_id, 2).await;
    let second_id = issue_federated_mfa_pending(
        &db,
        attempt_2,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [3; 32],
        [4; 32],
        Utc::now(),
    )
    .await
    .expect("the restart succeeds");

    assert_eq!(second_id, first_id, "the restart must reuse the same row");
    let (failure_count, status) = failure_count_and_status(&db, first_id).await;
    assert_eq!(failure_count, 4, "the budget must survive the restart");
    assert_eq!(status, crate::models::FederationMfaPendingStatus::Pending);
    assert_eq!(
        active_row_count_for_user(&db, f.user_id).await,
        1,
        "exactly one active row, not two"
    );
}

/// RFC 096 `:756`'s own stickiest case, named by the dispatch as the one
/// to be most careful with: an exhausted row must not become a fresh
/// pending one through a restart, no matter how many times it is tried.
#[tokio::test]
async fn f02_restart_while_exhausted_stays_exhausted() {
    let db = fresh_db();
    let f = restart_fixture(&db, 3).await;
    let attempt_1 = an_exchanging_attempt(&db, f.provider_id, 3).await;
    let first_id = issue_federated_mfa_pending(
        &db,
        attempt_1,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [5; 32],
        [6; 32],
        Utc::now(),
    )
    .await
    .expect("F02 succeeds");
    set_failure_count_and_status(&db, first_id, 5, "exhausted").await;

    for tag in [4u8, 5u8] {
        let attempt = an_exchanging_attempt(&db, f.provider_id, tag).await;
        let id = issue_federated_mfa_pending(
            &db,
            attempt,
            f.provider_id,
            0,
            0,
            f.user_id,
            None,
            [tag; 32],
            [tag.wrapping_add(10); 32],
            Utc::now(),
        )
        .await
        .expect("the restart succeeds");
        assert_eq!(id, first_id, "every restart returns the same exhausted row");
    }
    let (failure_count, status) = failure_count_and_status(&db, first_id).await;
    assert_eq!(failure_count, 5);
    assert_eq!(status, crate::models::FederationMfaPendingStatus::Exhausted);
    assert_eq!(active_row_count_for_user(&db, f.user_id).await, 1);
}

/// The "reuse" half of the mechanism, not only "the count survives":
/// the three per-attempt artifacts a new upstream round trip actually
/// produces are refreshed, so the restarted attempt's own cookies
/// validate against the row afterward.
#[tokio::test]
async fn f02_restart_while_still_live_refreshes_the_bindings() {
    let db = fresh_db();
    let f = restart_fixture(&db, 6).await;
    let attempt_1 = an_exchanging_attempt(&db, f.provider_id, 6).await;
    let now = Utc::now();
    let first_id = issue_federated_mfa_pending(
        &db,
        attempt_1,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [7; 32],
        [8; 32],
        now,
    )
    .await
    .expect("F02 succeeds");
    set_failure_count_and_status(&db, first_id, 2, "pending").await;

    let attempt_2 = an_exchanging_attempt(&db, f.provider_id, 7).await;
    let second_id = issue_federated_mfa_pending(
        &db,
        attempt_2,
        f.provider_id,
        0,
        0,
        f.user_id,
        Some("/next".to_owned()),
        [11; 32],
        [12; 32],
        now,
    )
    .await
    .expect("the restart succeeds");

    assert_eq!(second_id, first_id);
    let row = repos::federation_mfa_pending::find_by_id(&db, first_id)
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.browser_binding_sha256, [11; 32]);
    assert_eq!(row.csrf_binding_sha256, [12; 32]);
    assert_eq!(row.next_path, Some("/next".to_owned()));
    assert_eq!(
        row.failure_count, 2,
        "the budget is untouched by the refresh"
    );
}

/// A row already expired by wall clock, but not yet durably
/// terminalized, is not a budget worth protecting — F03's own expiry
/// check would refuse to count a proof against it either way. A
/// restart at this point must still work, not hand back a continuation
/// that can never be completed.
#[tokio::test]
async fn f02_restart_after_expiry_invalidates_the_stale_row_and_issues_a_fresh_one() {
    let db = fresh_db();
    let f = restart_fixture(&db, 8).await;
    let attempt_1 = an_exchanging_attempt(&db, f.provider_id, 8).await;
    let t0 = Utc::now();
    let first_id = issue_federated_mfa_pending(
        &db,
        attempt_1,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [13; 32],
        [14; 32],
        t0,
    )
    .await
    .expect("F02 succeeds");
    set_failure_count_and_status(&db, first_id, 3, "pending").await;

    let attempt_2 = an_exchanging_attempt(&db, f.provider_id, 9).await;
    let after_expiry = t0 + TimeDelta::seconds(301);
    let second_id = issue_federated_mfa_pending(
        &db,
        attempt_2,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [15; 32],
        [16; 32],
        after_expiry,
    )
    .await
    .expect("the restart succeeds");

    assert_ne!(
        second_id, first_id,
        "an already-dead continuation is not reused"
    );
    let (old_failure_count, old_status) = failure_count_and_status(&db, first_id).await;
    assert_eq!(
        old_failure_count, 3,
        "the stale row's own count is untouched by its own invalidation"
    );
    assert_eq!(
        old_status,
        crate::models::FederationMfaPendingStatus::Invalidated
    );
    let (new_failure_count, new_status) = failure_count_and_status(&db, second_id).await;
    assert_eq!(new_failure_count, 0);
    assert_eq!(
        new_status,
        crate::models::FederationMfaPendingStatus::Pending
    );
    assert_eq!(
        active_row_count_for_user(&db, f.user_id).await,
        1,
        "the stale, now-invalidated row is outside the active set"
    );
}

/// A second provider, distinct from `seed_provider`'s own fixed
/// `"f04-provider"` slug (`UNIQUE`) -- for the one test in this file
/// that needs two providers linked to the same user at once.
async fn seed_second_provider(db: &Database, tag: u8) -> FederationProviderId {
    let now = Utc::now();
    let row = FederationProviderRow {
        id: FederationProviderId::new(),
        slug: format!("restart-provider-b-{tag}"),
        display_name: "Restart Provider B".into(),
        issuer: "https://idp-b.example".into(),
        client_id: "client-b".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::ProvisionOnFirstLogin,
        enabled: true,
        allowed_origins: String::new(),
        created_at: now,
        updated_at: now,
    };
    let id = row.id;
    repos::federation_provider::create(db, &row, None)
        .await
        .expect("seed second provider");
    id
}

/// RFC 096-B1 stage 11b fix round, R1 — the review's own measured
/// probe, as a real test: a live `federation_webauthn_ceremony` row's
/// composite foreign key has `ON DELETE CASCADE` and no `ON UPDATE`
/// clause, so `NO ACTION` governs its three identity columns.
/// `refresh_for_restart_within_tx` changes exactly those three on a
/// restart through a *different* linked provider — before the fix,
/// this failed with a raw `FOREIGN KEY constraint failed`
/// (`StoreError::Db`), not the reuse the stage exists to perform.
#[tokio::test]
async fn f02_restart_through_a_different_provider_consumes_a_live_ceremony() {
    let db = fresh_db();
    let provider_a = seed_provider(&db).await;
    let provider_b = seed_second_provider(&db, 90).await;
    let user = a_federated_user("restart-ceremony-cross", None);
    let user_id = user.id;
    repos::users::create(&db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        &db,
        a_federated_link(user_id, provider_a, "sub-restart-ceremony-cross-a"),
    )
    .await
    .expect("seed link a");
    repos::federation_link::upsert(
        &db,
        a_federated_link(user_id, provider_b, "sub-restart-ceremony-cross-b"),
    )
    .await
    .expect("seed link b");
    seed_webauthn_credential(&db, user_id, 90).await;

    let attempt_1 = an_exchanging_attempt(&db, provider_a, 90).await;
    let now = Utc::now();
    let first_id = issue_federated_mfa_pending(
        &db, attempt_1, provider_a, 0, 0, user_id, None, [20; 32], [21; 32], now,
    )
    .await
    .expect("F02 succeeds");
    begin_federation_webauthn_ceremony(
        &db,
        first_id,
        b"sealed-state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .expect("F06 succeeds");
    assert!(
        repos::federation_webauthn_ceremony::find_by_pending_id(&db, first_id)
            .await
            .unwrap()
            .is_some(),
        "control: the ceremony exists before the restart"
    );

    let attempt_2 = an_exchanging_attempt(&db, provider_b, 91).await;
    let second_id = issue_federated_mfa_pending(
        &db, attempt_2, provider_b, 0, 0, user_id, None, [22; 32], [23; 32], now,
    )
    .await
    .expect("the restart must succeed despite the live ceremony, not surface a raw FK error");

    assert_eq!(second_id, first_id, "reuse, not a fresh row");
    assert!(
        repos::federation_webauthn_ceremony::find_by_pending_id(&db, first_id)
            .await
            .unwrap()
            .is_none(),
        "the ceremony bound to the previous round's identity is consumed, \
         not silently repointed at the new one"
    );
    let row = repos::federation_mfa_pending::find_by_id(&db, first_id)
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(
        row.provider_id, provider_b,
        "the row now reflects the restart's own provider"
    );
}

/// The same consumption, without a provider change — "do not
/// special-case `provider_id` alone" (the dispatch's own instruction):
/// the fix must not be narrow enough to miss the day
/// `provider_config_version`/`provider_activation_generation` stop
/// being fixed at `(0, 0)`.
#[tokio::test]
async fn f02_restart_through_the_same_provider_still_consumes_a_live_ceremony() {
    let db = fresh_db();
    let f = restart_fixture(&db, 92).await;
    seed_webauthn_credential(&db, f.user_id, 92).await;

    let attempt_1 = an_exchanging_attempt(&db, f.provider_id, 92).await;
    let now = Utc::now();
    let first_id = issue_federated_mfa_pending(
        &db,
        attempt_1,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [24; 32],
        [25; 32],
        now,
    )
    .await
    .expect("F02 succeeds");
    begin_federation_webauthn_ceremony(
        &db,
        first_id,
        b"sealed-state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .expect("F06 succeeds");

    let attempt_2 = an_exchanging_attempt(&db, f.provider_id, 93).await;
    let second_id = issue_federated_mfa_pending(
        &db,
        attempt_2,
        f.provider_id,
        0,
        0,
        f.user_id,
        None,
        [26; 32],
        [27; 32],
        now,
    )
    .await
    .expect("the restart succeeds");

    assert_eq!(second_id, first_id);
    assert!(
        repos::federation_webauthn_ceremony::find_by_pending_id(&db, first_id)
            .await
            .unwrap()
            .is_none(),
        "retry requires F06, unconditionally on every restart"
    );
}

// ── F03 — submit/complete federated local MFA (Protocol) ───────────────
//
// `complete_federated_mfa` trusts its own caller's proof the way the
// manifest asks (RFC 096-B1 stage 10b dispatch `:72-74`): these tests
// never go through the verifier (`sui-id-core`, a downstream crate this
// one cannot depend on) -- they call F03 directly with the plain
// primitives a real promotion path would have already produced, the
// same shape `complete_second_factor`'s own tests use for L02.

struct F03Fixture {
    provider_id: FederationProviderId,
    user_id: UserId,
    pending_id: FederationMfaPendingId,
}

/// Provider, user, link, a claimed attempt, a real F02-created pending
/// row, and a confirmed TOTP enrolment with recovery codes
/// `["hash-a","hash-b"]` -- `tag` keeps every seeded identifier distinct
/// across tests sharing one `fresh_db()`.
async fn f03_fixture(db: &Database, tag: u8) -> F03Fixture {
    let provider_id = seed_provider(db).await;
    let user = a_federated_user(&format!("f03-{tag}"), None);
    let user_id = user.id;
    repos::users::create(db, &user).await.expect("seed user");
    repos::federation_link::upsert(
        db,
        a_federated_link(user_id, provider_id, &format!("sub-f03-{tag}")),
    )
    .await
    .expect("seed link");
    let attempt_id = an_exchanging_attempt(db, provider_id, tag).await;
    let pending_id = call_f02(db, attempt_id, provider_id, user_id, Utc::now())
        .await
        .expect("F02 succeeds");
    user_totp::upsert_pending(db, user_id, b"f03-fixture-totp-secret")
        .await
        .expect("seed totp pending");
    user_totp::confirm_with_recovery(db, user_id, br#"["hash-a","hash-b"]"#)
        .await
        .expect("confirm totp");
    F03Fixture {
        provider_id,
        user_id,
        pending_id,
    }
}

async fn row_status(
    db: &Database,
    id: FederationMfaPendingId,
) -> crate::models::FederationMfaPendingStatus {
    repos::federation_mfa_pending::find_by_id(db, id)
        .await
        .expect("read")
        .expect("row exists")
        .status
}

#[tokio::test]
async fn f03_totp_promotes_consumes_step_observes_link_and_creates_session() {
    let db = fresh_db();
    let f = f03_fixture(&db, 30).await;
    let now = Utc::now();

    let outcome = complete_federated_mfa(&db, f.pending_id, Some(F03Proof::Totp { step: 5 }), now)
        .await
        .expect("promotion succeeds");
    match outcome {
        F03Outcome::Promoted {
            session,
            evicted,
            method,
        } => {
            assert_eq!(session.user_id, f.user_id);
            assert_eq!(
                session.auth_methods,
                vec![
                    sui_id_shared::AuthMethod::Fed,
                    sui_id_shared::AuthMethod::Totp
                ]
            );
            assert_eq!(session.last_step_up_at, Some(now));
            assert_eq!(evicted, 0);
            assert_eq!(method, "totp");
        }
        other => panic!("expected Promoted, got {other:?}"),
    }

    let row = repos::federation_mfa_pending::find_by_id(&db, f.pending_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.status,
        crate::models::FederationMfaPendingStatus::Completed
    );
    assert!(row.continuation_sealed.is_none());
    let totp_row = user_totp::get(&db, f.user_id).await.unwrap().unwrap();
    assert_eq!(totp_row.last_used_step, 5, "the step was advanced durably");
    let link = repos::federation_link::find_by_sub(&db, f.provider_id, "sub-f03-30")
        .await
        .unwrap()
        .expect("link still exists");
    assert!(
        link.last_seen_at > link.linked_at,
        "last_seen_at was touched by the promotion"
    );
    record_exactly_once_coverage("F03");
}

#[tokio::test]
async fn f03_recovery_promotes_and_swaps_the_consumed_hash() {
    let db = fresh_db();
    let f = f03_fixture(&db, 31).await;
    let now = Utc::now();
    let expected_sealed = user_totp::get(&db, f.user_id)
        .await
        .unwrap()
        .unwrap()
        .recovery_codes_enc
        .expect("recovery codes sealed");
    let new_sealed = user_totp::seal_recovery_codes(&db, br#"["hash-b"]"#).unwrap();

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Recovery {
            expected_sealed,
            new_sealed: new_sealed.clone(),
        }),
        now,
    )
    .await
    .expect("promotion succeeds");
    match outcome {
        F03Outcome::Promoted { session, .. } => {
            assert_eq!(
                session.auth_methods,
                vec![
                    sui_id_shared::AuthMethod::Fed,
                    sui_id_shared::AuthMethod::RecoveryCode
                ]
            );
            assert_eq!(
                session.last_step_up_at, None,
                "a recovery code is not a fresh factor (RFC 102 N5)"
            );
        }
        other => panic!("expected Promoted, got {other:?}"),
    }
    let totp_row = user_totp::get(&db, f.user_id).await.unwrap().unwrap();
    assert_eq!(totp_row.recovery_codes_enc, Some(new_sealed));
}

#[tokio::test]
async fn f03_wrong_proof_increments_to_one_through_four() {
    let db = fresh_db();
    let f = f03_fixture(&db, 32).await;
    for expected in 1..=4 {
        let outcome = complete_federated_mfa(
            &db,
            f.pending_id,
            Some(F03Proof::Rejected {
                reason: MfaRejectionReason::Totp,
            }),
            Utc::now(),
        )
        .await
        .expect("rejection succeeds");
        match outcome {
            F03Outcome::RejectedStillPending {
                failure_count,
                reason,
            } => {
                assert_eq!(failure_count, expected);
                assert_eq!(reason, MfaRejectionReason::Totp);
            }
            other => panic!("expected RejectedStillPending({expected}), got {other:?}"),
        }
        assert_eq!(
            row_status(&db, f.pending_id).await,
            crate::models::FederationMfaPendingStatus::Pending
        );
    }
}

#[tokio::test]
async fn f03_fifth_wrong_proof_exhausts_and_clears_continuation() {
    let db = fresh_db();
    let f = f03_fixture(&db, 33).await;
    for _ in 1..=4 {
        complete_federated_mfa(
            &db,
            f.pending_id,
            Some(F03Proof::Rejected {
                reason: MfaRejectionReason::Malformed,
            }),
            Utc::now(),
        )
        .await
        .expect("rejection succeeds");
    }
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::Malformed,
        }),
        Utc::now(),
    )
    .await
    .expect("the fifth rejection succeeds");
    match outcome {
        F03Outcome::AttemptsExhausted { reason } => {
            assert_eq!(reason, MfaRejectionReason::Malformed)
        }
        other => panic!("expected AttemptsExhausted, got {other:?}"),
    }
    let row = repos::federation_mfa_pending::find_by_id(&db, f.pending_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.status,
        crate::models::FederationMfaPendingStatus::Exhausted
    );
    assert!(row.continuation_sealed.is_none());

    // No sixth increment exists.
    let sixth = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::Malformed,
        }),
        Utc::now(),
    )
    .await;
    assert!(matches!(sixth, Err(StoreError::Conflict)));
}

#[tokio::test]
async fn f03_invalidated_on_expiry() {
    let db = fresh_db();
    let f = f03_fixture(&db, 34).await;
    let past_expiry = Utc::now() + TimeDelta::seconds(400);
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        past_expiry,
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => assert_eq!(reason, InvalidatedReason::Expired),
        other => panic!("expected Invalidated(Expired), got {other:?}"),
    }
    assert_eq!(
        row_status(&db, f.pending_id).await,
        crate::models::FederationMfaPendingStatus::Invalidated
    );
}

#[tokio::test]
async fn f03_invalidated_on_disabled_provider() {
    let db = fresh_db();
    let f = f03_fixture(&db, 35).await;
    repos::federation_provider::set_enabled(&db, f.provider_id, false, Utc::now())
        .await
        .expect("disable provider");
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => {
            assert_eq!(reason, InvalidatedReason::ProviderDisabled)
        }
        other => panic!("expected Invalidated(ProviderDisabled), got {other:?}"),
    }
}

#[tokio::test]
async fn f03_invalidated_on_disabled_user() {
    let db = fresh_db();
    let f = f03_fixture(&db, 36).await;
    repos::users::set_disabled(&db, f.user_id, true)
        .await
        .expect("disable user");
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => assert_eq!(reason, InvalidatedReason::UserInactive),
        other => panic!("expected Invalidated(UserInactive), got {other:?}"),
    }
}

#[tokio::test]
async fn f03_invalidated_when_the_factor_is_disabled_after_the_row_was_issued() {
    let db = fresh_db();
    let f = f03_fixture(&db, 37).await;
    db.with_conn({
        let user_id = f.user_id;
        move |conn| {
            conn.execute(
                "UPDATE user_totp SET enabled = 0 WHERE user_id = ?1",
                [user_id.to_string()],
            )?;
            Ok(())
        }
    })
    .await
    .expect("disable totp");
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => assert_eq!(reason, InvalidatedReason::FactorChanged),
        other => panic!("expected Invalidated(FactorChanged), got {other:?}"),
    }
}

/// RFC 096-B1 stage 10b fix round, R2. Under normal, FK-enforced
/// operation this scenario cannot arise against a *live* pending row:
/// `federation_mfa_pending`'s own composite `FOREIGN KEY ... ON DELETE
/// CASCADE` (migration 0047) removes the row itself the instant the
/// link is removed — measured directly and reported in this round's
/// own package. This test simulates the one gap the cascade is not
/// itself the defense against: `PRAGMA foreign_keys` off for one
/// statement, deleting the link without cascading the row, then back
/// on before calling F03 — the scenario `federation_link::exists_
/// within_tx`'s own doc names as what it is actually for.
#[tokio::test]
async fn f03_invalidated_when_the_link_is_removed_without_the_row_cascading() {
    let db = fresh_db();
    let f = f03_fixture(&db, 42).await;
    db.with_conn({
        let user_id = f.user_id;
        let provider_id = f.provider_id;
        move |conn| {
            conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
            conn.execute(
                "DELETE FROM federation_link WHERE user_id = ?1 AND provider_id = ?2",
                [user_id.to_string(), provider_id.to_string()],
            )?;
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            Ok(())
        }
    })
    .await
    .expect("remove the link without cascading the row");

    // Control: the row really did survive the probe -- without this,
    // the Invalidated(LinkChanged) result below would be
    // indistinguishable from a cascaded-away row's own NotFound.
    assert!(
        repos::federation_mfa_pending::find_by_id(&db, f.pending_id)
            .await
            .unwrap()
            .is_some(),
        "the probe must leave the pending row in place"
    );

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => assert_eq!(reason, InvalidatedReason::LinkChanged),
        other => panic!("expected Invalidated(LinkChanged), got {other:?}"),
    }
}

#[tokio::test]
async fn f03_stale_totp_step_is_treated_as_a_wrong_proof() {
    let db = fresh_db();
    let f = f03_fixture(&db, 38).await;
    // Advance last_used_step to 5 first (a real promotion, or a direct
    // write -- direct write here, since only the freshness boundary is
    // under test).
    db.with_conn({
        let user_id = f.user_id;
        move |conn| {
            conn.execute(
                "UPDATE user_totp SET last_used_step = 5 WHERE user_id = ?1",
                [user_id.to_string()],
            )?;
            Ok(())
        }
    })
    .await
    .expect("advance step");

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        F03Outcome::RejectedStillPending {
            failure_count,
            reason,
        } => {
            assert_eq!(failure_count, 1);
            assert_eq!(reason, MfaRejectionReason::Totp);
        }
        other => panic!("expected RejectedStillPending(Totp), got {other:?}"),
    }
}

#[tokio::test]
async fn f03_stale_recovery_blob_is_treated_as_a_wrong_proof() {
    let db = fresh_db();
    let f = f03_fixture(&db, 39).await;
    let stale_sealed = user_totp::seal_recovery_codes(&db, br#"["already-replaced"]"#).unwrap();

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Recovery {
            expected_sealed: stale_sealed,
            new_sealed: vec![],
        }),
        Utc::now(),
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        F03Outcome::RejectedStillPending { reason, .. } => {
            assert_eq!(reason, MfaRejectionReason::Recovery)
        }
        other => panic!("expected RejectedStillPending(Recovery), got {other:?}"),
    }
}

#[tokio::test]
async fn f03_unknown_pending_id_is_not_found() {
    let db = fresh_db();
    let result = complete_federated_mfa(
        &db,
        FederationMfaPendingId::new(),
        Some(F03Proof::Totp { step: 1 }),
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(StoreError::NotFound)));
}

/// RFC 094 M2a criterion 2, by hand (the same reason `f02_injected_
/// commit_failure_rolls_back_everything` is): G20 does not cover Class-P
/// commands.
#[tokio::test]
async fn f03_injected_commit_failure_rolls_back_everything() {
    let db = fresh_db();
    let f = f03_fixture(&db, 40).await;
    db.fail_next_commit_for_test();
    let result = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        row_status(&db, f.pending_id).await,
        crate::models::FederationMfaPendingStatus::Pending,
        "the guarded status flip must not have committed"
    );
    let totp_row = user_totp::get(&db, f.user_id).await.unwrap().unwrap();
    assert_eq!(
        totp_row.last_used_step, 0,
        "the step advance must not have committed either"
    );

    // Control: the same call, no failure injected, really promotes --
    // without this, the assertions above would pass identically if the
    // injection never ran at all.
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("the control succeeds");
    assert!(matches!(outcome, F03Outcome::Promoted { .. }));
    record_rollback_coverage("F03");
}

/// RFC 096-B1 stage 10b dispatch `:55-58`'s equality boundary: two
/// transactions racing at count 4, one wrong and one correct. Both
/// futures are polled together by `tokio::join!` (stage 3's own
/// `exactly_one_of_two_concurrent_claims_wins` is the precedent this
/// mirrors), and the outcome is read off whichever actually won rather
/// than assumed in advance -- F03's own guard shape (one guarded
/// `UPDATE`, not a read then a separate write) means the two calls
/// cannot both see count 4, only one can, but *which* one is exactly
/// the thing `tokio::join!` leaves to the scheduler.
#[tokio::test]
async fn f03_two_concurrent_submissions_at_count_four_yield_exactly_one_winner() {
    let db = fresh_db();
    let f = f03_fixture(&db, 41).await;
    for _ in 1..=4 {
        complete_federated_mfa(
            &db,
            f.pending_id,
            Some(F03Proof::Rejected {
                reason: MfaRejectionReason::Totp,
            }),
            Utc::now(),
        )
        .await
        .expect("seed to count 4");
    }
    assert_eq!(
        row_status(&db, f.pending_id).await,
        crate::models::FederationMfaPendingStatus::Pending
    );

    let now = Utc::now();
    let (wrong, correct) = tokio::join!(
        complete_federated_mfa(
            &db,
            f.pending_id,
            Some(F03Proof::Rejected {
                reason: MfaRejectionReason::Totp,
            }),
            now,
        ),
        complete_federated_mfa(&db, f.pending_id, Some(F03Proof::Totp { step: 5 }), now),
    );

    match (wrong, correct) {
        (Ok(F03Outcome::AttemptsExhausted { .. }), Err(StoreError::Conflict)) => {
            // The wrong proof won the race to the fifth strike: the
            // correct one, arriving after, must not promote.
        }
        (Err(StoreError::Conflict), Ok(F03Outcome::Promoted { .. })) => {
            // The correct proof won first, while count was still 4
            // (0-4 is still eligible to promote); the wrong one,
            // arriving after, has nothing left to count.
        }
        other => panic!(
            "expected exactly one winner (AttemptsExhausted+Conflict or Conflict+Promoted), got {other:?}"
        ),
    }
}

// ── RFC 096-B1 stage 11: F06, and F03's WebAuthn branch ─────────────────
//
// These tests call `begin_federation_webauthn_ceremony` (F06) and
// `complete_federated_mfa` (F03) directly with plain primitives, the
// same discipline the comment above `f03_fixture` already states for
// TOTP/Recovery: there is no verifier here (`sui-id-core`, a downstream
// crate), and no handler either (this stage's own "no handler"
// constraint) -- the only caller exercising this branch end to end
// today is this file.

/// Seeds one passkey credential for `user_id` and returns its row id
/// plus the blob actually stored for it. `create`'s own second argument
/// is the *plaintext* it seals before writing `passkey_enc` -- the
/// fixture's own `UserWebauthnCredentialRow.passkey_enc` field is
/// never what ends up on disk, so the real value has to be read back.
async fn seed_webauthn_credential(
    db: &Database,
    user_id: UserId,
    tag: u8,
) -> (WebauthnCredentialId, Vec<u8>) {
    let row = UserWebauthnCredentialRow {
        id: WebauthnCredentialId::new(),
        user_id,
        credential_id: vec![tag; 16],
        passkey_enc: vec![],
        nickname: "Passkey".to_owned(),
        created_at: Utc::now(),
        last_used_at: None,
    };
    let id = row.id;
    let raw_credential_id = row.credential_id.clone();
    repos::user_webauthn_credentials::create(db, &row, &[0xAA, tag])
        .await
        .expect("seed webauthn credential");
    let sealed = repos::user_webauthn_credentials::find_by_credential_id(db, &raw_credential_id)
        .await
        .expect("read back")
        .expect("credential exists")
        .passkey_enc;
    (id, sealed)
}

async fn ceremony_count(db: &Database) -> i64 {
    db.with_conn(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM federation_webauthn_ceremony",
            [],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

async fn current_passkey_enc(db: &Database, id: WebauthnCredentialId) -> Vec<u8> {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT passkey_enc FROM user_webauthn_credentials WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("read passkey_enc")
}

#[tokio::test]
async fn f06_creates_a_ceremony_bound_to_the_pending_row() {
    let db = fresh_db();
    let f = f03_fixture(&db, 50).await;
    seed_webauthn_credential(&db, f.user_id, 50).await;
    let now = Utc::now();

    let ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"sealed-state".to_vec(),
        "idp.example".to_owned(),
        "https://idp.example".to_owned(),
        now,
    )
    .await
    .expect("F06 succeeds");

    let ceremony = repos::federation_webauthn_ceremony::find_by_pending_id(&db, f.pending_id)
        .await
        .unwrap()
        .expect("ceremony exists");
    assert_eq!(ceremony.id, ceremony_id);
    assert_eq!(ceremony.pending_id, f.pending_id);
    assert_eq!(ceremony.provider_id, f.provider_id);
    assert_eq!(ceremony.user_id, f.user_id);
    assert_eq!(ceremony.rp_id, "idp.example");
    assert_eq!(ceremony.origin, "https://idp.example");
    assert_eq!(ceremony.ceremony_state_sealed, b"sealed-state");
    assert!(
        ceremony.expires_at <= ceremony.pending_expires_at,
        "RFC 096 :794-798: expiry no later than the pending row"
    );
}

/// `contracts/write-commands.toml`'s own F06 row: "atomically replace
/// at most one ... ceremony," through F06 itself rather than the repo
/// function directly (already
/// covered at that layer by
/// `federation_webauthn_ceremony::tests::a_second_insert_for_the_same_pending_id_replaces_the_first`).
#[tokio::test]
async fn f06_replaces_an_existing_ceremony_for_the_same_pending_row() {
    let db = fresh_db();
    let f = f03_fixture(&db, 51).await;
    seed_webauthn_credential(&db, f.user_id, 51).await;
    let now = Utc::now();
    let first = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"first".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    let second = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"second".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    assert_ne!(first, second);
    assert_eq!(ceremony_count(&db).await, 1);
    let ceremony = repos::federation_webauthn_ceremony::find_by_pending_id(&db, f.pending_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ceremony.id, second);
    assert_eq!(ceremony.ceremony_state_sealed, b"second");
}

/// `contracts/write-commands.toml`'s F06 row: "require ... at least one
/// WebAuthn credential enrolled." Schema-enforced invariants (`UNIQUE
/// (pending_id)`, the composite `FOREIGN KEY`, the `expires_at <=
/// pending_expires_at` `CHECK`) are proven at `tests_rfc096_b1_stage8`;
/// this one is F06's own code, with nothing in migration 0047 to lean
/// on -- the table has no column naming "does this user have a
/// passkey" at all.
#[tokio::test]
async fn f06_refuses_without_at_least_one_webauthn_credential() {
    let db = fresh_db();
    let f = f03_fixture(&db, 52).await;
    let result = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(StoreError::Conflict)));
    assert_eq!(ceremony_count(&db).await, 0);
}

#[tokio::test]
async fn f06_refuses_when_the_pending_row_has_expired() {
    let db = fresh_db();
    let f = f03_fixture(&db, 53).await;
    seed_webauthn_credential(&db, f.user_id, 53).await;
    let past_expiry = Utc::now() + TimeDelta::seconds(400);
    let result = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        past_expiry,
    )
    .await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

/// F06 shares `authority_drift_within_tx` with F03 -- this is the one
/// representative check exercised directly through F06's own call
/// site; the other two (link removed, user inactive) are the same
/// shared function F03's own `f03_invalidated_on_disabled_user`/
/// `..._when_the_link_is_removed_without_the_row_cascading` already
/// exercise, and F06 has no `Invalidated` outcome of its own to give a
/// second, F06-flavoured assertion anything new to check.
#[tokio::test]
async fn f06_refuses_when_the_provider_is_disabled() {
    let db = fresh_db();
    let f = f03_fixture(&db, 54).await;
    seed_webauthn_credential(&db, f.user_id, 54).await;
    repos::federation_provider::set_enabled(&db, f.provider_id, false, Utc::now())
        .await
        .unwrap();
    let result = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

#[tokio::test]
async fn f06_refuses_when_the_row_is_no_longer_pending() {
    let db = fresh_db();
    let f = f03_fixture(&db, 55).await;
    seed_webauthn_credential(&db, f.user_id, 55).await;
    complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Totp { step: 5 }),
        Utc::now(),
    )
    .await
    .expect("promote via totp first");
    let result = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        Utc::now(),
    )
    .await;
    assert!(matches!(result, Err(StoreError::Conflict)));
}

/// RFC 094 M2a criterion 2, by hand -- the same reason
/// `f03_injected_commit_failure_rolls_back_everything` is: G20 does not
/// cover Class-P commands.
#[tokio::test]
async fn f06_injected_commit_failure_rolls_back_everything() {
    let db = fresh_db();
    let f = f03_fixture(&db, 56).await;
    seed_webauthn_credential(&db, f.user_id, 56).await;
    db.fail_next_commit_for_test();
    let result = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        Utc::now(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        ceremony_count(&db).await,
        0,
        "the guarded insert must not have committed"
    );

    // Control: the same call, no failure injected, really creates a
    // ceremony -- without this, the assertion above would pass just as
    // well if the injection never ran at all.
    begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        Utc::now(),
    )
    .await
    .expect("the control succeeds");
    assert_eq!(ceremony_count(&db).await, 1);
    record_rollback_coverage("F06");
}

#[tokio::test]
async fn f03_webauthn_promotes_swaps_the_passkey_and_creates_session() {
    let db = fresh_db();
    let f = f03_fixture(&db, 60).await;
    let (credential_id, expected_sealed) = seed_webauthn_credential(&db, f.user_id, 60).await;
    let now = Utc::now();
    let ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"sealed-state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    let new_sealed = vec![0xBB, 60];

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id,
            credential_id,
            expected_sealed: expected_sealed.clone(),
            new_sealed: new_sealed.clone(),
        }),
        now,
    )
    .await
    .expect("promotion succeeds");
    match outcome {
        F03Outcome::Promoted {
            session, method, ..
        } => {
            assert_eq!(
                session.auth_methods,
                vec![
                    sui_id_shared::AuthMethod::Fed,
                    sui_id_shared::AuthMethod::Webauthn
                ]
            );
            assert_eq!(
                session.last_step_up_at,
                Some(now),
                "webauthn is a fresh factor, the same as totp (RFC 102 N5)"
            );
            assert_eq!(method, "webauthn");
        }
        other => panic!("expected Promoted, got {other:?}"),
    }
    assert_eq!(
        current_passkey_enc(&db, credential_id).await,
        new_sealed,
        "the passkey blob was swapped durably"
    );
    assert!(
        repos::federation_webauthn_ceremony::find_by_pending_id(&db, f.pending_id)
            .await
            .unwrap()
            .is_none(),
        "the ceremony is consumed on a successful promotion"
    );
    let link = repos::federation_link::find_by_sub(&db, f.provider_id, "sub-f03-60")
        .await
        .unwrap()
        .expect("link still exists");
    assert!(link.last_seen_at > link.linked_at);
}

/// RFC 096 `:798-801`, quoted in full by the dispatch: "Wrong WebAuthn
/// completion increments the shared failure count and consumes only
/// that failed ceremony; retry requires F06." Both halves in one test:
/// the ceremony is gone but the row survives, and the *same*
/// `failure_count` a wrong TOTP submission would have advanced is what
/// advanced here too.
#[tokio::test]
async fn f03_webauthn_wrong_completion_consumes_only_the_ceremony_and_shares_the_counter() {
    let db = fresh_db();
    let f = f03_fixture(&db, 61).await;
    seed_webauthn_credential(&db, f.user_id, 61).await;
    let now = Utc::now();
    begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"sealed-state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    assert_eq!(ceremony_count(&db).await, 1);

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::Webauthn,
        }),
        now,
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        F03Outcome::RejectedStillPending {
            failure_count,
            reason,
        } => {
            assert_eq!(failure_count, 1);
            assert_eq!(reason, MfaRejectionReason::Webauthn);
        }
        other => panic!("expected RejectedStillPending(Webauthn), got {other:?}"),
    }
    assert_eq!(
        row_status(&db, f.pending_id).await,
        crate::models::FederationMfaPendingStatus::Pending,
        "the row survives a wrong webauthn completion -- retry requires F06"
    );
    assert_eq!(
        ceremony_count(&db).await,
        0,
        "the failed ceremony is consumed (:798-801)"
    );

    let outcome2 = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::Totp,
        }),
        Utc::now(),
    )
    .await
    .expect("second rejection succeeds");
    match outcome2 {
        F03Outcome::RejectedStillPending { failure_count, .. } => assert_eq!(
            failure_count, 2,
            "the TOTP wrong proof advanced the SAME counter the webauthn one just did"
        ),
        other => panic!("expected RejectedStillPending(2), got {other:?}"),
    }
}

/// RFC 096 `:767-768`: exhaustion "deletes bound method ceremonies" --
/// by *any* reason reaching the fifth strike, not only a wrong WebAuthn
/// one. Distinguishes this from the per-attempt WebAuthn-only deletion
/// the test above already proves.
#[tokio::test]
async fn f03_exhaustion_deletes_a_live_ceremony_even_when_the_fifth_wrong_proof_was_totp() {
    let db = fresh_db();
    let f = f03_fixture(&db, 62).await;
    seed_webauthn_credential(&db, f.user_id, 62).await;
    let now = Utc::now();
    begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"sealed-state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    for _ in 1..=4 {
        complete_federated_mfa(
            &db,
            f.pending_id,
            Some(F03Proof::Rejected {
                reason: MfaRejectionReason::Totp,
            }),
            Utc::now(),
        )
        .await
        .expect("seed to count 4");
    }
    assert_eq!(
        ceremony_count(&db).await,
        1,
        "unconsumed by a wrong TOTP proof below exhaustion"
    );
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::Totp,
        }),
        Utc::now(),
    )
    .await
    .expect("the fifth rejection succeeds");
    assert!(matches!(
        outcome,
        F03Outcome::AttemptsExhausted {
            reason: MfaRejectionReason::Totp
        }
    ));
    assert_eq!(
        ceremony_count(&db).await,
        0,
        ":767-768's \"deletes bound method ceremonies\" fires on exhaustion by any reason"
    );
}

/// The dispatch's own `CeremonyInvalid` description, "bound to
/// something other than what was presented": a concurrent F06 call
/// (the user reloaded the WebAuthn prompt) that
/// replaced the ceremony since the stale candidate was built.
#[tokio::test]
async fn f03_webauthn_invalidated_when_the_ceremony_was_replaced_since_the_candidate_was_built() {
    let db = fresh_db();
    let f = f03_fixture(&db, 63).await;
    let (credential_id, expected_sealed) = seed_webauthn_credential(&db, f.user_id, 63).await;
    let now = Utc::now();
    let stale_ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"first".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"second".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id: stale_ceremony_id,
            credential_id,
            expected_sealed,
            new_sealed: vec![0xCC],
        }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => {
            assert_eq!(reason, InvalidatedReason::CeremonyInvalid)
        }
        other => panic!("expected Invalidated(CeremonyInvalid), got {other:?}"),
    }
    assert_eq!(
        ceremony_count(&db).await,
        1,
        "the current (second) ceremony must survive an invalidation triggered by a stale one"
    );
}

#[tokio::test]
async fn f03_webauthn_invalidated_when_no_ceremony_exists_at_all() {
    let db = fresh_db();
    let f = f03_fixture(&db, 64).await;
    let (credential_id, expected_sealed) = seed_webauthn_credential(&db, f.user_id, 64).await;
    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id: FederationWebauthnCeremonyId::new(),
            credential_id,
            expected_sealed,
            new_sealed: vec![0xCC],
        }),
        Utc::now(),
    )
    .await
    .expect("invalidation succeeds");
    assert!(matches!(
        outcome,
        F03Outcome::Invalidated {
            reason: InvalidatedReason::CeremonyInvalid
        }
    ));
}

/// The ceremony's own, narrower expiry (`:794-798`) is nested inside
/// the pending row's own -- this reaches `CeremonyInvalid` while the
/// pending row itself is still well inside its own 300-second budget,
/// distinct from `f03_invalidated_on_expiry`'s own row-level expiry.
#[tokio::test]
async fn f03_webauthn_invalidated_when_the_ceremony_itself_has_expired_but_the_row_has_not() {
    let db = fresh_db();
    let f = f03_fixture(&db, 65).await;
    let (credential_id, expected_sealed) = seed_webauthn_credential(&db, f.user_id, 65).await;
    let t0 = Utc::now();
    let ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        t0,
    )
    .await
    .unwrap();
    let after_ceremony_expiry = t0 + TimeDelta::seconds(61);
    assert!(
        after_ceremony_expiry
            < t0 + TimeDelta::seconds(repos::federation_mfa_pending::PENDING_LIFETIME_SECS),
        "the probe must stay inside the pending row's own window"
    );

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id,
            credential_id,
            expected_sealed,
            new_sealed: vec![0xCC],
        }),
        after_ceremony_expiry,
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => {
            assert_eq!(reason, InvalidatedReason::CeremonyInvalid)
        }
        other => panic!("expected Invalidated(CeremonyInvalid), got {other:?}"),
    }
}

/// The WebAuthn analogue of `f03_invalidated_when_the_factor_is_disabled_after_the_row_was_issued`:
/// the credential the candidate names was deleted entirely in the
/// window between the verifier's own read and F03's transaction.
#[tokio::test]
async fn f03_webauthn_invalidated_when_the_credential_is_deleted_after_the_candidate_was_built() {
    let db = fresh_db();
    let f = f03_fixture(&db, 66).await;
    let (credential_id, expected_sealed) = seed_webauthn_credential(&db, f.user_id, 66).await;
    let now = Utc::now();
    let ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    repos::user_webauthn_credentials::delete(&db, credential_id, f.user_id)
        .await
        .expect("delete credential");

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id,
            credential_id,
            expected_sealed,
            new_sealed: vec![0xCC],
        }),
        now,
    )
    .await
    .expect("invalidation succeeds");
    match outcome {
        F03Outcome::Invalidated { reason } => assert_eq!(reason, InvalidatedReason::FactorChanged),
        other => panic!("expected Invalidated(FactorChanged), got {other:?}"),
    }
}

/// A stale `expected_sealed` (the blob moved since the candidate read
/// it -- e.g. the same physical passkey used in an unrelated
/// authentication) is a wrong proof, the same treatment Recovery's own
/// blob mismatch already gets, decided *before* `complete_within_tx`
/// runs rather than after. It still consumes the ceremony: by RFC 096's
/// own vocabulary this is a `Webauthn`-reasoned rejection like any
/// other, and ":798-801"'s "consumes only that failed ceremony" does
/// not distinguish a cryptographically-wrong assertion from a
/// stale-blob one -- both leave the row pending and require F06 again.
#[tokio::test]
async fn f03_webauthn_stale_expected_blob_is_treated_as_a_wrong_proof() {
    let db = fresh_db();
    let f = f03_fixture(&db, 67).await;
    let (credential_id, _real_sealed) = seed_webauthn_credential(&db, f.user_id, 67).await;
    let now = Utc::now();
    let ceremony_id = begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();
    let wrong_expected = vec![0xDE, 0xAD];

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Webauthn {
            ceremony_id,
            credential_id,
            expected_sealed: wrong_expected,
            new_sealed: vec![0xCC],
        }),
        now,
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        F03Outcome::RejectedStillPending { reason, .. } => {
            assert_eq!(reason, MfaRejectionReason::Webauthn)
        }
        other => panic!("expected RejectedStillPending(Webauthn), got {other:?}"),
    }
    assert_eq!(
        row_status(&db, f.pending_id).await,
        crate::models::FederationMfaPendingStatus::Pending
    );
    assert_eq!(
        ceremony_count(&db).await,
        0,
        "a Webauthn-reasoned rejection consumes the ceremony regardless of why"
    );
}

/// RFC 096 `:802`, direction 1 of `MethodMismatch` -- tested here only
/// as F03's own, already-converted primitive (the verifier, in
/// `sui-id-core`, is what actually selects this reason from "no live
/// ceremony"; this proves F03's own handling of the reason once
/// selected, the same division `f03_wrong_proof_increments_to_one_through_four`
/// already draws for `Totp`).
#[tokio::test]
async fn f03_method_mismatch_is_rejected_without_touching_any_ceremony() {
    let db = fresh_db();
    let f = f03_fixture(&db, 68).await;
    seed_webauthn_credential(&db, f.user_id, 68).await;
    let now = Utc::now();
    begin_federation_webauthn_ceremony(
        &db,
        f.pending_id,
        b"state".to_vec(),
        "idp.example".into(),
        "https://idp.example".into(),
        now,
    )
    .await
    .unwrap();

    let outcome = complete_federated_mfa(
        &db,
        f.pending_id,
        Some(F03Proof::Rejected {
            reason: MfaRejectionReason::MethodMismatch,
        }),
        now,
    )
    .await
    .expect("rejection succeeds");
    match outcome {
        F03Outcome::RejectedStillPending { reason, .. } => {
            assert_eq!(reason, MfaRejectionReason::MethodMismatch)
        }
        other => panic!("expected RejectedStillPending(MethodMismatch), got {other:?}"),
    }
    assert_eq!(
        ceremony_count(&db).await,
        1,
        "a method-mismatch rejection leaves a live webauthn ceremony untouched -- \
         the user can still complete the pending assertion afterward"
    );
}
