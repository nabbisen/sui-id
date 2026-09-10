use super::*;

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
