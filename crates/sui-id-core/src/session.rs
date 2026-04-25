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
use sui_id_store::models::{AuditLogRow, SessionRow};
use sui_id_store::repos::{audit, credentials, sessions, users};
use sui_id_store::Database;
use sui_id_shared::ids::{SessionId, UserId};

const SESSION_LIFETIME_HOURS: i64 = 12;

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

pub fn login(db: &Database, clock: &SharedClock, username: &str, password: &str) -> CoreResult<SessionRow> {
    let user = match users::find_by_username(db, username) {
        Ok(u) => u,
        Err(sui_id_store::StoreError::NotFound) => {
            // Run a dummy verify to keep response time roughly constant
            // regardless of whether the username existed.
            let _ = verify_password(
                password,
                "$argon2id$v=19$m=65536,t=2,p=1$c2FsdHNhbHRzYWx0$ZHVtbXloYXNoZHVtbXloYXNoZHVtbXloYXNoZHVtbQ",
            );
            record_login_failure(db, clock, username, "unknown user");
            return Err(CoreError::InvalidCredentials);
        }
        Err(e) => return Err(e.into()),
    };

    if user.is_disabled || user.is_deleted {
        record_login_failure(db, clock, username, "user disabled or deleted");
        return Err(CoreError::InvalidCredentials);
    }

    let cred = credentials::get(db, user.id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
        other => other.into(),
    })?;

    if let Err(e) = verify_password(password, &cred.password_hash) {
        record_login_failure(db, clock, username, "wrong password");
        return Err(e);
    }

    let now = clock.now();
    let row = SessionRow {
        id: SessionId::new(),
        user_id: user.id,
        expires_at: now + Duration::hours(SESSION_LIFETIME_HOURS),
        created_at: now,
        revoked_at: None,
    };
    sessions::insert(db, &row)?;
    record_login_success(db, clock, user.id);
    Ok(row)
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
