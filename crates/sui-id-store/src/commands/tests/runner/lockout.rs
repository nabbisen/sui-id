use super::*;

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

/// RFC 102 stage 9c: after `auth.mfa.lockout` the second-factor count is at
/// the threshold; an operator's unlock must clear it with the lock.
async fn seed_mfa_locked_user(db: &Database) -> UserId {
    let mut user = a_user();
    user.locked_until = Some(Utc::now() + TimeDelta::hours(48));
    repos::users::create(db, &user).await.expect("create user");
    let id = user.id;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE users SET mfa_failure_count = 5 WHERE id = ?1",
            [id.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("seed count");
    id
}

async fn mfa_failure_count(db: &Database, id: UserId) -> i64 {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT mfa_failure_count FROM users WHERE id = ?1",
            [id.to_string()],
            |r| r.get(0),
        )?)
    })
    .await
    .expect("count")
}

#[tokio::test]
async fn u08_unlock_also_clears_the_second_factor_count() {
    let db = fresh_db();
    let id = seed_mfa_locked_user(&db).await;
    assert_eq!(mfa_failure_count(&db, id).await, 5);

    admin_unlock_user(&db, id).await.expect("unlock");

    assert_eq!(mfa_failure_count(&db, id).await, 0, "second-factor count");
    let row = repos::users::get(&db, id).await.expect("get");
    assert!(row.locked_until.is_none());
    assert_eq!(row.failed_login_count, 0);
}

#[tokio::test]
async fn the_unguarded_admin_unlock_clears_the_second_factor_count_too() {
    // `users::admin_unlock` is not U08's path but is the same statement in
    // another form; the two must not drift.
    let db = fresh_db();
    let id = seed_mfa_locked_user(&db).await;
    repos::users::admin_unlock(&db, id).await.expect("unlock");
    assert_eq!(mfa_failure_count(&db, id).await, 0);
}

#[tokio::test]
async fn a_password_verification_does_not_clear_the_second_factor_count() {
    // The reverse guard: only the whole sign-in (L02) or an operator unlock
    // resets it. `clear_lockout` is the password path's reset.
    let db = fresh_db();
    let id = seed_mfa_locked_user(&db).await;
    repos::users::clear_lockout(&db, id).await.expect("clear");
    assert_eq!(mfa_failure_count(&db, id).await, 5);
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
