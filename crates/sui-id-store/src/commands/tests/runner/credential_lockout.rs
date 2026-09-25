use super::*;

// ── RFC 118 D1, D3, D5 — a credential change clears the *password* lockout ──
//
// U09 (self-service change) and U10 (completion of a reset link) clear
// `failed_login_count` always, clear `locked_until` only while
// `mfa_failure_count` is under the second-factor threshold, and never touch
// `mfa_failure_count`. `locked_until` is one column with two causes, so the
// carve-out is the property these tests own and mutate.

/// Counters as the database holds them.
#[derive(Debug, PartialEq, Eq)]
struct Lock {
    password_failures: i64,
    locked_until: Option<chrono::DateTime<Utc>>,
    mfa_failures: i64,
}

async fn lock_of(db: &Database, id: UserId) -> Lock {
    db.with_conn(move |c| {
        Ok(c.query_row(
            "SELECT failed_login_count, locked_until, mfa_failure_count FROM users WHERE id = ?1",
            [id.to_string()],
            |r| {
                Ok(Lock {
                    password_failures: r.get(0)?,
                    locked_until: r.get(1)?,
                    mfa_failures: r.get(2)?,
                })
            },
        )?)
    })
    .await
    .expect("lock")
}

/// A local user with a credential, `failures` counted password failures, a
/// lock until `until` (if any), and `mfa_failures` second-factor failures.
async fn seed_locked(
    db: &Database,
    failures: i64,
    until: Option<chrono::DateTime<Utc>>,
    mfa_failures: i64,
) -> UserId {
    let mut user = a_user();
    user.failed_login_count = failures;
    user.locked_until = until;
    repos::users::create(db, &user).await.expect("create user");
    repos::credentials::upsert(
        db,
        &crate::models::CredentialRow {
            user_id: user.id,
            password_hash: "old-hash-placeholder".into(),
            updated_at: Utc::now(),
        },
    )
    .await
    .expect("seed credential");
    let id = user.id;
    db.with_conn(move |c| {
        c.execute(
            "UPDATE users SET mfa_failure_count = ?1 WHERE id = ?2",
            rusqlite::params![mfa_failures, id.to_string()],
        )?;
        Ok(())
    })
    .await
    .expect("seed mfa count");
    id
}

fn new_credential(id: UserId) -> crate::models::CredentialRow {
    crate::models::CredentialRow {
        user_id: id,
        password_hash: "new-hash-placeholder".into(),
        updated_at: Utc::now(),
    }
}

async fn u09(db: &Database, id: UserId) {
    change_password_self(db, id, new_credential(id), None, false)
        .await
        .expect("U09");
}

async fn u10(db: &Database, id: UserId) {
    let token = super::passwords::seed_reset_token(db, id).await;
    consume_and_reset_password(db, id, token, new_credential(id), Utc::now(), false)
        .await
        .expect("U10");
}

async fn last_note(db: &Database) -> String {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .expect("row")
        .note
        .unwrap_or_default()
}

fn in_an_hour() -> Option<chrono::DateTime<Utc>> {
    Some(Utc::now() + TimeDelta::hours(1))
}

// ── the defect, and the fix, on each path ──

#[tokio::test]
async fn u10_clears_the_counter_and_a_live_password_lock() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    u10(&db, id).await;
    assert_eq!(
        lock_of(&db, id).await,
        Lock {
            password_failures: 0,
            locked_until: None,
            mfa_failures: 0
        }
    );
}

#[tokio::test]
async fn u09_clears_the_counter_and_a_live_password_lock() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    u09(&db, id).await;
    assert_eq!(
        lock_of(&db, id).await,
        Lock {
            password_failures: 0,
            locked_until: None,
            mfa_failures: 0
        }
    );
}

