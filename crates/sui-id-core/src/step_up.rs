//! Step-up authentication: requiring a fresh proof of a strong factor
//! before a sensitive action.
//!
//! ## What "step-up" means here
//!
//! A regular session is fine for routine reads (the dashboard, your
//! own profile, the audit log) but should not, on its own, be enough
//! to authorise a destructive or security-relevant action — change a
//! password, revoke every other session at once, delete a client,
//! force-reset another user's MFA, rotate a signing key. The
//! prevailing risk is **session theft**: someone who has stolen a
//! cookie can navigate the UI, but should not be able to immediately
//! ratchet that into permanent damage.
//!
//! Step-up auth closes that gap by requiring a fresh MFA challenge
//! shortly before the sensitive action, regardless of how the
//! session was originally established. The freshness window is
//! short — five minutes by default — so a stolen cookie is useful
//! for almost no destructive purpose.
//!
//! ## What we *don't* do here
//!
//! - **Force step-up on accounts that don't have MFA enrolled.** If
//!   the user's only factor is a password, a "step-up" challenge
//!   would be a password re-prompt — same factor, same theft model,
//!   no security gain, large UX cost. We document this clearly and
//!   leave it to operators (and to this project's own roadmap of
//!   making MFA mandatory) to close the gap by enrolling MFA.
//! - **Per-action policy customisation.** The freshness window is
//!   one number. If you need different windows for different
//!   actions, the better answer is to require MFA for the user
//!   instead of building a fine-grained policy DSL.
//! - **Storing per-action proof tokens.** Some systems mint a
//!   short-lived "this user proved MFA at 14:07; the action
//!   submitted at 14:09 carries the proof token". We instead just
//!   compare `now - session.last_step_up_at` because the session
//!   is the action's authorisation context already, and an extra
//!   token is one more piece of state to lose.

use crate::errors::CoreResult;
use crate::time::SharedClock;
use crate::webauthn;
use chrono::{DateTime, Duration};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::repos::{sessions, user_totp};
use sui_id_store::Database;

/// Default freshness window: a session whose last step-up
/// happened within this many seconds is treated as fresh. Long
/// enough to avoid retyping a TOTP code three times in one
/// admin-cleanup pass, short enough that a session stolen
/// hours after a previous step-up gets re-challenged.
pub const STEP_UP_FRESHNESS_SECS: i64 = 300; // 5 minutes

/// Outcome of `policy_for_session`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepUpDecision {
    /// The session is allowed through without a challenge — either
    /// because the user has no MFA enrolled (so a step-up would
    /// just be a password re-prompt and not a meaningful gate), or
    /// because the session's `last_step_up_at` is within the
    /// freshness window.
    Allow,
    /// The session must complete a step-up challenge before the
    /// caller proceeds. The handler typically responds with a
    /// redirect to `/me/security/step-up?return_to=...`.
    Challenge,
}

/// Decide whether a session can perform a sensitive action right
/// now, or must first complete a step-up challenge.
///
/// The rule:
///
/// - If the user has *no* MFA factor enrolled (no TOTP, no
///   passkeys), allow. Step-up for a password-only account would
///   be a password re-prompt, which buys nothing against an
///   attacker who already has the cookie *and* the password.
/// - Otherwise, require `last_step_up_at >= now - freshness_secs`.
///   Sessions with `last_step_up_at = None` (password-only
///   established, or pre-migration row) always need to challenge.
pub fn policy_for_session(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    last_step_up_at: Option<DateTime<chrono::Utc>>,
    freshness_secs: i64,
) -> CoreResult<StepUpDecision> {
    if !user_has_mfa(db, user_id)? {
        return Ok(StepUpDecision::Allow);
    }
    let last = match last_step_up_at {
        Some(t) => t,
        None => return Ok(StepUpDecision::Challenge),
    };
    let cutoff = clock.now() - Duration::seconds(freshness_secs);
    if last >= cutoff {
        Ok(StepUpDecision::Allow)
    } else {
        Ok(StepUpDecision::Challenge)
    }
}

