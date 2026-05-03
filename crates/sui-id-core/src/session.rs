//! Admin session lifecycle.
//!
//! Sessions are server-side rows; the cookie value is the session id. We
//! purposefully do not embed any user data in the cookie itself, so that
//! revocation always wins (deleting the row immediately invalidates any
//! outstanding cookie).
//!
//! Authentication outcomes (success and failure) are written to the audit
//! log so that operators can investigate after the fact. Failures are
//! recorded *without* the supplied password.

use crate::errors::{CoreError, CoreResult};
use crate::password::verify_password;
use crate::time::SharedClock;
use chrono::{DateTime, Duration, Utc};
use sui_id_store::models::{AuditLogRow, SessionRow};
use sui_id_store::repos::{audit, credentials, sessions, users};
use sui_id_store::Database;
use sui_id_shared::ids::{SessionId, UserId};

const SESSION_LIFETIME_HOURS: i64 = 12;

/// A fixed dummy Argon2id PHC string used as a decoy when we want to
/// burn comparable wall-clock time on a path that wouldn't otherwise
/// hit the password hash — most importantly the "this user is locked"
/// path, but also the "no such user" path. Without this, an attacker
/// could distinguish locked accounts (instant 401) from active
/// accounts (Argon2-delayed 401) by timing alone.
///
/// The hash is well-formed but does not match any real password.
const DUMMY_PHC: &str =
    "$argon2id$v=19$m=65536,t=2,p=1$c2FsdHNhbHRzYWx0$ZHVtbXloYXNoZHVtbXloYXNoZHVtbXloYXNoZHVtbQ";

/// Progressive backoff curve. Maps a *new* consecutive-failure count
/// (so n = 1 means "this is the first failure") to an optional lock
/// window length.
///
/// The first two failures get no lock — every operator typo deserves
/// a free pass. From the third onward the window grows exponentially,
/// capped at the operator-configured `max_secs`. The cap is
/// configurable so operators can choose between a 15-minute cooldown
/// for low-stakes installs and the full 48 hours for tighter setups;
/// see `[security] max_lockout` in the config.
pub fn lockout_backoff(failures: i64, max_secs: i64) -> Option<Duration> {
    let secs: i64 = match failures {
        ..=2 => return None,
        3 => 30,
        4 => 60,
        5 => 5 * 60,
        6 => 30 * 60,
        7 => 2 * 60 * 60,
        8 => 6 * 60 * 60,
        9 => 12 * 60 * 60,
        _ => 24 * 60 * 60,
    };
    Some(Duration::seconds(secs.min(max_secs)))
}

fn record_login_failure(db: &Database, clock: &SharedClock, username: &str, reason: &str) {
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: clock.now(),
            actor: None,
            action: "auth.login.failure".into(),
            target: Some(username.to_owned()),
            result: "denied".into(),
            note: Some(reason.to_owned()),
        },
    );
}

fn record_login_success(db: &Database, clock: &SharedClock, user_id: UserId) {
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: clock.now(),
            actor: Some(user_id),
            action: "auth.login.success".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: None,
        },
    );
}

pub fn login(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    max_lockout_secs: i64,
) -> CoreResult<SessionRow> {
    match login_with_mfa(db, clock, username, password, max_lockout_secs)? {
        LoginOutcome::SessionEstablished(row) => Ok(row),
        LoginOutcome::MfaRequired { .. } => Err(CoreError::Unauthenticated),
    }
}

/// Outcome of a password-only login attempt.
///
/// `SessionEstablished` is the normal path: password OK and the user does
/// not have MFA enrolled, so a session is issued immediately.
///
/// `MfaRequired` is returned when the user has TOTP enabled. The bin
/// layer is expected to set a short-lived cookie pointing at the
/// `pending` row and redirect to the MFA challenge page; only after the
/// user submits a valid code does a real session get created (via
/// `crate::mfa::verify_pending`).
pub enum LoginOutcome {
    SessionEstablished(SessionRow),
    MfaRequired {
        pending: sui_id_store::models::LoginPendingMfaRow,
    },
}

