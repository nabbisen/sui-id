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
use crate::password::{DUMMY_PHC, verify_password};
use crate::time::SharedClock;
use chrono::Duration;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::Database;
use sui_id_store::models::{AuditLogRow, SessionRow};
use sui_id_store::repos::{audit, credentials, sessions, users};

const SESSION_LIFETIME_HOURS: i64 = 12;

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

/// The best-effort `auth.login.failure` row for a sign-in refused before
/// any credential check (unknown, disabled, deleted or locked). Also used
/// by the directory path in the HTTP layer for the same refusals.
pub async fn record_refused_login(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    reason: &str,
) {
    record_login_failure(db, clock, username, reason).await;
}

async fn record_login_failure(db: &Database, clock: &SharedClock, username: &str, reason: &str) {
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
    )
    .await;
}

pub async fn login(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    max_lockout_secs: i64,
) -> CoreResult<SessionRow> {
    match login_with_mfa(
        db,
        clock,
        username,
        password,
        max_lockout_secs,
        SessionAudience::Any,
    )
    .await?
    {
        LoginOutcome::SessionEstablished(row) => Ok(row),
        LoginOutcome::MfaRequired { .. } | LoginOutcome::AudienceRefused => {
            Err(CoreError::Unauthenticated)
        }
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
    /// RFC 102 A7: the password was correct, but the session would be
    /// refused afterwards (a user without admin read access signing in
    /// to an admin-only destination). Nothing was written: no session, no
    /// success event, no bookkeeping.
    AudienceRefused,
}

/// Who a session created by this sign-in is for (RFC 102 A7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAudience {
    /// Any active user: the OIDC authorize flow and `/me`.
    Any,
    /// The admin panel: only a role that can read it.
    AdminReaders,
}

/// Password authentication that respects per-user MFA enrolment.
pub async fn login_with_mfa(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    max_lockout_secs: i64,
    audience: SessionAudience,
) -> CoreResult<LoginOutcome> {
    let user = match users::find_by_username(db, username).await {
        Ok(u) => u,
        Err(sui_id_store::StoreError::NotFound) => {
            // Constant-time-ish dummy verify regardless of branch.
            let _ = verify_password(password, DUMMY_PHC).await;
            record_login_failure(db, clock, username, "unknown user").await;
            return Err(CoreError::InvalidCredentials);
        }
        Err(e) => return Err(e.into()),
    };

    if user.is_disabled || user.is_deleted {
        let _ = verify_password(password, DUMMY_PHC).await;
        record_login_failure(db, clock, username, "user disabled or deleted").await;
        return Err(CoreError::InvalidCredentials);
    }

    // Lockout check. We do it *before* fetching the credential row
    // and running Argon2 — there's no point grinding the hash for an
    // account that we already know we're going to refuse. To
    // preserve timing equivalence with the active-and-wrong-password
    // path, we still run a dummy Argon2 verify before returning.
    if let Some(locked_until) = user.locked_until
        && locked_until > clock.now()
    {
        let _ = verify_password(password, DUMMY_PHC).await;
        // Audit-logged with a different reason so operators can
        // distinguish a brute-force attempt from honest typos.
        // The HTTP response is the same generic 401 either way.
        record_login_failure(db, clock, username, "account locked").await;
        return Err(CoreError::InvalidCredentials);
    }
    if user.locked_until.is_some() {
        // Stale lock — `locked_until` is in the past. Fall through;
        // a successful sign-in clears it (L01, L02, L03), as does a
        // credential change for the password lockout (U09, U10, RFC 118),
        // and a failure restarts the counter from where it was
        // (which is correct: the attacker has been sleeping, but so
        // has our knowledge of them).
    }

    // RFC 115 D8: a local account with no credential row (created without a
    // password, not yet activated) is refused like any other wrong
    // password: the dummy verify runs for timing equivalence and the
    // failure is counted and audited by U22 below. This used to return
    // here, before both, so "never activated" was distinguishable by
    // timing and its attempts were neither counted nor audited.
    let cred = match credentials::get(db, user.id).await {
        Ok(cred) => Some(cred),
        Err(sui_id_store::StoreError::NotFound) => None,
        Err(other) => return Err(other.into()),
    };
    let verdict = match &cred {
        Some(cred) => verify_password(password, &cred.password_hash).await,
        None => {
            let _ = verify_password(password, DUMMY_PHC).await;
            Err(CoreError::InvalidCredentials)
        }
    };

    if let Err(e) = verdict {
        // Wrong password: bump the counter and, if it crosses the
        // threshold, stamp the lock — atomically, as RFC 094's U22.
        // `lock_window_for_count` runs *inside* the transaction against
        // the freshly-incremented count, so the branch is decided from
        // the same guarded read the counter update used; there is no
        // window where the counter is bumped but an owed lock is not
        // yet set, and no window where the counter bumps without its
        // audit row (both previously separate, best-effort calls — see
        // `sui_id_store::commands::record_login_failure`'s own doc
        // comment). A failure to record this — the DB write itself
        // failing — now surfaces as an error from this call rather
        // than being silently swallowed: RFC 094/085 both treat an
        // audit-subsystem failure as an operation failure, not a
        // silent gap, and that now holds here for the first time.
        sui_id_store::commands::record_login_failure(db, user.id, move |count| {
            lockout_backoff(count, max_lockout_secs)
        })
        .await?;
        return Err(e);
    }

    // RFC 102 A7: refuse a session nobody will hold before anything is
    // written — no pending-MFA row, no session, no event — from the role
    // read above (outside any transaction).
    if audience == SessionAudience::AdminReaders && !user.role.can_read_admin() {
        return Ok(LoginOutcome::AudienceRefused);
    }

    // Branch on MFA enrolment.
    if crate::mfa::is_mfa_enabled(db, user.id).await? {
        // The password counter and stale lock are reset by L02, when the
        // whole sign-in commits (RFC 102 stage 3 ruling), not here.
        let pending = crate::mfa::issue_pending_mfa(db, clock, user.id).await?;
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
        )
        .await;
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
    // RFC 102 L01: the session, its `auth.login.success` event, the
    // counter and stale-lock reset, `last_login_at` and cap eviction
    // commit together or not at all. RFC 074's best-effort
    // `set_last_login` is superseded: a failing write here means the
    // database is failing, and the sign-in fails with it. A failure is
    // never counted as a wrong password (A9); the caller returns the
    // uniform failure and logs the cause.
    sui_id_store::commands::sign_in_with_password(db, row.clone())
        .await
        .map_err(|e| match e {
            // The in-transaction re-read lost: the user was disabled,
            // deleted or locked after the password was verified. That is
            // an ordinary refused sign-in, not a storage fault.
            sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
            other => other.into(),
        })?;
    Ok(LoginOutcome::SessionEstablished(row))
}