#[tokio::test]
async fn a_lapsed_lock_still_has_its_counter_cleared() {
    // The counter does not reset when a lock lapses (RFC 118, correction 2),
    // so a credential change is the first thing that does.
    let db = fresh_db();
    let id = seed_locked(&db, 7, Some(Utc::now() - TimeDelta::minutes(5)), 0).await;
    u10(&db, id).await;
    let lock = lock_of(&db, id).await;
    assert_eq!(lock.password_failures, 0);
    assert_eq!(lock.locked_until, None);
}

// ── the carve-out: the second-factor lock is not the password lock ──

#[tokio::test]
async fn u10_keeps_a_lock_the_second_factor_lockout_set_and_its_count() {
    let db = fresh_db();
    let until = Utc::now() + TimeDelta::hours(48);
    let id = seed_locked(&db, 2, Some(until), 5).await;
    u10(&db, id).await;
    assert_eq!(
        lock_of(&db, id).await,
        Lock {
            password_failures: 0,
            locked_until: Some(until),
            mfa_failures: 5
        },
        "the password counter is cleared; the second-factor lock and count are not"
    );
}

#[tokio::test]
async fn u09_keeps_a_lock_the_second_factor_lockout_set_and_its_count() {
    let db = fresh_db();
    let until = Utc::now() + TimeDelta::hours(48);
    let id = seed_locked(&db, 2, Some(until), 5).await;
    u09(&db, id).await;
    assert_eq!(
        lock_of(&db, id).await,
        Lock {
            password_failures: 0,
            locked_until: Some(until),
            mfa_failures: 5
        }
    );
}

#[tokio::test]
async fn the_carve_out_is_the_threshold_and_not_any_second_factor_failure() {
    // Four second-factor failures is under the threshold (5): no second-factor
    // lock can have set this lock, so it is cleared, and the count is kept.
    let db = fresh_db();
    let id = seed_locked(&db, 3, in_an_hour(), 4).await;
    u10(&db, id).await;
    assert_eq!(
        lock_of(&db, id).await,
        Lock {
            password_failures: 0,
            locked_until: None,
            mfa_failures: 4
        }
    );
    // And at the threshold exactly, the lock stays.
    let until = Utc::now() + TimeDelta::hours(1);
    let at = seed_locked(
        &db,
        3,
        Some(until),
        crate::commands::MFA_FAILURE_LOCKOUT_THRESHOLD,
    )
    .await;
    u09(&db, at).await;
    assert_eq!(lock_of(&db, at).await.locked_until, Some(until));
}

// ── only the target's row ──

#[tokio::test]
async fn another_users_lock_is_untouched() {
    let db = fresh_db();
    let target = seed_locked(&db, 4, in_an_hour(), 0).await;
    let other = seed_locked(&db, 6, in_an_hour(), 3).await;
    let before = lock_of(&db, other).await;
    u10(&db, target).await;
    u09(&db, target).await;
    assert_eq!(lock_of(&db, other).await, before);
}

// ── same transaction ──

#[tokio::test]
async fn u10_injected_failure_leaves_the_counter_and_the_lock_exactly_as_they_were() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    let before = lock_of(&db, id).await;
    let token = super::passwords::seed_reset_token(&db, id).await;
    db.fault_injector().fail_before_next_append();
    let result =
        consume_and_reset_password(&db, id, token, new_credential(id), Utc::now(), false).await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert_eq!(lock_of(&db, id).await, before, "the clear rolled back");
    assert_eq!(
        repos::credentials::get(&db, id)
            .await
            .expect("cred")
            .password_hash,
        "old-hash-placeholder",
        "and so did the credential: they commit or roll back together"
    );
}

#[tokio::test]
async fn u09_injected_failure_leaves_the_counter_and_the_lock_exactly_as_they_were() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    let before = lock_of(&db, id).await;
    db.fault_injector().fail_before_next_append();
    let result = change_password_self(&db, id, new_credential(id), None, false).await;
    assert!(result.is_err(), "injected failure must surface as Err");
    assert_eq!(lock_of(&db, id).await, before, "the clear rolled back");
    assert_eq!(
        repos::credentials::get(&db, id)
            .await
            .expect("cred")
            .password_hash,
        "old-hash-placeholder"
    );
}