/// Password authentication that respects per-user MFA enrolment.
pub fn login_with_mfa(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    max_lockout_secs: i64,
) -> CoreResult<LoginOutcome> {
    let user = match users::find_by_username(db, username) {
        Ok(u) => u,
        Err(sui_id_store::StoreError::NotFound) => {
            // Constant-time-ish dummy verify regardless of branch.
            let _ = verify_password(password, DUMMY_PHC);
            record_login_failure(db, clock, username, "unknown user");
            return Err(CoreError::InvalidCredentials);
        }
        Err(e) => return Err(e.into()),
    };

    if user.is_disabled || user.is_deleted {
        let _ = verify_password(password, DUMMY_PHC);
        record_login_failure(db, clock, username, "user disabled or deleted");
        return Err(CoreError::InvalidCredentials);
    }

    // Lockout check. We do it *before* fetching the credential row
    // and running Argon2 — there's no point grinding the hash for an
    // account that we already know we're going to refuse. To
    // preserve timing equivalence with the active-and-wrong-password
    // path, we still run a dummy Argon2 verify before returning.
    if let Some(locked_until) = user.locked_until {
        if locked_until > clock.now() {
            let _ = verify_password(password, DUMMY_PHC);
            // Audit-logged with a different reason so operators can
            // distinguish a brute-force attempt from honest typos.
            // The HTTP response is the same generic 401 either way.
            record_login_failure(db, clock, username, "account locked");
            return Err(CoreError::InvalidCredentials);
        }
        // Stale lock — `locked_until` is in the past. Fall through;
        // a successful password will clear it via `clear_lockout`,
        // and a failure restarts the counter from where it was
        // (which is correct: the attacker has been sleeping, but so
        // has our knowledge of them).
    }

    let cred = credentials::get(db, user.id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
        other => other.into(),
    })?;

    if let Err(e) = verify_password(password, &cred.password_hash) {
        // Wrong password: bump the counter, possibly stamp a lock,
        // and audit. The lock window is computed from the new count
        // — so the third failure stamps the first 30-second lock,
        // the fourth stamps a one-minute lock, and so on.
        let next_count = users::record_login_failure(db, user.id, None).unwrap_or(0);
        if let Some(window) = lockout_backoff(next_count, max_lockout_secs) {
            let until = clock.now() + window;
            let _ = users::record_login_failure(db, user.id, Some(until));
            // Re-emit the audit row with the lock-applied note so
            // the same /admin/login submission yields one informative
            // event in the log, not two.
            let _ = audit::append(
                db,
                &AuditLogRow {
                    at: clock.now(),
                    actor: Some(user.id),
                    action: "auth.login.locked".into(),
                    target: Some(user.id.to_string()),
                    result: "denied".into(),
                    note: Some(format!(
                        "consecutive failures = {next_count}, locked for {} s",
                        window.num_seconds()
                    )),
                },
            );
        } else {
            record_login_failure(db, clock, username, "wrong password");
        }
        return Err(e);
    }

    // Password OK: reset counter and clear any stale lock.
    let _ = users::clear_lockout(db, user.id);

    // Branch on MFA enrolment.
    if crate::mfa::is_mfa_enabled(db, user.id)? {
        let pending = crate::mfa::issue_pending_mfa(db, clock, user.id)?;
        // Audit success of the *password* step. The MFA step issues its
        // own audit entry on completion.
        let _ = audit::append(
            db,
            &AuditLogRow {
                at: clock.now(),
                actor: Some(user.id),
                action: "auth.login.password_ok_mfa_required".into(),
                target: Some(user.id.to_string()),
                result: "ok".into(),
                note: None,
            },
        );
        return Ok(LoginOutcome::MfaRequired { pending });
    }

    let now = clock.now();
    let row = SessionRow {
        id: SessionId::new(),
        user_id: user.id,
        expires_at: now + Duration::hours(SESSION_LIFETIME_HOURS),
        created_at: now,
        revoked_at: None,
        // No MFA was required for this user, so the only factor is
        // the password. The session's `acr` will be "1" and its
        // `amr` will be ["pwd"].
        auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
        // No step-up has happened (and none was needed for login,
        // since this user has no MFA enrolled). Sensitive actions
        // that gate on `step_up::is_fresh` will see `None` here
        // and behave appropriately for a no-MFA account — see the
        // `is_fresh` doc comment.
        last_step_up_at: None,
            last_used_at: None,
    };
    sessions::insert(db, &row)?;
    enforce_concurrent_session_cap(db, clock, user.id);
    record_login_success(db, clock, user.id);
    Ok(LoginOutcome::SessionEstablished(row))
}

