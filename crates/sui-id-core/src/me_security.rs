//! Core operations behind the `/me/security` self-service surface.
//!
//! These are the *actions* a signed-in user can take on their own
//! account that aren't simple reads (the reads are inlined into the
//! handler since they're cheap query wrappers). Today this is just
//! password change; future entries will land here too — for example
//! a self-serve recovery-email change once email support arrives.

use crate::errors::{CoreError, CoreResult};
use crate::password;
use crate::time::SharedClock;
use chrono::Utc;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::models::{AuditLogRow, CredentialRow};
use sui_id_store::repos::{audit, credentials, refresh_tokens, sessions};
use sui_id_store::Database;

/// Result of a successful self-service password change. The numbers
/// let the caller decide what to put in a flash message
/// ("Signed out 3 other sessions"), but they aren't load-bearing —
/// the action has already taken effect by the time you see them.
#[derive(Debug, Clone, Copy)]
pub struct PasswordChangeReport {
    /// Number of session rows revoked. Excludes the current session
    /// when `keep_current` was supplied.
    pub sessions_revoked: usize,
    /// Number of refresh-token rows revoked.
    pub refresh_tokens_revoked: usize,
}

/// Change the signed-in user's password.
///
/// `keep_current_session` controls whether the cookie session that
/// authorised this request stays alive. The default UX is to leave
/// the current session alive (otherwise the user is logged out the
/// instant they save the form, which feels broken even though it
/// is technically the most paranoid stance) but to revoke every
/// *other* session and every refresh token. That way an attacker
/// who has stolen a refresh token or has another live cookie loses
/// access immediately.
///
/// We deliberately do **not** invoke account lockout on a wrong
/// `current_password`. The user is already authenticated by their
/// session; brute-forcing the current-password field would be a
/// strange attack to mount, since it requires the cookie to begin
/// with. We do leave the rate limiter in the caller's hands so
/// that someone with a stolen cookie can't grind here either.
///
/// Errors:
/// - [`CoreError::InvalidCredentials`] if `current_password` does
///   not verify against the stored hash. Same error variant the
///   regular login path uses, which keeps callers' error mapping
///   simple.
/// - [`CoreError::BadRequest`] if `new_password` violates the
///   password policy (length, etc.).
/// - storage / hashing failures bubble up as [`CoreError::Internal`]
///   or [`CoreError::Password`].
pub fn change_password_self(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    current_password: &str,
    new_password: &str,
    keep_current_session: Option<SessionId>,
    revoke_others: bool,
) -> CoreResult<PasswordChangeReport> {
    // 1. Load the existing credential row. If it's missing, the
    //    user account exists without a password (shouldn't happen
    //    in practice, but be explicit) — refuse the same as a
    //    wrong password to avoid an oracle.
    let row = credentials::get(db, user_id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
        other => CoreError::from(other),
    })?;

    // 2. Verify the current password.
    password::verify_password(current_password, &row.password_hash)?;

    // 3. Enforce the policy on the new one. Done after the verify
    //    so that someone fishing for "is X my password?" via this
    //    endpoint doesn't get differentiated errors based on
    //    whether their guess passed policy.
    password::check_password_policy(new_password)?;

    // 4. Hash and store. `must_change` is reset — the user has
    //    just demonstrated agency.
    let new_phc = password::hash_password(new_password)?;
    credentials::upsert(
        db,
        &CredentialRow {
            user_id,
            password_hash: new_phc,
            must_change: false,
            updated_at: Utc::now(),
        },
    )?;

    // 5. Optionally sweep other live state. The caller asked for
    //    this when the box was checked; we revoke every other
    //    session and every active refresh token. The current
    //    session stays alive so the user isn't booted out of the
    //    page they're using.
    let mut report = PasswordChangeReport {
        sessions_revoked: 0,
        refresh_tokens_revoked: 0,
    };
    if revoke_others {
        report.sessions_revoked = match keep_current_session {
            Some(keep) => sessions::revoke_all_for_user_except(db, user_id, keep)?,
            None => sessions::revoke_all_for_user(db, user_id)?,
        };
        report.refresh_tokens_revoked = refresh_tokens::revoke_all_for_user(db, user_id)?;
    }

    // 6. Audit. The note carries the sweep counts so an operator
    //    looking at the log later can see at a glance whether the
    //    user opted to sign out other sessions.
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: clock.now(),
            actor: Some(user_id),
            action: "auth.password.changed_self".into(),
            target: Some(user_id.to_string()),
            result: "ok".into(),
            note: Some(format!(
                "sessions_revoked={} refresh_tokens_revoked={}",
                report.sessions_revoked, report.refresh_tokens_revoked
            )),
        },
    );

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_id_shared::ids::UserId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::users;

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    fn create_user_with_password(db: &Database, password: &str) -> UserId {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "alice".into(),
                display_name: None,
                is_admin: true,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                failed_login_count: 0,
                locked_until: None,
                email: None,
            },
        )
        .expect("create user");
        let phc = password::hash_password(password).expect("hash");
        credentials::upsert(
            db,
            &CredentialRow {
                user_id: id,
                password_hash: phc,
                must_change: false,
                updated_at: now,
            },
        )
        .expect("set credential");
        id
    }

    #[test]
    fn happy_path_replaces_hash_and_returns_zero_sweep_when_box_unchecked() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password");
        let r = change_password_self(
            &db,
            &clock,
            uid,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
        )
        .expect("change");
        assert_eq!(r.sessions_revoked, 0);
        assert_eq!(r.refresh_tokens_revoked, 0);
        // Old password no longer verifies; new one does.
        let stored = credentials::get(&db, uid).expect("cred").password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_err());
        assert!(password::verify_password("the-new-tester-password", &stored).is_ok());
    }

    #[test]
    fn wrong_current_password_is_rejected_as_invalid_credentials() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password");
        let r = change_password_self(
            &db,
            &clock,
            uid,
            "wrong-current-tester-password",
            "the-new-tester-password",
            None,
            false,
        );
        assert!(matches!(r, Err(CoreError::InvalidCredentials)));
        // Stored hash is unchanged.
        let stored = credentials::get(&db, uid).expect("cred").password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_ok());
    }

    #[test]
    fn weak_new_password_is_rejected_after_current_is_verified() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password");
        let r = change_password_self(
            &db,
            &clock,
            uid,
            "the-old-tester-password",
            "short",
            None,
            false,
        );
        assert!(matches!(r, Err(CoreError::BadRequest(_))), "{r:?}");
        // Stored hash unchanged — failure must not partially apply.
        let stored = credentials::get(&db, uid).expect("cred").password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_ok());
    }

    #[test]
    fn must_change_flag_is_reset_on_self_change() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password");
        // Set must_change=true via direct upsert, simulating a
        // pending admin reset.
        let phc = password::hash_password("the-old-tester-password").expect("hash");
        credentials::upsert(
            &db,
            &CredentialRow {
                user_id: uid,
                password_hash: phc,
                must_change: true,
                updated_at: Utc::now(),
            },
        )
        .expect("upsert");
        change_password_self(
            &db,
            &clock,
            uid,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
        )
        .expect("change");
        let row = credentials::get(&db, uid).expect("cred");
        assert!(!row.must_change, "must_change should be cleared");
    }

    #[test]
    fn audit_event_is_appended() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password");
        change_password_self(
            &db,
            &clock,
            uid,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
        )
        .expect("change");
        let rows = audit::recent(&db, 50).expect("audit");
        assert!(
            rows.iter().any(|r| r.action == "auth.password.changed_self"),
            "expected auth.password.changed_self in audit log; got: {:?}",
            rows.iter().map(|r| r.action.as_str()).collect::<Vec<_>>()
        );
    }
}
