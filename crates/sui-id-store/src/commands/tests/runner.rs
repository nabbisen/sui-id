use super::*;
use crate::crypto::MasterKey;
use crate::models::{EmailOutboxRow, EmailOutboxState, SessionRow, UserRow};
use crate::repos;
use crate::{Database, StoreError};
use chrono::{TimeDelta, Utc};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

/// A stand-in for the authorizing admin's `UserId` in tests that
/// call `create_user` — not a real `AdminActor` (this crate
/// can't name that type), just the `UserId` its caller-discipline
/// contract asks for.
fn an_admin() -> UserId {
    UserId::new()
}

fn a_user() -> UserRow {
    UserRow {
        id: UserId::new(),
        username: format!("user-{}", uuid::Uuid::new_v4()),
        display_name: None,
        email: None,
        email_normalized: None,
        email_verified_at: None,
        preferred_lang: None,
        is_admin: false,
        role: crate::models::Role::User,
        is_disabled: false,
        is_deleted: false,
        last_login_at: None,
        user_uuid: uuid::Uuid::new_v4(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        failed_login_count: 0,
        locked_until: None,
        source: crate::models::UserSource::Local,
        external_stable_id: None,
    }
}

fn a_client() -> crate::models::ClientRow {
    crate::models::ClientRow {
        id: ClientId::new(),
        name: format!("client-{}", uuid::Uuid::new_v4()),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: crate::models::ConsentPolicy::default(),
        registered_via: crate::models::RegistrationSource::default(),
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

async fn latest_audit_action(db: &Database) -> Option<String> {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .map(|row| row.action)
}

#[tokio::test]
async fn k01_rotates_key_and_appends_audit_row() {
    let db = fresh_db();
    let new_id = SigningKeyId::new();
    let audited = rotate_signing_key(
        &db,
        new_id,
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
    )
    .await
    .expect("rotate");
    audited.into_inner();

    let active = repos::signing_keys::active(&db).await.expect("active key");
    assert_eq!(active.id, new_id);

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("signing_key.rotate")
    );
}

// ── Stage 2 items 2-3: injected-failure rollback proofs ──────────
//
// The acceptance bar (per the Stage 2 direction, §3): an injected
// failure must roll back *both* the mutation and the audit row,
// verified by observing state before and after -- not by asserting
// that the call returned `Err`. All three tests below do exactly
// that: capture "before" state, inject, assert `Err`, then assert
// "after" state is identical to "before" -- for both the domain
// table and the audit log.

#[tokio::test]
async fn k01_injected_failure_before_append_rolls_back_the_mutation() {
    let db = fresh_db();
    let before_active = repos::signing_keys::active(&db).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = rotate_signing_key(
        &db,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The mutation (key insert/activation) never happened: same
    // "no active key" state as before, not a half-activated new key.
    assert_eq!(
        before_active.err().map(|e| e.to_string()),
        repos::signing_keys::active(&db)
            .await
            .err()
            .map(|e| e.to_string())
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn k01_injected_failure_after_append_rolls_back_the_mutation_and_the_append() {
    let db = fresh_db();
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_after_next_append();
    let result = rotate_signing_key(
        &db,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
    )
    .await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // append_within_tx really ran (it's before this injection
    // point) and really inserted a row -- proving this rolls back
    // requires that the row is gone, not merely that this test
    // never looked for it.
    assert!(
        repos::signing_keys::active(&db).await.is_err(),
        "no active key: the mutation rolled back with the audit row"
    );
    assert_eq!(
        latest_audit_action(&db).await,
        before_audit,
        "the audit row append_within_tx just wrote must not survive \
         an uncommitted transaction"
    );
}

#[tokio::test]
async fn k01_injected_commit_failure_rolls_back_the_mutation_and_the_append() {
    // Distinct from the two tests above: the closure itself returns
    // `Ok` (mutation succeeded, event built, append succeeded) --
    // this proves that a rejection at SQLite's own commit boundary,
    // which no domain code observes or controls, still prevents
    // `Audited<T>` from ever being constructed and still leaves no
    // trace in either table.
    let db = fresh_db();
    let before_audit = latest_audit_action(&db).await;

    db.fail_next_commit_for_test();
    let result = rotate_signing_key(
        &db,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
    )
    .await;
    assert!(
        result.is_err(),
        "a rejected commit must surface as Err, not a silently-empty Ok"
    );

    assert!(
        repos::signing_keys::active(&db).await.is_err(),
        "no active key: a rejected commit persists nothing"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);

    // The hook self-disarmed after firing once: a real rotation now
    // succeeds normally, proving this test didn't leave the
    // connection unable to commit anything ever again.
    let audited = rotate_signing_key(
        &db,
        SigningKeyId::new(),
        "ed25519".into(),
        b"sealed-placeholder".to_vec(),
        b"public-key-placeholder".to_vec(),
    )
    .await
    .expect("rotate after the injected commit failure has cleared");
    audited.into_inner();
}

#[tokio::test]
async fn u22_below_threshold_emits_failure_not_lockout() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let audited = record_login_failure(&db, user.id, |_count| None)
        .await
        .expect("record failure");
    assert_eq!(audited.into_inner(), 1);
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.login.failure")
    );

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.failed_login_count, 1);
    assert!(row.locked_until.is_none());
}

#[tokio::test]
async fn u22_crossing_threshold_emits_lockout_and_sets_locked_until() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    // Threshold of 1: the very first failure crosses it.
    let audited = record_login_failure(&db, user.id, |count| {
        (count >= 1).then_some(TimeDelta::seconds(30))
    })
    .await
    .expect("record failure");
    assert_eq!(audited.into_inner(), 1);
    let latest = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("one row");
    assert_eq!(latest.action, "auth.lockout");
    // The lock window's length must survive into the audit note —
    // this is the detail the two-call, non-atomic predecessor put
    // in a second, separately-appended row; U22 carries it as an
    // attribute on the one row it appends instead.
    let note = latest.note.expect("lockout row must carry a note");
    assert!(
        note.contains("locked_for_secs=30"),
        "note must record the lock window length: {note:?}"
    );

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(
        row.locked_until.is_some(),
        "locked_until must be set on the crossing transaction"
    );
}

#[tokio::test]
async fn u22_lockout_and_counter_update_are_the_same_transaction() {
    // Regression proof for the thing this command replaces: the
    // current authn::session code makes two separate calls (bump,
    // then a second best-effort call to stamp the lock), so a
    // crash between them leaves the counter bumped but no lock.
    // Here there is only one call and one transaction; there is no
    // window where the counter is bumped but the lock (when owed)
    // is not yet set, because both writes and the audit append
    // share one `with_tx`.
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    record_login_failure(&db, user.id, |count| {
        (count >= 1).then_some(TimeDelta::seconds(60))
    })
    .await
    .expect("record failure");

    let row = repos::users::get(&db, user.id).await.expect("get");
    // If the counter and lock could ever be observed apart, this
    // assertion is what would eventually catch it under a
    // concurrency/fault-injection harness -- for now this proves
    // the shape: one call produced both effects. The fault
    // injector now exists (see the k01_injected_* tests above);
    // genuine concurrent execution is proven separately below.
    assert_eq!(row.failed_login_count, 1);
    assert!(row.locked_until.is_some());
}

#[tokio::test]
async fn concurrent_class_a_commands_maintain_one_unbroken_chain() {
    // RFC 094 Stage 2: "prove the audit chain is read and written
    // on the caller transaction." `repos::audit`'s own tests
    // already prove `append_within_tx` behaves correctly called
    // directly and in isolation; this proves the same property
    // through the real Class-A/registry path, under genuinely
    // concurrent execution rather than by reading the
    // mutex-serialization argument in its doc comment. If the
    // chain-head read and the row insert were ever split across
    // two separate transactions (a regression this test would
    // catch), concurrent commands could compute the same
    // `prev_hash` and fork the chain, or `verify_chain_tail` would
    // report a break.
    let db = fresh_db();
    const N: usize = 20;
    let mut user_ids = Vec::with_capacity(N);
    for _ in 0..N {
        let user = a_user();
        repos::users::create(&db, &user).await.expect("create user");
        user_ids.push(user.id);
    }

    let handles: Vec<_> = user_ids
        .into_iter()
        .map(|user_id| {
            let db = db.clone();
            tokio::spawn(async move {
                record_login_failure(&db, user_id, |_count| None)
                    .await
                    .expect("record failure")
            })
        })
        .collect();
    for handle in handles {
        handle.await.expect("task join");
    }

    let report = repos::audit::verify_chain_tail(&db, (N * 2) as i64)
        .await
        .expect("verify chain");
    assert_eq!(
        report.checked, N,
        "exactly one audit row per concurrent command, no lost or duplicated rows"
    );
    assert!(
        report.broken_at_seq.is_none(),
        "no fork: every row's prev_hash must chain from exactly one predecessor, \
         even though the audit-row writes raced"
    );
}

#[tokio::test]
async fn u01_normal_branch_creates_user_and_credential() {
    let db = fresh_db();
    let user = a_user();
    let cred = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "argon2-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };

    let audited = create_user(&db, an_admin(), user.clone(), Some(cred), false)
        .await
        .expect("create");
    audited.into_inner();

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.id, user.id);
    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("user.create")
    );
}