/// Apply the concurrent-session cap (v0.25.0). When
/// `server_settings.max_concurrent_sessions` is non-zero and the
/// user's count of active sessions exceeds it, revoke the
/// oldest sessions in FIFO order until the count is back at the
/// cap.
///
/// Best-effort: any DB error here only logs at the call site
/// (we keep this function infallible by absorbing settings/repo
/// failures). The new session is already inserted by the time
/// we run, so a failure to evict an old session does not block
/// the user from signing in — at worst the cap is briefly
/// exceeded until the next login or until the next idle-timeout
/// pass cleans things up.
pub(crate) fn enforce_concurrent_session_cap(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
) {
    let settings = match sui_id_store::repos::server_settings::get(db) {
        Ok(s) => s,
        Err(_) => return,
    };
    let cap = settings.max_concurrent_sessions;
    if cap <= 0 {
        return;
    }
    let now = clock.now();
    let count = match sessions::count_active_for_user(db, user_id, now) {
        Ok(c) => c,
        Err(_) => return,
    };
    if count <= cap {
        return;
    }
    let evict = count - cap;
    let oldest = match sessions::oldest_active_for_user(db, user_id, now, evict) {
        Ok(rows) => rows,
        Err(_) => return,
    };
    for old in oldest {
        let _ = sessions::revoke(db, old.id);
    }
}

/// Resolve a session id to its user, if the session is still active.
///
/// In addition to the obvious revoked / expired_at checks, since
/// v0.25.0 this also enforces the optional **idle-session-timeout**:
/// if the server-settings row's `idle_session_timeout_secs` is
/// non-zero and the session's last presentation was longer ago
/// than that, the session is treated as expired and revoked
/// in-place before returning `Unauthenticated`. The revoke is a
/// best-effort cleanup; the auth decision does not depend on it
/// succeeding.
///
/// `last_used_at = NULL` (rows from before migration 0018) is
/// treated as "as old as `created_at`" — the conservative choice
/// that aligns pre-migration sessions with the same idle policy
/// as new ones.
pub fn resolve(db: &Database, clock: &SharedClock, id: SessionId) -> CoreResult<UserId> {
    let row = sessions::get(db, id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
        other => other.into(),
    })?;
    let now = clock.now();
    if row.revoked_at.is_some() || row.expires_at <= now {
        return Err(CoreError::Unauthenticated);
    }
    // Idle-timeout enforcement.
    if let Ok(settings) = sui_id_store::repos::server_settings::get(db) {
        let timeout = settings.idle_session_timeout_secs;
        if timeout > 0 {
            let reference = row.last_used_at.unwrap_or(row.created_at);
            let elapsed = (now - reference).num_seconds();
            if elapsed > timeout {
                // Past idle window: revoke and refuse. The revoke
                // is best-effort; if it fails, the next request
                // for the same id will simply re-evaluate and
                // reach the same conclusion.
                let _ = sessions::revoke(db, id);
                return Err(CoreError::Unauthenticated);
            }
        }
    }
    Ok(row.user_id)
}

