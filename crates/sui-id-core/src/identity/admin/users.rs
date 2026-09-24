//! User admin operations (RFC 075, v0.62.0).
use crate::actor::{AdminActor, ReadOnlyAdminActor};
use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::models::UserRow;
use sui_id_store::repos::users;
// Shared audit helpers from parent module.
/// What an administrator supplies to create a user. **There is no password
/// field, and there must not be one** (RFC 115 D4): nobody but the account
/// holder ever chooses a password. The account is activated through an
/// administrator-issued recovery link (RFC 103), where the holder sets it.
pub struct CreateUserSpec<'a> {
    pub username: &'a str,
    pub display_name: Option<&'a str>,
    /// Optional email address. Stored if non-empty, dropped to None
    /// otherwise. The admin form treats it as an optional field; the
    /// setup wizard recommends but does not enforce filling it in.
    pub email: Option<&'a str>,
    pub is_admin: bool,
}

pub async fn create_user(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    let actor_id = actor.user_id();
    if spec.username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    let now = clock.now();
    let row = UserRow {
        source: sui_id_store::models::UserSource::default(),
        external_stable_id: None,
        id: UserId::new(),
        username: spec.username.to_owned(),
        display_name: spec.display_name.map(str::to_owned),
        email: spec
            .email
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        email_normalized: spec
            .email
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(sui_id_shared::normalize_email),
        email_verified_at: None,
        // No language preference yet; admin user manages own
        // language on /me/profile.
        preferred_lang: None,
        is_admin: spec.is_admin,
        role: if spec.is_admin {
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
    };
    // RFC 094 U01: the insert and the `user.create` audit event commit in one
    // Class-A transaction. No credential is written (RFC 115 D4).
    sui_id_store::commands::create_user(db, actor_id, row.clone())
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::Conflict => {
                CoreError::Conflict("username already in use".into())
            }
            other => CoreError::from(other),
        })?;
    Ok(row)
}

pub async fn list_users(db: &Database, actor: &ReadOnlyAdminActor) -> CoreResult<Vec<UserRow>> {
    let _ = actor; // capability proof; target list is implicit
    Ok(users::list(db).await?)
}