/// Whether the user has any MFA factor enrolled (TOTP enabled
/// *or* at least one WebAuthn credential).
pub fn user_has_mfa(db: &Database, user_id: UserId) -> CoreResult<bool> {
    let totp = user_totp::get(db, user_id)?
        .map(|r| r.enabled)
        .unwrap_or(false);
    if totp {
        return Ok(true);
    }
    let has_passkey = webauthn::has_credentials(db, user_id)?;
    Ok(has_passkey)
}

/// Mark a session as having just successfully completed a step-up
/// challenge. The caller has *already* verified the second factor
/// (TOTP code or WebAuthn assertion). This function only updates
/// the session row's `last_step_up_at`.
pub fn touch_step_up(
    db: &Database,
    clock: &SharedClock,
    session_id: SessionId,
) -> CoreResult<()> {
    sessions::touch_step_up(db, session_id, clock.now())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password;
    use chrono::Utc;
    use sui_id_shared::ids::UserId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::{CredentialRow, UserRow};
    use sui_id_store::repos::{credentials, user_totp, users};

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    fn create_user(db: &Database) -> UserId {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "u".into(),
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
            },
        )
        .expect("create user");
        let phc = password::hash_password("the-tester-password").expect("hash");
        credentials::upsert(
            db,
            &CredentialRow {
                user_id: id,
                password_hash: phc,
                must_change: false,
                updated_at: now,
            },
        )
        .expect("cred");
        id
    }

    fn enrol_totp(db: &Database, user_id: UserId) {
        // Enrolment goes through pending-then-confirm; for tests we
        // just need an "MFA enrolled" row, so do both halves.
        user_totp::upsert_pending(db, user_id, b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09")
            .expect("upsert pending");
        user_totp::confirm_with_recovery(db, user_id, b"[]").expect("confirm");
    }

    #[test]
    fn user_with_no_mfa_is_always_allowed() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db);
        let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS).unwrap();
        assert_eq!(r, StepUpDecision::Allow);
        let r = policy_for_session(
            &db,
            &clock,
            uid,
            Some(Utc::now() - Duration::days(7)),
            STEP_UP_FRESHNESS_SECS,
        )
        .unwrap();
        assert_eq!(r, StepUpDecision::Allow);
    }

    #[test]
    fn mfa_user_with_no_step_up_must_challenge() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db);
        enrol_totp(&db, uid);
        let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS).unwrap();
        assert_eq!(r, StepUpDecision::Challenge);
    }

    #[test]
    fn mfa_user_with_fresh_step_up_is_allowed() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db);
        enrol_totp(&db, uid);
        let now = clock.now();
        let r = policy_for_session(&db, &clock, uid, Some(now), STEP_UP_FRESHNESS_SECS).unwrap();
        assert_eq!(r, StepUpDecision::Allow);
    }

    #[test]
    fn mfa_user_with_stale_step_up_must_challenge_again() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db);
        enrol_totp(&db, uid);
        let stale = clock.now() - Duration::seconds(STEP_UP_FRESHNESS_SECS + 60);
        let r = policy_for_session(&db, &clock, uid, Some(stale), STEP_UP_FRESHNESS_SECS).unwrap();
        assert_eq!(r, StepUpDecision::Challenge);
    }

    #[test]
    fn touch_step_up_updates_session_row() {
        use sui_id_store::models::SessionRow;
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db);
        let session_id = SessionId::new();
        let now = clock.now();
        sessions::insert(
            &db,
            &SessionRow {
                id: session_id,
                user_id: uid,
                expires_at: now + Duration::hours(8),
                created_at: now,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
                last_step_up_at: None,
            },
        )
        .expect("insert");

        touch_step_up(&db, &clock, session_id).expect("touch");
        let row = sessions::get(&db, session_id).expect("get");
        assert!(row.last_step_up_at.is_some());
    }
}