/// Update `sessions.last_used_at` to `now`, throttled.
///
/// Called from authenticated request handlers via the
/// `RequireSession` extractor (or its admin-checking variant).
/// Throttled at the HTTP layer — we write the column at most once
/// per minute per session — so that a busy session does not
/// generate one DB write per HTTP request. Throttling is not
/// stored separately; the throttle decision is made by comparing
/// the row's existing `last_used_at` against `now -
/// LAST_USED_AT_THROTTLE_SECS`.
pub fn touch_last_used(
    db: &Database,
    clock: &SharedClock,
    id: SessionId,
) -> CoreResult<()> {
    let now = clock.now();
    let row = match sessions::get(db, id) {
        Ok(r) => r,
        Err(sui_id_store::StoreError::NotFound) => return Ok(()),
        Err(other) => return Err(other.into()),
    };
    let stale = match row.last_used_at {
        Some(t) => (now - t).num_seconds() >= LAST_USED_AT_THROTTLE_SECS,
        None => true,
    };
    if stale {
        sessions::touch_last_used(db, id, now)?;
    }
    Ok(())
}

/// Throttle window for `touch_last_used`: a session whose
/// `last_used_at` is more recent than this many seconds is not
/// re-written on the current request. Sixty seconds is the
/// classic "bucket" granularity — enough to dampen 1-write-per-
/// HTTP-request load, fine-grained enough that the idle-timeout
/// check stays meaningful (a few-minutes timeout would still
/// reflect actual usage).
pub const LAST_USED_AT_THROTTLE_SECS: i64 = 60;

pub fn logout(db: &Database, id: SessionId) -> CoreResult<()> {
    sessions::revoke(db, id)?;
    Ok(())
}

/// End a user's RP-facing session. Revokes the named session and **all**
/// outstanding refresh tokens for that user. Used by RP-initiated logout
/// where we want a clean slate, not just one expired cookie.
pub fn logout_user(db: &Database, clock: &SharedClock, user_id: UserId) -> CoreResult<()> {
    let _ = clock; // signature kept symmetric with other lifecycle fns
    sessions::revoke_all_for_user(db, user_id)?;
    sui_id_store::repos::refresh_tokens::revoke_all_for_user(db, user_id)?;
    Ok(())
}

#[cfg(test)]
mod lockout_tests {
    //! Properties and units around `lockout_backoff`. The function
    //! itself is a small piece of arithmetic, but it sits on the
    //! hottest security-decision path in sui-id, so it earns dense
    //! testing.

    use super::lockout_backoff;
    use proptest::prelude::*;

    #[test]
    fn first_two_failures_yield_no_lock() {
        // Operators routinely fat-finger a password; the first two
        // attempts must have no observable consequence beyond
        // bumping the failure counter.
        assert_eq!(lockout_backoff(1, 24 * 60 * 60), None);
        assert_eq!(lockout_backoff(2, 24 * 60 * 60), None);
    }

    #[test]
    fn third_failure_yields_a_short_lock() {
        let d = lockout_backoff(3, 24 * 60 * 60).expect("lock at 3rd failure");
        assert_eq!(d.num_seconds(), 30);
    }

    #[test]
    fn lock_window_is_capped_at_max_secs() {
        // Ninth+ failure on the curve hits 12h+; with a 1-hour cap
        // we should see exactly 1 hour, never higher.
        let cap = 60 * 60;
        for n in 9..20 {
            let d = lockout_backoff(n, cap).expect("locked");
            assert!(
                d.num_seconds() <= cap,
                "failure {n} produced {} s, exceeds cap {} s",
                d.num_seconds(),
                cap
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            ..ProptestConfig::default()
        })]