// ── D5 — the operator keeps a signal ──

#[tokio::test]
async fn the_event_carries_lockout_cleared_when_something_was_cleared() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    u10(&db, id).await;
    assert_eq!(last_note(&db).await, "origin=email lockout_cleared=4");

    let id = seed_locked(&db, 3, None, 0).await;
    u09(&db, id).await;
    assert_eq!(
        last_note(&db).await,
        "sessions_revoked=0 refresh_tokens_revoked=0 lockout_cleared=3"
    );
}

#[tokio::test]
async fn the_event_carries_no_lockout_cleared_when_nothing_was_there() {
    let db = fresh_db();
    let id = seed_locked(&db, 0, None, 0).await;
    u10(&db, id).await;
    assert_eq!(last_note(&db).await, "origin=email");
    let id = seed_locked(&db, 0, None, 0).await;
    u09(&db, id).await;
    assert_eq!(
        last_note(&db).await,
        "sessions_revoked=0 refresh_tokens_revoked=0"
    );
}

#[tokio::test]
async fn a_kept_second_factor_lock_alone_is_not_reported_as_cleared() {
    // Counter already zero, and the only lock is the second-factor one, which
    // is kept: nothing was cleared, so the attribute is absent.
    let db = fresh_db();
    let id = seed_locked(&db, 0, Some(Utc::now() + TimeDelta::hours(1)), 5).await;
    u10(&db, id).await;
    assert_eq!(last_note(&db).await, "origin=email");
}

// ── D4 — the pre-clear snapshot U10 hands back ──

async fn u10_snapshot(db: &Database, id: UserId) -> crate::repos::users::PasswordLockoutCleared {
    let token = super::passwords::seed_reset_token(db, id).await;
    consume_and_reset_password(db, id, token, new_credential(id), Utc::now(), false)
        .await
        .expect("U10")
        .into_inner()
}

#[tokio::test]
async fn u10_returns_what_it_found_before_it_cleared() {
    let db = fresh_db();
    let id = seed_locked(&db, 4, in_an_hour(), 0).await;
    let found = u10_snapshot(&db, id).await;
    assert_eq!(found.password_failures, 4);
    assert!(found.lock_lifted);
    assert_eq!(found.second_factor_lock_until, None);
    assert!(found.cleared());
    assert_eq!(found.attribute(), Some(4));
    // And the database no longer says it: the snapshot is the only record.
    assert_eq!(lock_of(&db, id).await.password_failures, 0);
}

#[tokio::test]
async fn u10_returns_the_time_a_kept_second_factor_lock_lifts() {
    let db = fresh_db();
    let until = Utc::now() + TimeDelta::hours(48);
    let id = seed_locked(&db, 2, Some(until), 5).await;
    let found = u10_snapshot(&db, id).await;
    assert_eq!(found.second_factor_lock_until, Some(until));
    assert!(!found.lock_lifted, "a kept lock is not a lifted one");
    assert!(found.cleared(), "the counter was cleared");
    assert_eq!(found.attribute(), Some(2));
}

#[tokio::test]
async fn u10_returns_nothing_to_report_for_an_account_with_nothing_to_clear() {
    let db = fresh_db();
    let id = seed_locked(&db, 0, None, 0).await;
    let found = u10_snapshot(&db, id).await;
    assert!(!found.cleared());
    assert_eq!(found.attribute(), None);
    assert_eq!(found.second_factor_lock_until, None);
}

#[tokio::test]
async fn a_second_factor_lock_that_has_lapsed_is_not_reported_as_kept() {
    let db = fresh_db();
    let id = seed_locked(&db, 0, Some(Utc::now() - TimeDelta::minutes(1)), 5).await;
    let found = u10_snapshot(&db, id).await;
    assert_eq!(found.second_factor_lock_until, None);
    assert!(!found.cleared());
}