/// Resolve a session id to its user, if the session is still active.
///
/// A session whose user is disabled or deleted is not active: it is
/// refused as `Unauthenticated`, from the same read as the session row.
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
pub async fn resolve(db: &Database, clock: &SharedClock, id: SessionId) -> CoreResult<UserId> {
    // The user's active state comes from the same statement as the session,
    // so a disabled or deleted user's session is refused without a second
    // round trip.
    let (row, user_active) = sessions::get_with_user_active(db, id)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
            other => other.into(),
        })?;
    let now = clock.now();
    if row.revoked_at.is_some() || row.expires_at <= now || !user_active {
        return Err(CoreError::Unauthenticated);
    }
    // Idle-timeout enforcement.
    if let Ok(settings) = sui_id_store::repos::server_settings::get(db).await {
        let timeout = settings.idle_session_timeout_secs;
        if timeout > 0 {
            let reference = row.last_used_at.unwrap_or(row.created_at);
            let elapsed = (now - reference).num_seconds();
            if elapsed > timeout {
                // Past idle window: revoke and refuse. The revoke
                // is best-effort; if it fails, the next request
                // for the same id will simply re-evaluate and
                // reach the same conclusion.
                let _ = sessions::revoke(db, id).await;
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
pub async fn touch_last_used(db: &Database, clock: &SharedClock, id: SessionId) -> CoreResult<()> {
    let now = clock.now();
    let row = match sessions::get(db, id).await {
        Ok(r) => r,
        Err(sui_id_store::StoreError::NotFound) => return Ok(()),
        Err(other) => return Err(other.into()),
    };
    let stale = match row.last_used_at {
        Some(t) => (now - t).num_seconds() >= LAST_USED_AT_THROTTLE_SECS,
        None => true,
    };
    if stale {
        sessions::touch_last_used(db, id, now).await?;
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

pub async fn logout(db: &Database, id: SessionId) -> CoreResult<()> {
    sessions::revoke(db, id).await?;
    Ok(())
}

/// End a user's RP-facing session. Revokes the named session and **all**
/// outstanding refresh tokens for that user. Used by RP-initiated logout
/// where we want a clean slate, not just one expired cookie.
pub async fn logout_user(db: &Database, clock: &SharedClock, user_id: UserId) -> CoreResult<()> {
    let _ = clock; // signature kept symmetric with other lifecycle fns
    sessions::revoke_all_for_user(db, user_id).await?;
    sui_id_store::repos::refresh_tokens::revoke_all_for_user(db, user_id).await?;
    Ok(())
}

#[cfg(test)]
#[path = "session/lockout_tests.rs"]
mod lockout_tests;

#[cfg(test)]
#[path = "session/session_limit_tests.rs"]
mod session_limit_tests;