#[tokio::test]
async fn u01_hibp_branch_emits_the_warned_event_name() {
    let db = fresh_db();
    let user = a_user();

    create_user(&db, an_admin(), user.clone(), None, true)
        .await
        .expect("create");

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("user.create_warned_hibp")
    );
}

#[tokio::test]
async fn u01_rolls_back_credential_and_user_together_on_conflict() {
    // A real SQL-level failure (duplicate primary key), distinct
    // from -- and weaker than -- the two injected-failure tests
    // right below: SQLite's default `ABORT` conflict resolution
    // means the one failed statement here writes nothing on its
    // own, so "nothing new persisted" would hold even if `class_a`
    // never rolled back anything. Kept for the real-SQL-failure
    // case it does cover; the injected tests are what actually
    // exercise `class_a`'s own rollback behavior.
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user)
        .await
        .expect("first create");

    let before = latest_audit_action(&db).await;

    let mut dup = a_user();
    dup.id = user.id; // force the conflict
    let result = create_user(&db, an_admin(), dup, None, false).await;
    assert!(matches!(result, Err(StoreError::Conflict)));

    // No new audit row from the failed attempt.
    assert_eq!(latest_audit_action(&db).await, before);
}

#[tokio::test]
async fn u01_injected_failure_before_append_rolls_back_the_user_insert() {
    let db = fresh_db();
    let user = a_user();
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = create_user(&db, an_admin(), user.clone(), None, false).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The user insert really ran (it's before this injection
    // point); proving rollback requires it to be gone, not merely
    // that this test never looked for it.
    assert!(
        repos::users::get(&db, user.id).await.is_err(),
        "no user row: the insert rolled back"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u01_injected_failure_after_append_rolls_back_the_user_insert_and_the_append() {
    let db = fresh_db();
    let user = a_user();
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_after_next_append();
    let result = create_user(&db, an_admin(), user.clone(), None, false).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::users::get(&db, user.id).await.is_err(),
        "no user row: the insert rolled back with the audit row"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U02-U05 — user administration wave (disable/enable/delete/role) ─

async fn seed_active_session(db: &Database, user_id: UserId) -> sui_id_shared::ids::SessionId {
    let id = sui_id_shared::ids::SessionId::new();
    repos::sessions::insert(
        db,
        &SessionRow {
            id,
            user_id,
            expires_at: Utc::now() + TimeDelta::hours(1),
            created_at: Utc::now(),
            revoked_at: None,
            auth_methods: vec![],
            last_step_up_at: None,
            last_used_at: None,
        },
    )
    .await
    .expect("seed session");
    id
}

#[tokio::test]
async fn u02_disable_flips_flag_revokes_session_and_records_reason() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;

    let audited = disable_user(
        &db,
        an_admin(),
        user.id,
        Some("policy violation".to_string()),
    )
    .await
    .expect("disable");
    audited.into_inner();

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(row.is_disabled);

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_some(),
        "the target's session must be revoked in the same transaction as the disable"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.disable");
    assert_eq!(tail.note.as_deref(), Some("reason=policy violation"));
}

#[tokio::test]
async fn u02_disable_without_reason_records_no_note() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    disable_user(&db, an_admin(), user.id, None)
        .await
        .expect("disable");

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.note, None);
}

#[tokio::test]
async fn u02_injected_failure_before_append_rolls_back_disable_and_session_revoke() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = disable_user(&db, an_admin(), user.id, None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(!row.is_disabled, "the flag flip rolled back");
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the session revoke rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u03_enable_clears_disabled_flag_and_appends_event() {
    let db = fresh_db();
    let mut user = a_user();
    user.is_disabled = true;
    repos::users::create(&db, &user).await.expect("create user");

    let audited = enable_user(&db, an_admin(), user.id).await.expect("enable");
    audited.into_inner();

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(!row.is_disabled);

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.enable");
    assert_eq!(tail.note, None);
}

#[tokio::test]
async fn u04_delete_soft_deletes_revokes_session_and_records_reason() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;

    let audited = delete_user(&db, an_admin(), user.id, Some("gdpr request".to_string()))
        .await
        .expect("delete");
    audited.into_inner();

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(row.is_deleted);
    assert!(row.is_disabled);

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(session.revoked_at.is_some());

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.delete");
    assert_eq!(tail.note.as_deref(), Some("reason=gdpr request"));
}

#[tokio::test]
async fn u04_injected_failure_before_append_rolls_back_delete_and_session_revoke() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = delete_user(&db, an_admin(), user.id, None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert!(!row.is_deleted, "the soft-delete rolled back");
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the session revoke rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u05_role_change_updates_role_and_records_old_and_new_role() {
    let db = fresh_db();
    let mut admin_a = a_user();
    admin_a.role = crate::models::Role::Admin;
    admin_a.is_admin = true;
    let mut admin_b = a_user();
    admin_b.role = crate::models::Role::Admin;
    admin_b.is_admin = true;
    repos::users::create(&db, &admin_a).await.expect("create a");
    repos::users::create(&db, &admin_b).await.expect("create b");

    // Two admins exist, so demoting one leaves one behind — the
    // guard must not fire.
    let audited = change_user_role(&db, an_admin(), admin_a.id, crate::models::Role::User)
        .await
        .expect("role change");
    audited.into_inner();

    let row = repos::users::get(&db, admin_a.id).await.expect("get");
    assert_eq!(row.role, crate::models::Role::User);

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.role_change");
    assert_eq!(tail.note.as_deref(), Some("old_role=admin new_role=user"));
}

#[tokio::test]
async fn u05_last_admin_guard_blocks_demotion_and_makes_no_change() {
    let db = fresh_db();
    let mut only_admin = a_user();
    only_admin.role = crate::models::Role::Admin;
    only_admin.is_admin = true;
    repos::users::create(&db, &only_admin)
        .await
        .expect("create admin");
    let before_audit = latest_audit_action(&db).await;

    let result = change_user_role(&db, an_admin(), only_admin.id, crate::models::Role::User).await;
    assert!(
        matches!(result, Err(StoreError::Conflict)),
        "demoting the last admin must be rejected"
    );

    let row = repos::users::get(&db, only_admin.id).await.expect("get");
    assert_eq!(
        row.role,
        crate::models::Role::Admin,
        "the guard fired before the mutation, not after"
    );
    assert_eq!(
        latest_audit_action(&db).await,
        before_audit,
        "a rejected guard must not emit an audit row"
    );
}

#[tokio::test]
async fn u05_promoting_a_user_is_never_blocked_by_the_last_admin_guard() {
    let db = fresh_db();
    let mut only_admin = a_user();
    only_admin.role = crate::models::Role::Admin;
    only_admin.is_admin = true;
    let candidate = a_user();
    repos::users::create(&db, &only_admin)
        .await
        .expect("create admin");
    repos::users::create(&db, &candidate)
        .await
        .expect("create candidate");

    // Promotion never reduces the admin count, so the guard (which
    // only fires when `old_role.is_admin() && !new_role.is_admin()`)
    // must not apply here even though only one admin exists.
    change_user_role(&db, an_admin(), candidate.id, crate::models::Role::Admin)
        .await
        .expect("promotion must not be blocked");

    let row = repos::users::get(&db, candidate.id).await.expect("get");
    assert_eq!(row.role, crate::models::Role::Admin);
}

/// The property `u05_last_admin_guard_blocks_demotion_and_makes_no_
/// change` does not cover: that test demotes the *only* admin, which
/// the HTTP handler's pre-transaction `count_admins` check already
/// catches on its own — it never exercises the guard's actual reason
/// for existing inside the transaction, the race between two
/// concurrent demotions that both observe "2 admins left" before
/// either commits. Reviewer finding, 2026-09-08 (`.git-exclude/
/// reviewed/094-wave-a-user-admin-u01-u05-2026-09-08.md` §2):
/// mutating away the guard's `if` block still failed the
/// single-admin test, so that test could not distinguish "the guard
/// works" from "the guard is needed." Measured directly (temporary
/// probe, since reverted) before writing this: two admins, two
/// concurrent demotions, `successes=1 admins_remaining=1` — the race
/// really is closed by `count_admins_within_tx`/`get_role_within_tx`
/// reading from the same transaction that performs the demotion.
/// Same shape as `concurrent_class_a_commands_maintain_one_unbroken_
/// chain` above: real `tokio::spawn` concurrency through the real
/// `class_a` path, not an argument about mutex serialization.
#[tokio::test]
async fn u05_concurrent_demotions_of_the_last_two_admins_leave_exactly_one() {
    let db = fresh_db();
    let mut admin_a = a_user();
    admin_a.role = crate::models::Role::Admin;
    admin_a.is_admin = true;
    let mut admin_b = a_user();
    admin_b.role = crate::models::Role::Admin;
    admin_b.is_admin = true;
    repos::users::create(&db, &admin_a).await.expect("create a");
    repos::users::create(&db, &admin_b).await.expect("create b");

    let handles: Vec<_> = [admin_a.id, admin_b.id]
        .into_iter()
        .map(|target| {
            let db = db.clone();
            tokio::spawn(async move {
                change_user_role(&db, an_admin(), target, crate::models::Role::User).await
            })
        })
        .collect();

    let mut successes = 0;
    let mut conflicts = 0;
    for handle in handles {
        match handle.await.expect("task join") {
            Ok(audited) => {
                audited.into_inner();
                successes += 1;
            }
            Err(StoreError::Conflict) => conflicts += 1,
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    assert_eq!(
        successes, 1,
        "exactly one demotion must win the race, not zero and not both"
    );
    assert_eq!(
        conflicts, 1,
        "the loser must see the guard, not a silent success"
    );
    assert_eq!(
        repos::users::count_admins(&db).await.expect("count"),
        1,
        "the last admin must never be reachable via a race the pre-check missed"
    );
}

// ── U06 — admin password reset ───────────────────────────────────

#[tokio::test]
async fn u06_reset_swaps_credential_revokes_session_and_appends_event() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = reset_user_password(&db, an_admin(), user.id, new_credential)
        .await
        .expect("reset");
    audited.into_inner();

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_some(),
        "the target's session must be revoked in the same transaction as the reset"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "user.reset_password");
}

#[tokio::test]
async fn u06_injected_failure_before_append_rolls_back_credential_and_session_revoke() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result = reset_user_password(&db, an_admin(), user.id, new_credential).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the session revoke rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U07 — admin MFA reset ──────────────────────────────────────

async fn seed_passkey(db: &Database, user_id: UserId) {
    repos::user_webauthn_credentials::create(
        db,
        &crate::models::UserWebauthnCredentialRow {
            id: sui_id_shared::ids::WebauthnCredentialId::new(),
            user_id,
            credential_id: format!("cred-{}", uuid::Uuid::new_v4()).into_bytes(),
            passkey_enc: vec![],
            nickname: "test passkey".into(),
            created_at: Utc::now(),
            last_used_at: None,
        },
        b"passkey-json-placeholder",
    )
    .await
    .expect("seed passkey");
}

#[tokio::test]
async fn u07_reset_removes_totp_and_passkeys_and_appends_event() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    seed_passkey(&db, user.id).await;
    seed_passkey(&db, user.id).await;

    let audited = admin_reset_mfa(
        &db,
        an_admin(),
        user.id,
        Some("lost authenticator".to_string()),
    )
    .await
    .expect("reset");
    let (totp_removed, passkeys_removed) = audited.into_inner();
    assert!(totp_removed);
    assert_eq!(passkeys_removed, 2);

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_none(),
        "TOTP enrollment must be gone"
    );
    assert!(
        repos::user_webauthn_credentials::list_for_user(&db, user.id)
            .await
            .expect("list passkeys")
            .is_empty(),
        "every passkey must be gone"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "mfa.admin_reset");
    assert_eq!(
        tail.note.as_deref(),
        Some("totp=removed passkeys=2 reason=lost authenticator")
    );
}

