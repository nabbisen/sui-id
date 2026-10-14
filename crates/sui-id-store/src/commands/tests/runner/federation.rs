//! RFC 096-B1 stage 7 — F04: first federated provisioning, in one Class-A
//! transaction. RFC 094 M2a's own criteria, applied to the newest
//! command: injected append failures roll back every write (criterion 2),
//! and one successful provisioning writes precisely one audit record
//! (criterion 3) -- plus the three rechecks the manifest itself names:
//! absent link, verified unique email, username uniqueness, each proven
//! by losing the race deliberately.

use super::*;
use crate::models::{FederationLinkRow, FederationProviderRow, ProvisionMode};
use crate::repos::federation_login_attempt;
use sui_id_shared::ids::{
    FederationLoginAttemptId, FederationMfaPendingId, FederationProviderId, SessionId,
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