        /// The curve must be monotonically non-decreasing: a higher
        /// failure count never produces a *shorter* lock than a
        /// lower one (within the same cap). A regression that
        /// flipped the table around would let an attacker
        /// preferentially time more attempts.
        #[test]
        fn backoff_is_monotone_in_failure_count(
            cap in 1i64..(48 * 60 * 60),
            a in 1i64..15,
            b in 1i64..15,
        ) {
            prop_assume!(a <= b);
            let da = lockout_backoff(a, cap).map(|d| d.num_seconds()).unwrap_or(0);
            let db = lockout_backoff(b, cap).map(|d| d.num_seconds()).unwrap_or(0);
            prop_assert!(db >= da, "{a} -> {da}s, {b} -> {db}s");
        }

        /// No matter how many failures or how the curve evolves, the
        /// returned window never exceeds the operator-set cap. This
        /// is the property the configuration knob is supposed to
        /// give us — operators choose 15min and they get 15min.
        #[test]
        fn backoff_is_bounded_by_max_secs(
            cap in 1i64..(48 * 60 * 60),
            n in 1i64..50,
        ) {
            if let Some(d) = lockout_backoff(n, cap) {
                prop_assert!(d.num_seconds() <= cap);
            }
        }
    }
}

#[cfg(test)]
mod session_limit_tests {
    //! Idle-session timeout and concurrent-session cap (v0.25.0).
    use super::*;
    use crate::time::{system_clock, MockClock, SharedClock};
    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::Database;

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    fn make_user(db: &Database) -> UserId {
        use sui_id_store::models::UserRow;
        use sui_id_store::repos::users;
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "alice".into(),
                display_name: None,
                is_admin: false,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                failed_login_count: 0,
                locked_until: None,
                email: None,
                preferred_lang: None,
            },
        )
        .expect("user");
        id
    }

    fn insert_session(
        db: &Database,
        user_id: UserId,
        created_at: chrono::DateTime<Utc>,
        last_used_at: Option<chrono::DateTime<Utc>>,
    ) -> SessionId {
        let id = SessionId::new();
        sessions::insert(
            db,
            &SessionRow {
                id,
                user_id,
                expires_at: created_at + ChronoDuration::hours(24),
                created_at,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
                last_step_up_at: None,
                last_used_at,
            },
        )
        .expect("insert");
        id
    }

    #[test]
    fn resolve_passes_when_idle_timeout_disabled() {
        let db = fresh_db();
        let clock = system_clock();
        let uid = make_user(&db);
        // last_used_at is far in the past; default settings have
        // idle_session_timeout_secs = 0 = disabled, so resolve
        // must succeed.
        let stale = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let sid = insert_session(&db, uid, Utc::now(), Some(stale));
        assert_eq!(resolve(&db, &clock, sid).expect("resolve"), uid);
    }

    #[test]
    fn resolve_revokes_after_idle_window() {
        let db = fresh_db();
        let uid = make_user(&db);
        // Configure a 60-second idle timeout.
        sui_id_store::repos::server_settings::update_idle_session_timeout(
            &db,
            60,
            Utc::now(),
        )
        .expect("set timeout");
        // Make a session that was last used 2 minutes ago and a
        // mock clock at "now", so elapsed = 120s > 60s.
        let now = Utc::now();
        let stale = now - ChronoDuration::seconds(120);
        let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(stale));
        let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
        // First call: idle window exceeded → revoke + Unauth.
        assert!(matches!(
            resolve(&db, &clock, sid),
            Err(CoreError::Unauthenticated)
        ));
        // The session is now revoked in the DB.
        let row = sessions::get(&db, sid).expect("get");
        assert!(row.revoked_at.is_some());
    }

    #[test]
    fn resolve_passes_within_idle_window() {
        let db = fresh_db();
        let uid = make_user(&db);
        sui_id_store::repos::server_settings::update_idle_session_timeout(
            &db,
            300,
            Utc::now(),
        )
        .expect("set timeout");
        let now = Utc::now();
        let recent = now - ChronoDuration::seconds(10);
        let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(recent));
        let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
        assert_eq!(resolve(&db, &clock, sid).expect("resolve"), uid);
    }

    #[test]
    fn resolve_treats_null_last_used_at_as_created_at() {
        let db = fresh_db();
        let uid = make_user(&db);
        sui_id_store::repos::server_settings::update_idle_session_timeout(
            &db,
            60,
            Utc::now(),
        )
        .expect("set timeout");
        // last_used_at = None: created 2 minutes ago, so falling
        // back to created_at means 120 > 60 = revoked.
        let now = Utc::now();
        let sid = insert_session(&db, uid, now - ChronoDuration::seconds(120), None);
        let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
        assert!(matches!(
            resolve(&db, &clock, sid),
            Err(CoreError::Unauthenticated)
        ));
    }

    #[test]
    fn touch_last_used_throttles_within_window() {
        let db = fresh_db();
        let uid = make_user(&db);
        let now = Utc::now();
        let original = now - ChronoDuration::seconds(10);
        let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(original));
        let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
        // Throttle window is 60s; 10s old should not write.
        touch_last_used(&db, &clock, sid).expect("touch");
        let row = sessions::get(&db, sid).expect("get");
        assert_eq!(row.last_used_at, Some(original));
    }

    #[test]
    fn touch_last_used_writes_when_stale() {
        let db = fresh_db();
        let uid = make_user(&db);
        let now = Utc::now();
        let stale = now - ChronoDuration::seconds(120);
        let sid = insert_session(&db, uid, now - ChronoDuration::hours(1), Some(stale));
        let clock: SharedClock = std::sync::Arc::new(MockClock::at(now));
        touch_last_used(&db, &clock, sid).expect("touch");
        let row = sessions::get(&db, sid).expect("get");
        // The new value should be ~now, definitely not the stale one.
        let updated = row.last_used_at.expect("set");
        assert!(updated > stale);
    }

    #[test]
    fn enforce_cap_does_nothing_when_cap_zero() {
        let db = fresh_db();
        let clock = system_clock();
        let uid = make_user(&db);
        // Insert 5 active sessions; cap = 0 = disabled.
        for i in 0..5 {
            let _ = insert_session(
                &db,
                uid,
                Utc::now() - ChronoDuration::seconds(i),
                None,
            );
        }
        enforce_concurrent_session_cap(&db, &clock, uid);
        let active =
            sessions::count_active_for_user(&db, uid, Utc::now()).expect("count");
        assert_eq!(active, 5);
    }

    #[test]
    fn enforce_cap_evicts_oldest_in_fifo_order() {
        let db = fresh_db();
        let clock = system_clock();
        let uid = make_user(&db);
        // Cap = 2; insert 4 sessions with distinct created_at.
        sui_id_store::repos::server_settings::update_max_concurrent_sessions(
            &db,
            2,
            Utc::now(),
        )
        .expect("set cap");
        let base = Utc::now() - ChronoDuration::hours(1);
        let s1 = insert_session(&db, uid, base, None);
        let s2 = insert_session(&db, uid, base + ChronoDuration::seconds(1), None);
        let s3 = insert_session(&db, uid, base + ChronoDuration::seconds(2), None);
        let s4 = insert_session(&db, uid, base + ChronoDuration::seconds(3), None);
        // Run eviction: 4 active, cap 2 → 2 oldest (s1, s2)
        // are revoked.
        enforce_concurrent_session_cap(&db, &clock, uid);
        let still_active = |sid: SessionId| {
            sessions::get(&db, sid).expect("get").revoked_at.is_none()
        };
        assert!(!still_active(s1), "s1 should be revoked");
        assert!(!still_active(s2), "s2 should be revoked");
        assert!(still_active(s3), "s3 should remain");
        assert!(still_active(s4), "s4 should remain");
    }
}