#[tokio::test]
async fn u07_reset_with_no_factors_reports_absent_and_zero() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let audited = admin_reset_mfa(&db, an_admin(), user.id, None)
        .await
        .expect("reset");
    let (totp_removed, passkeys_removed) = audited.into_inner();
    assert!(!totp_removed);
    assert_eq!(passkeys_removed, 0);

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.note.as_deref(), Some("totp=absent passkeys=0"));
}

#[tokio::test]
async fn u07_reset_of_nonexistent_user_returns_not_found_and_appends_nothing() {
    let db = fresh_db();
    let before_audit = latest_audit_action(&db).await;

    let result = admin_reset_mfa(&db, an_admin(), UserId::new(), None).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "the existence probe (get_role_within_tx) must reject a target that \
         was never created, not silently succeed with nothing to remove"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u07_reset_of_soft_deleted_user_returns_not_found() {
    // Reviewer finding, 2026-09-09 (`.git-exclude/reviewed/
    // 094-wave-b-u07-2026-09-09.md` §2): `get_role_within_tx`
    // (reused here as an existence probe) filters `is_deleted = 0`;
    // `users::get`, the pre-conversion check it replaced, did not.
    // A reset against a soft-deleted user used to succeed; it must
    // now be rejected, deliberately, not as an unstated side effect
    // of borrowing a helper for its query shape.
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    repos::users::soft_delete(&db, user.id)
        .await
        .expect("soft delete");
    let before_audit = latest_audit_action(&db).await;

    let result = admin_reset_mfa(&db, an_admin(), user.id, None).await;
    assert!(
        matches!(result, Err(StoreError::NotFound)),
        "a soft-deleted target must be rejected, not silently reset"
    );

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_some(),
        "the TOTP row must be untouched -- rejection happens before any mutation"
    );
    assert_eq!(
        latest_audit_action(&db).await,
        before_audit,
        "a rejected reset must not emit an audit row"
    );
}

