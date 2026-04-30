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
use chrono::Duration;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::Database;
use sui_id_store::models::{AuditLogRow, SessionRow};
use sui_id_store::repos::{audit, credentials, sessions, users};

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
    };
    sessions::insert(db, &row)?;
    record_login_success(db, clock, user.id);
    Ok(LoginOutcome::SessionEstablished(row))
}

/// Resolve a session id to its user, if the session is still active.
pub fn resolve(db: &Database, clock: &SharedClock, id: SessionId) -> CoreResult<UserId> {
    let row = sessions::get(db, id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
        other => other.into(),
    })?;
    if row.revoked_at.is_some() || row.expires_at <= clock.now() {
        return Err(CoreError::Unauthenticated);
    }
    Ok(row.user_id)
}

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
