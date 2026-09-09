//! Core operations behind the `/me/security` self-service surface.
//!
//! These are the *actions* a signed-in user can take on their own
//! account that aren't simple reads (the reads are inlined into the
//! handler since they're cheap query wrappers). Today this is just
//! password change; future entries will land here too — for example
//! a self-serve recovery-email change once email support arrives.

use crate::actor::SelfActor;
use crate::errors::{CoreError, CoreResult};
use crate::hibp::{self, HibpClient, HibpEnforcement};
use crate::password;
use crate::time::SharedClock;
use chrono::Utc;
use sui_id_store::Database;
use sui_id_store::models::{CredentialRow, HibpMode};
use sui_id_store::repos::credentials;

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
    /// `true` when the new password was found in breach data and
    /// `hibp_mode` was `Warn` (the change is still allowed in that case).
    pub hibp_warned: bool,
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
#[allow(clippy::too_many_arguments)]
pub async fn change_password_self(
    db: &Database,
    // Unused since the RFC 094 U09 conversion: the audit timestamp is
    // now `Database::class_a`'s own `chrono::Utc::now()` call, not this
    // caller-injected clock — the tracked, open "registry clock source"
    // gap (`migration-checklist.md`, Stage 2) applies here too and isn't
    // this conversion's to fix. Kept in the signature rather than
    // removed, to avoid an unrelated public-API change and because a
    // future runner-level fix for that gap would need it back.
    _clock: &SharedClock,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: HibpMode,
    actor: &SelfActor,
    current_password: &str,
    new_password: &str,
    keep_current_session: Option<sui_id_shared::ids::SessionId>,
    revoke_others: bool,
    min_password_len: usize,
) -> CoreResult<PasswordChangeReport> {
    let user_id = actor.user_id();
    // 1. Load the existing credential row. If it's missing, the
    //    user account exists without a password (shouldn't happen
    //    in practice, but be explicit) — refuse the same as a
    //    wrong password to avoid an oracle.
    let row = credentials::get(db, user_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::InvalidCredentials,
        other => CoreError::from(other),
    })?;

    // 2. Verify the current password.
    password::verify_password(current_password, &row.password_hash)?;

    // 3. Enforce the policy on the new one. Done after the verify
    //    so that someone fishing for "is X my password?" via this
    //    endpoint doesn't get differentiated errors based on
    //    whether their guess passed policy.
    password::check_password_policy(new_password, min_password_len)?;

    // 3b. RFC 003: HIBP breach check. Enforced here for consistency with
    //     the setup wizard. Fail-open: network errors let the change
    //     through. Block mode returns BadRequest; Warn mode proceeds but
    //     sets the flag in the report so the UI can surface a nudge.
    let hibp_warned = match hibp::enforce_hibp(hibp_mode, hibp_client, new_password).await {
        HibpEnforcement::Blocked { .. } => {
            return Err(CoreError::BadRequest(
                "New password found in known data breaches. Please choose a different password."
                    .into(),
            ));
        }
        HibpEnforcement::AllowedWithWarning { .. } => true,
        _ => false,
    };

    // 4-6. Hash, store, optionally sweep other live state, and append
    // the audit event — all in one Class-A transaction (RFC 094 U09).
    // Previously: an unguarded `credentials::upsert`, two more
    // best-effort revoke calls only reached when `revoke_others` was
    // set, and a fire-and-forget `audit::append` after all of it. A
    // failure between any of these steps used to leave the password
    // changed with some, none, or all of the requested sweep applied,
    // and no audit row recording what actually happened.
    let new_phc = password::hash_password(new_password)?;
    let credential = CredentialRow {
        user_id,
        password_hash: new_phc,
        must_change: false,
        updated_at: Utc::now(),
    };
    let audited = sui_id_store::commands::change_password_self(
        db,
        user_id,
        credential,
        keep_current_session,
        revoke_others,
    )
    .await?;
    let (sessions_revoked, refresh_tokens_revoked) = audited.into_inner();

    Ok(PasswordChangeReport {
        sessions_revoked,
        refresh_tokens_revoked,
        hibp_warned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_id_shared::ids::UserId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::{audit, users};

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    async fn create_user_with_password(db: &Database, password: &str) -> UserId {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "alice".into(),
                display_name: None,
                is_admin: true,
                role: if true {
                    sui_id_store::models::Role::Admin
                } else {
                    sui_id_store::models::Role::User
                },
                last_login_at: None,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                failed_login_count: 0,
                locked_until: None,
                source: sui_id_store::models::UserSource::Local,
                external_stable_id: None,
                email: None,
                preferred_lang: None,
                email_normalized: None,
                email_verified_at: None,
            },
        )
        .await
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
        .await
        .expect("set credential");
        id
    }

    fn self_actor_for(user_id: UserId) -> crate::actor::SelfActor {
        crate::actor::Actor::from_session(
            user_id,
            sui_id_store::models::Role::User,
            sui_id_shared::ids::SessionId::new(),
        )
        .into_self()
    }

    #[tokio::test]
    async fn happy_path_replaces_hash_and_returns_zero_sweep_when_box_unchecked() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password").await;
        let actor = self_actor_for(uid);
        let r = change_password_self(
            &db,
            &clock,
            None,
            sui_id_store::models::HibpMode::Off,
            &actor,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
            crate::security::SecurityLevel::Standard.password_min_len(),
        )
        .await
        .expect("change");
        assert_eq!(r.sessions_revoked, 0);
        assert_eq!(r.refresh_tokens_revoked, 0);
        // Old password no longer verifies; new one does.
        let stored = credentials::get(&db, uid)
            .await
            .expect("cred")
            .password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_err());
        assert!(password::verify_password("the-new-tester-password", &stored).is_ok());
    }

    #[tokio::test]
    async fn wrong_current_password_is_rejected_as_invalid_credentials() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password").await;
        let actor = self_actor_for(uid);
        let r = change_password_self(
            &db,
            &clock,
            None,
            sui_id_store::models::HibpMode::Off,
            &actor,
            "wrong-current-tester-password",
            "the-new-tester-password",
            None,
            false,
            crate::security::SecurityLevel::Standard.password_min_len(),
        )
        .await;
        assert!(matches!(r, Err(CoreError::InvalidCredentials)));
        // Stored hash is unchanged.
        let stored = credentials::get(&db, uid)
            .await
            .expect("cred")
            .password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_ok());
    }

    #[tokio::test]
    async fn weak_new_password_is_rejected_after_current_is_verified() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password").await;
        let actor = self_actor_for(uid);
        let r = change_password_self(
            &db,
            &clock,
            None,
            sui_id_store::models::HibpMode::Off,
            &actor,
            "the-old-tester-password",
            "short",
            None,
            false,
            crate::security::SecurityLevel::Standard.password_min_len(),
        )
        .await;
        assert!(matches!(r, Err(CoreError::BadRequest(_))), "{r:?}");
        // Stored hash unchanged — failure must not partially apply.
        let stored = credentials::get(&db, uid)
            .await
            .expect("cred")
            .password_hash;
        assert!(password::verify_password("the-old-tester-password", &stored).is_ok());
    }

    #[tokio::test]
    async fn must_change_flag_is_reset_on_self_change() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password").await;
        let actor = self_actor_for(uid);
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
        .await
        .expect("upsert");
        change_password_self(
            &db,
            &clock,
            None,
            sui_id_store::models::HibpMode::Off,
            &actor,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
            crate::security::SecurityLevel::Standard.password_min_len(),
        )
        .await
        .expect("change");
        let row = credentials::get(&db, uid).await.expect("cred");
        assert!(!row.must_change, "must_change should be cleared");
    }

    #[tokio::test]
    async fn audit_event_is_appended() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user_with_password(&db, "the-old-tester-password").await;
        let actor = self_actor_for(uid);
        change_password_self(
            &db,
            &clock,
            None,
            sui_id_store::models::HibpMode::Off,
            &actor,
            "the-old-tester-password",
            "the-new-tester-password",
            None,
            false,
            crate::security::SecurityLevel::Standard.password_min_len(),
        )
        .await
        .expect("change");
        let rows = audit::recent(&db, 50).await.expect("audit");
        assert!(
            rows.iter()
                .any(|r| r.action == "auth.password.changed_self"),
            "expected auth.password.changed_self in audit log; got: {:?}",
            rows.iter().map(|r| r.action.as_str()).collect::<Vec<_>>()
        );
    }
}