#[tokio::test]
async fn u07_injected_failure_before_append_rolls_back_totp_and_passkey_removal() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::user_totp::upsert_pending(&db, user.id, b"totp-secret-placeholder")
        .await
        .expect("seed totp");
    seed_passkey(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = admin_reset_mfa(&db, an_admin(), user.id, None).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    assert!(
        repos::user_totp::get(&db, user.id)
            .await
            .expect("get totp")
            .is_some(),
        "the TOTP delete rolled back"
    );
    assert_eq!(
        repos::user_webauthn_credentials::list_for_user(&db, user.id)
            .await
            .expect("list passkeys")
            .len(),
        1,
        "the passkey delete rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U08 — CLI operator unlock ──────────────────────────────────

#[tokio::test]
async fn u08_unlock_clears_lockout_and_appends_actorless_event() {
    let db = fresh_db();
    let mut user = a_user();
    user.failed_login_count = 5;
    user.locked_until = Some(Utc::now() + TimeDelta::hours(1));
    repos::users::create(&db, &user).await.expect("create user");

    let audited = admin_unlock_user(&db, user.id).await.expect("unlock");
    audited.into_inner();

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.failed_login_count, 0);
    assert!(row.locked_until.is_none());

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "admin.user.unlock");
    assert_eq!(
        tail.actor, None,
        "the CLI operator authenticates no user; the row must carry no actor"
    );
    assert_eq!(tail.target.as_deref(), Some(user.id.to_string().as_str()));
}