pub async fn set_user_disabled(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: sui_id_shared::ids::UserId,
    disabled: bool,
    reason: Option<String>,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    if actor_id == target && disabled {
        return Err(CoreError::BadRequest(
            "cannot disable your own account; have another administrator do it".into(),
        ));
    }
    // RFC 094 U02/U03: the flag flip, the target's session/refresh-token/
    // auth-code revocations (disable only), and the audit event commit in
    // one Class-A transaction — replacing the previous unguarded
    // `users::set_disabled` followed by three separate best-effort revoke
    // calls and a fire-and-forget `audit_with_note`.
    let result = if disabled {
        sui_id_store::commands::disable_user(
            db,
            actor_id,
            actor.session_id(),
            target,
            reason,
            clock.now(),
        )
        .await
    } else {
        sui_id_store::commands::enable_user(db, actor_id, actor.session_id(), target, clock.now())
            .await
    };
    result.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

pub async fn delete_user(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: sui_id_shared::ids::UserId,
    reason: Option<String>,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    if actor_id == target {
        return Err(CoreError::BadRequest(
            "cannot delete your own account".into(),
        ));
    }
    // RFC 094 U04: same atomicity shift as U02/U03 above.
    sui_id_store::commands::delete_user(
        db,
        actor_id,
        actor.session_id(),
        target,
        reason,
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

/// Result of a MFA reset, mostly informational so the UI can tell the
/// operator how much it actually removed.
pub struct MfaResetReport {
    /// True if a TOTP enrolment was deleted.
    pub totp_removed: bool,
    /// Number of WebAuthn credentials deleted.
    pub passkeys_removed: usize,
}

/// Forcibly remove every MFA factor for `target`. This is the recovery
/// path operators use when a user has lost access to their TOTP
/// authenticator and recovery codes, and every registered passkey, all
/// at once. Self-service recovery is impossible at that point; an
/// administrator deliberately downgrading the user back to
/// password-only is the only way out.
///
/// The action is privileged and audit-logged: every reset records who
/// reset whose factors and what was removed. Operators reviewing the
/// audit log later should be able to reconstruct exactly what happened.
///
/// We do **not** restrict self-resets — an administrator who has locked
/// themselves out of their own MFA can use this path on themselves
/// provided they still have a valid session, which means the typical
/// case of "lost the second factor outright" still requires another
/// admin to act on their behalf.
pub async fn admin_reset_mfa(
    db: &Database,
    clock: &SharedClock,
    actor: &AdminActor,
    target: sui_id_shared::ids::UserId,
    reason: Option<String>,
) -> CoreResult<MfaResetReport> {
    let actor_id = actor.user_id();
    // RFC 094 U07: the existence check, the TOTP/passkey removal, and the
    // mfa.admin_reset audit event now commit in one Class-A transaction —
    // replacing the previous unguarded delete-then-delete-then-fire-and-
    // forget-append sequence, which also appended via a raw `AuditLogRow`
    // that bypassed this crate's own `AuthorizedCommandContext` entirely.
    //
    // Sessions are still deliberately left alone (unchanged from before
    // this conversion): the reset restores login capability rather than
    // forcing a logout. Operators who want both run `user.disable` /
    // `user.enable` as well.
    let audited = sui_id_store::commands::admin_reset_mfa(
        db,
        actor_id,
        actor.session_id(),
        target,
        reason,
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    let (totp_removed, passkeys_removed) = audited.into_inner();

    Ok(MfaResetReport {
        totp_removed,
        passkeys_removed,
    })
}

/// Remove every MFA factor for `username` on the operator's authority:
/// `sui-id admin reset-mfa` (RFC 103 D12), the recovery for an
/// administrator who lost every factor and has no other administrator to
/// act. The caller proved possession of the host's master key by opening
/// the database; there is no session, so the event has no actor and
/// carries `via = cli`.
///
/// Refuses, writing nothing: an empty reason (`BadRequest`), an unknown
/// user and a deleted user (`NotFound`).
pub async fn operator_reset_mfa(
    db: &Database,
    username: &str,
    reason: &str,
) -> CoreResult<(sui_id_shared::ids::UserId, MfaResetReport)> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(CoreError::BadRequest("a reason is required".into()));
    }
    let user = sui_id_store::repos::users::find_by_username(db, username)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        })?;
    if user.is_deleted {
        return Err(CoreError::NotFound);
    }
    // U07 re-checks the user inside its transaction (a user deleted after
    // this read is `NotFound` there too).
    let audited = sui_id_store::commands::operator_reset_mfa(db, user.id, reason.to_owned())
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        })?;
    let (totp_removed, passkeys_removed) = audited.into_inner();
    Ok((
        user.id,
        MfaResetReport {
            totp_removed,
            passkeys_removed,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use crate::time::system_clock;
    use sui_id_shared::ids::SessionId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::Role;

    fn admin_actor_for(user_id: UserId) -> crate::actor::AdminActor {
        Actor::from_session(user_id, Role::Admin, SessionId::new())
            .into_admin()
            .expect("admin actor")
    }

    #[tokio::test]
    async fn create_user_allocates_fresh_user_id_not_actor_id() {
        let db = Database::open_in_memory(MasterKey::generate()).expect("db");
        let clock = system_clock();
        let actor_id = UserId::new();
        let actor = admin_actor_for(actor_id);

        let created = create_user(
            &db,
            &clock,
            &actor,
            CreateUserSpec {
                username: "created",
                display_name: None,
                email: None,
                is_admin: false,
            },
        )
        .await
        .expect("create user");

        assert_ne!(created.id, actor_id);
        // RFC 115 D4: no password is chosen at creation, so no credential row.
        assert!(matches!(
            sui_id_store::repos::credentials::get(&db, created.id).await,
            Err(sui_id_store::StoreError::NotFound)
        ));
    }
}

// ---------- clients ----------
