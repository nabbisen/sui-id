//! User admin operations (RFC 075, v0.62.0).
use crate::actor::{AdminActor, ReadOnlyAdminActor};
use crate::errors::{CoreError, CoreResult};
use crate::hibp::{self, HibpClient, HibpEnforcement};
use crate::password::{check_password_policy, hash_password};
use crate::time::SharedClock;
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::models::{CredentialRow, HibpMode, UserRow};
use sui_id_store::repos::users;
// Shared audit helpers from parent module.
pub struct CreateUserSpec<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub display_name: Option<&'a str>,
    /// Optional email address. Stored if non-empty, dropped to None
    /// otherwise. The admin form treats it as an optional field; the
    /// setup wizard recommends but does not enforce filling it in.
    pub email: Option<&'a str>,
    pub is_admin: bool,
    /// Effective password minimum length — `PASSWORD_MIN_LEN` in
    /// production, `PASSWORD_MIN_LEN_DEV` when running with `--dev`.
    pub min_password_len: usize,
}

pub async fn create_user(
    db: &Database,
    clock: &SharedClock,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: sui_id_store::models::HibpMode,
    actor: &AdminActor,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    let actor_id = actor.user_id();
    if spec.username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    check_password_policy(spec.password, spec.min_password_len)?;
    // RFC 041: enforce HIBP consistently with all other password entrypoints.
    let hibp_result = hibp::enforce_hibp(hibp_mode, hibp_client, spec.password).await;
    let hibp_warned = matches!(hibp_result, HibpEnforcement::AllowedWithWarning { .. });

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
    let hash = hash_password(spec.password)?;
    let credential = CredentialRow {
        user_id: row.id,
        password_hash: hash,
        must_change: false,
        updated_at: now,
    };
    // RFC 094 U01: mutation, credential insert, and the closed-branch
    // `user.create` / `user.create_warned_hibp` audit event commit in one
    // Class-A transaction — replacing the previous unguarded `users::create`
    // + `credentials::upsert` + fire-and-forget `audit_ok` sequence.
    sui_id_store::commands::create_user(db, actor_id, row.clone(), Some(credential), hibp_warned)
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
        sui_id_store::commands::disable_user(db, actor_id, target, reason).await
    } else {
        sui_id_store::commands::enable_user(db, actor_id, target).await
    };
    result.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

pub async fn delete_user(
    db: &Database,
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
    sui_id_store::commands::delete_user(db, actor_id, target, reason)
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
    let audited = sui_id_store::commands::admin_reset_mfa(db, actor_id, target, reason)
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

/// Reset another user's password (admin-initiated).
///
/// Enforces the same HIBP policy as the setup wizard and self-service
/// password change (RFC 003 consistency requirement). Pass
/// `HibpMode::Off` / `None` to skip the check when HIBP is disabled.
///
/// # Security — RFC 005 LDAP shadow users
///
/// Password reset is blocked for users whose `source` is `Ldap` (or any
/// non-Local source).  Setting a local password on an LDAP shadow row would
/// allow the user to authenticate via the local credential path, bypassing
/// LDAP entirely.  Administrators who need to reset an LDAP user's password
/// must do so in the upstream directory.
#[allow(clippy::too_many_arguments)]
pub async fn reset_user_password(
    db: &Database,
    clock: &SharedClock,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: HibpMode,
    actor: &AdminActor,
    target: sui_id_shared::ids::UserId,
    new_password: &str,
    min_password_len: usize,
) -> CoreResult<()> {
    let actor_id = actor.user_id();
    check_password_policy(new_password, min_password_len)?;

    // RFC 005: block password reset on non-local (e.g. LDAP shadow) users.
    // Setting a local password on a shadow row bypasses the upstream directory.
    let user_row = users::get(db, target).await?;
    if user_row.source != sui_id_store::models::UserSource::Local {
        return Err(CoreError::BadRequest(
            "Cannot set a local password for a user managed by an external user source \
             (e.g. LDAP). Reset the password in the upstream directory instead."
                .into(),
        ));
    }

    // RFC 003: HIBP breach check on admin-driven password reset.
    // Fail-open: network failures let the reset through.
    if matches!(
        hibp::enforce_hibp(hibp_mode, hibp_client, new_password).await,
        HibpEnforcement::Blocked { .. }
    ) {
        return Err(CoreError::BadRequest(
            "New password found in known data breaches. Please choose a different password.".into(),
        ));
    }

    let hash = hash_password(new_password)?;
    let now = clock.now();
    let credential = CredentialRow {
        user_id: target,
        password_hash: hash,
        must_change: false,
        updated_at: now,
    };
    // RFC 094 U06: same atomicity shift as U02/U04 — the credential swap,
    // the target's session/refresh-token/auth-code revocations, and the
    // audit event now commit in one Class-A transaction, replacing the
    // previous unguarded `credentials::upsert` followed by three separate
    // best-effort revoke calls and a fire-and-forget `audit_ok`.
    sui_id_store::commands::reset_user_password(db, actor_id, target, credential)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::Actor;
    use crate::time::system_clock;
    use sui_id_shared::ids::SessionId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::{HibpMode, Role};

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
            None,
            HibpMode::Off,
            &actor,
            CreateUserSpec {
                username: "created",
                password: "created-user-password",
                display_name: None,
                email: None,
                is_admin: false,
                min_password_len: 12,
            },
        )
        .await
        .expect("create user");

        assert_ne!(created.id, actor_id);
    }
}

// ---------- clients ----------