#[tokio::test]
async fn u08_unlock_of_nonexistent_user_returns_not_found() {
    let db = fresh_db();
    let before_audit = latest_audit_action(&db).await;

    let result = admin_unlock_user(&db, UserId::new()).await;
    assert!(matches!(result, Err(StoreError::NotFound)));
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u08_injected_failure_before_append_rolls_back_the_unlock() {
    let db = fresh_db();
    let mut user = a_user();
    user.failed_login_count = 5;
    user.locked_until = Some(Utc::now() + TimeDelta::hours(1));
    repos::users::create(&db, &user).await.expect("create user");
    let before_audit = latest_audit_action(&db).await;

    db.fault_injector().fail_before_next_append();
    let result = admin_unlock_user(&db, user.id).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let row = repos::users::get(&db, user.id).await.expect("get");
    assert_eq!(row.failed_login_count, 5, "the counter reset rolled back");
    assert!(row.locked_until.is_some(), "the lock clear rolled back too");
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U09 — self password change ─────────────────────────────────

#[tokio::test]
async fn u09_change_with_sweep_revokes_others_keeps_current_and_appends_counts() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");

    let keep_id = seed_active_session(&db, user.id).await;
    let other_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = change_password_self(&db, user.id, new_credential, Some(keep_id), true)
        .await
        .expect("change password");
    let (sessions_revoked, refresh_tokens_revoked) = audited.into_inner();
    assert_eq!(sessions_revoked, 1);
    assert_eq!(refresh_tokens_revoked, 0);

    let kept = repos::sessions::get(&db, keep_id).await.expect("get kept");
    assert!(
        kept.revoked_at.is_none(),
        "the current session must survive"
    );
    let other = repos::sessions::get(&db, other_id)
        .await
        .expect("get other");
    assert!(
        other.revoked_at.is_some(),
        "every other session must be revoked"
    );

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.password.changed_self");
    assert_eq!(
        tail.note.as_deref(),
        Some("sessions_revoked=1 refresh_tokens_revoked=0")
    );
}

#[tokio::test]
async fn u09_change_without_sweep_revokes_nothing_and_reports_zero() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = change_password_self(&db, user.id, new_credential, None, false)
        .await
        .expect("change password");
    let (sessions_revoked, refresh_tokens_revoked) = audited.into_inner();
    assert_eq!(sessions_revoked, 0);
    assert_eq!(refresh_tokens_revoked, 0);

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "no sweep was requested; nothing should be revoked"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(
        tail.note.as_deref(),
        Some("sessions_revoked=0 refresh_tokens_revoked=0")
    );
}

#[tokio::test]
async fn u09_injected_failure_before_append_rolls_back_credential_and_revocations() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result = change_password_self(&db, user.id, new_credential, None, true).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(session.revoked_at.is_none(), "the sweep rolled back too");
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── U10 — forgot-password completion ───────────────────────────

async fn seed_reset_token(
    db: &Database,
    user_id: UserId,
) -> sui_id_shared::ids::PasswordResetTokenId {
    let id = sui_id_shared::ids::PasswordResetTokenId::new();
    repos::password_reset_tokens::insert(
        db,
        &crate::models::PasswordResetTokenRow {
            id,
            user_id,
            token_hash: b"token-hash-placeholder".to_vec(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + TimeDelta::hours(1),
            consumed_at: None,
            requester_ip: None,
        },
    )
    .await
    .expect("seed reset token");
    id
}

#[tokio::test]
async fn u10_completion_swaps_credential_consumes_token_and_revokes_everything() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let token_id = seed_reset_token(&db, user.id).await;
    let session_id = seed_active_session(&db, user.id).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    let consumed_at = Utc::now();
    let audited = consume_and_reset_password(&db, user.id, token_id, new_credential, consumed_at)
        .await
        .expect("complete reset");
    audited.into_inner();

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(cred.password_hash, "new-hash-placeholder");

    let token = repos::password_reset_tokens::find_by_hash(&db, b"token-hash-placeholder")
        .await
        .expect("find token")
        .expect("token still exists");
    assert!(token.consumed_at.is_some(), "the token must be consumed");

    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_some(),
        "forgot-password completion revokes every session, unlike self-change"
    );

    let tail = repos::audit::recent(&db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(tail.action, "auth.password.reset_completed");
    assert_eq!(
        tail.actor, None,
        "the token presenter is not an authenticated actor"
    );
    assert_eq!(tail.target.as_deref(), Some(user.id.to_string().as_str()));
}

#[tokio::test]
async fn u10_injected_failure_before_append_rolls_back_everything() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");
    repos::credentials::upsert(
        &db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            must_change: false,
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed old credential");
    let token_id = seed_reset_token(&db, user.id).await;
    let session_id = seed_active_session(&db, user.id).await;
    let before_audit = latest_audit_action(&db).await;

    let new_credential = crate::models::CredentialRow {
        user_id: user.id,
        password_hash: "new-hash-placeholder".into(),
        must_change: false,
        updated_at: Utc::now(),
    };
    db.fault_injector().fail_before_next_append();
    let result =
        consume_and_reset_password(&db, user.id, token_id, new_credential, Utc::now()).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    let cred = repos::credentials::get(&db, user.id)
        .await
        .expect("get credential");
    assert_eq!(
        cred.password_hash, "old-hash-placeholder",
        "the credential swap rolled back"
    );
    let token = repos::password_reset_tokens::find_by_hash(&db, b"token-hash-placeholder")
        .await
        .expect("find token")
        .expect("token still exists");
    assert!(token.consumed_at.is_none(), "the token consume rolled back");
    let session = repos::sessions::get(&db, session_id)
        .await
        .expect("get session");
    assert!(
        session.revoked_at.is_none(),
        "the revocation rolled back too"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

// ── T04/T09 — refresh-token rotation and initial issuance ───────

async fn seed_family(
    db: &Database,
) -> (UserId, ClientId, FamilyId, sui_id_shared::RefreshTokenHash) {
    let user = a_user();
    repos::users::create(db, &user).await.expect("create user");
    let client = a_client();
    repos::clients::create(db, &client)
        .await
        .expect("create client");

    let root_id = sui_id_shared::RefreshTokenId::generate();
    let family = FamilyId::root_of(&root_id);
    let row = crate::models::RefreshTokenRow {
        id: root_id,
        user_id: user.id,
        client_id: client.id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: family.clone(),
    };
    let prepared =
        repos::refresh_tokens::prepare_refresh_token(db.key(), row).expect("prepare root token");
    let hash = sui_id_shared::RefreshTokenHash::of(&prepared.raw_token);
    insert_initial_refresh_token(db, prepared)
        .await
        .expect("t09 initial issue");
    (user.id, client.id, family, hash)
}

fn a_successor(
    user_id: UserId,
    client_id: ClientId,
    family: FamilyId,
) -> crate::models::RefreshTokenRow {
    crate::models::RefreshTokenRow {
        id: sui_id_shared::RefreshTokenId::generate(),
        user_id,
        client_id,
        scope: "openid".into(),
        expires_at: Utc::now() + TimeDelta::hours(1),
        revoked_at: None,
        created_at: Utc::now(),
        auth_methods: vec![],
        family_id: family,
    }
}

#[tokio::test]
async fn t09_protocol_issues_initial_token_with_no_audit_row() {
    let db = fresh_db();
    let before = latest_audit_action(&db).await;
    let (_, _, _, hash) = seed_family(&db).await;

    // The row genuinely exists (T09 really wrote it) ...
    assert!(
        repos::refresh_tokens::begin_rotation(&db, &hash, &ClientId::new(), Utc::now())
            .await
            .is_err(),
        "sanity: wrong client must reject without revoking"
    );
    // ... but no audit row exists for it -- T09 has no path to
    // Audited<T>, by construction (Database::protocol).
    assert_eq!(latest_audit_action(&db).await, before);
}

#[tokio::test]
async fn t04_normal_rotation_revokes_old_inserts_successor_and_appends_rotated() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;

    let successor = a_successor(user_id, client_id, family.clone());
    let successor_id = successor.id.clone();
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), prepared)
        .await
        .expect("rotate");
    match audited.into_inner() {
        T04Outcome::Rotated { successor, .. } => {
            assert_eq!(successor.id, successor_id);
        }
        T04Outcome::TheftDetected { .. } => panic!("expected Rotated"),
    }

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.refresh.rotated")
    );
}

#[tokio::test]
async fn t04_reuse_revokes_family_and_appends_theft_detected() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;

    // First rotation: legitimate, wins.
    let s1 = a_successor(user_id, client_id, family.clone());
    let p1 = repos::refresh_tokens::prepare_refresh_token(db.key(), s1).unwrap();
    rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), p1)
        .await
        .expect("first rotation");

    // Replay of the now-revoked root token: reuse.
    let s2 = a_successor(user_id, client_id, family.clone());
    let p2 = repos::refresh_tokens::prepare_refresh_token(db.key(), s2).unwrap();
    let audited = rotate_refresh_token(&db, hash, client_id, Utc::now(), p2)
        .await
        .expect("replay");
    match audited.into_inner() {
        T04Outcome::TheftDetected { family_revoked } => {
            assert!(family_revoked >= 1, "at least the winner's successor");
        }
        T04Outcome::Rotated { .. } => panic!("expected TheftDetected"),
    }

    assert_eq!(
        latest_audit_action(&db).await.as_deref(),
        Some("auth.refresh.theft_detected")
    );

    let active: i64 = db
        .with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT COUNT(*) FROM refresh_tokens \
                     WHERE family_id = ?1 AND revoked_at IS NULL",
                    [family.as_str()],
                    |r| r.get(0),
                )
                .expect("count"))
        })
        .expect("query");
    assert_eq!(active, 0, "reuse must close the whole family");
}

#[tokio::test]
async fn t04_injected_failure_before_append_rolls_back_revoke_and_successor_insert() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;
    let before_audit = latest_audit_action(&db).await;

    let successor = a_successor(user_id, client_id, family);
    let successor_id = successor.id.clone();
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    db.fault_injector().fail_before_next_append();
    let result = rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
    assert!(result.is_err(), "injected failure must surface as Err");

    // The old row must still be active -- the guarded revoke rolled
    // back with everything else in this transaction.
    assert!(
        matches!(
            repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                .await
                .expect("old token must still be rotatable"),
            repos::refresh_tokens::RotationLookup::RotatedHere(_)
        ),
        "old row must still be active: the revoke rolled back"
    );
    assert!(
        db.with_conn_sync(|conn| {
            Ok(conn
                .query_row(
                    "SELECT 1 FROM refresh_tokens WHERE id = ?1",
                    [successor_id.as_str()],
                    |r| r.get::<_, i64>(0),
                )
                .is_err())
        })
        .unwrap(),
        "successor must not have been inserted"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn t04_injected_commit_failure_rolls_back_everything() {
    let db = fresh_db();
    let (user_id, client_id, family, hash) = seed_family(&db).await;
    let before_audit = latest_audit_action(&db).await;

    let successor = a_successor(user_id, client_id, family);
    let prepared = repos::refresh_tokens::prepare_refresh_token(db.key(), successor).unwrap();

    db.fail_next_commit_for_test();
    let result = rotate_refresh_token(&db, hash.clone(), client_id, Utc::now(), prepared).await;
    assert!(
        result.is_err(),
        "a rejected commit must surface as Err, not a silently-empty Ok"
    );

    assert!(
        matches!(
            repos::refresh_tokens::begin_rotation(&db, &hash, &client_id, Utc::now())
                .await
                .expect("old token must still be rotatable"),
            repos::refresh_tokens::RotationLookup::RotatedHere(_)
        ),
        "old row must still be active after a rejected commit"
    );
    assert_eq!(latest_audit_action(&db).await, before_audit);
}

#[tokio::test]
async fn u30_protocol_inserts_session_with_no_audit_row() {
    let db = fresh_db();
    let user = a_user();
    repos::users::create(&db, &user).await.expect("create user");

    let before = latest_audit_action(&db).await;

    let session = SessionRow {
        id: sui_id_shared::ids::SessionId::new(),
        user_id: user.id,
        expires_at: Utc::now() + TimeDelta::hours(1),
        created_at: Utc::now(),
        revoked_at: None,
        auth_methods: vec![],
        last_step_up_at: None,
        last_used_at: Some(Utc::now()),
    };
    insert_session(&db, session.clone())
        .await
        .expect("insert session");

    let fetched = repos::sessions::get(&db, session.id)
        .await
        .expect("get session");
    assert_eq!(fetched.id, session.id);

    // Protocol commands are not the tamper-evident chain -- no new
    // audit row, by construction (there is no code path from
    // `Database::protocol` to `audit::append_within_tx`).
    assert_eq!(latest_audit_action(&db).await, before);
}

#[tokio::test]
async fn o01_operational_enqueues_email_with_no_audit_row() {
    let db = fresh_db();
    let before = latest_audit_action(&db).await;

    let row = EmailOutboxRow {
        id: sui_id_shared::ids::EmailOutboxId::new(),
        state: EmailOutboxState::Queued,
        template: "forgot_password".into(),
        recipient_enc: vec![1, 2, 3],
        payload_enc: vec![4, 5, 6],
        attempt_count: 0,
        next_attempt_at: Utc::now(),
        last_error: None,
        locale: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    enqueue_email(&db, row).await.expect("enqueue");

    assert_eq!(latest_audit_action(&db).await, before);
}
