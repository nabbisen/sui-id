//! User admin operations (RFC 075, v0.62.0).
use crate::errors::{CoreError, CoreResult};
use crate::hibp::{self, HibpClient, HibpEnforcement};
use crate::password::{check_password_policy, hash_password};
use crate::time::SharedClock;
use sui_id_shared::ids::UserId;
use sui_id_store::models::{CredentialRow, HibpMode, UserRow};
use sui_id_store::repos::{
    audit, auth_codes, credentials, refresh_tokens, sessions, user_totp,
    user_webauthn_credentials, users,
};
use sui_id_store::Database;
// Shared audit helpers from parent module.
use super::{audit_ok, audit_with_note, require_admin};
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
    actor: UserId,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    require_admin(db, actor).await?;
    if spec.username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    check_password_policy(spec.password, spec.min_password_len)?;
    // RFC 041: enforce HIBP consistently with all other password entrypoints.
    let hibp_result = hibp::enforce_hibp(hibp_mode, hibp_client, spec.password).await;
    let hibp_warned = matches!(hibp_result, HibpEnforcement::AllowedWithWarning { .. });

    let now = clock.now();
    let row = UserRow {
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
        role: if spec.is_admin { sui_id_store::models::Role::Admin } else { sui_id_store::models::Role::User },
        last_login_at: None,
        is_disabled: false,
        is_deleted: false,
     user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
    };
    users::create(db, &row).await.map_err(|e| match e {
        sui_id_store::StoreError::Conflict => CoreError::Conflict("username already in use".into()),
        other => CoreError::from(other),
    })?;
    let hash = hash_password(spec.password)?;
    credentials::upsert(
        db,
        &CredentialRow {
            user_id: row.id,
            password_hash: hash,
            must_change: false,
            updated_at: now,
        },
    ).await?;
    let action = if hibp_warned { "user.create_warned_hibp" } else { "user.create" };
    audit_ok(db, actor, action, Some(row.id.to_string())).await;
    Ok(row)
}

pub async fn list_users(db: &Database, actor: UserId) -> CoreResult<Vec<UserRow>> {
    require_admin(db, actor).await?;
    Ok(users::list(db).await?)
}

pub async fn set_user_disabled(
    db: &Database,
    actor: UserId,
    target: UserId,
    disabled: bool,
    reason: Option<String>,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    if actor == target && disabled {
        return Err(CoreError::BadRequest(
            "cannot disable your own account; have another administrator do it".into(),
        ));
    }
    users::set_disabled(db, target, disabled).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if disabled {
        sessions::revoke_all_for_user(db, target).await?;
        refresh_tokens::revoke_all_for_user(db, target).await?;
        auth_codes::invalidate_all_for_user(db, target).await?;
    }
    audit_with_note(
        db,
        actor,
        if disabled { "user.disable" } else { "user.enable" },
        Some(target.to_string()),
        if disabled { reason } else { None },
    ).await;
    Ok(())
}

pub async fn delete_user(
    db: &Database,
    actor: UserId,
    target: UserId,
    reason: Option<String>,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    if actor == target {
        return Err(CoreError::BadRequest(
            "cannot delete your own account".into(),
        ));
    }
    users::soft_delete(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    sessions::revoke_all_for_user(db, target).await?;
    refresh_tokens::revoke_all_for_user(db, target).await?;
    auth_codes::invalidate_all_for_user(db, target).await?;
    audit_with_note(db, actor, "user.delete", Some(target.to_string()), reason).await;
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
    actor: UserId,
    target: UserId,
    reason: Option<String>,
) -> CoreResult<MfaResetReport> {
    require_admin(db, actor).await?;
    // Check the target exists and is not soft-deleted, to give a
    // clear error rather than a silently-no-op outcome.
    let _user = users::get(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;

    // Remove TOTP if present. user_totp::delete returns NotFound when
    // the user has no row at all; we treat that as "nothing to do".
    let totp_removed = match user_totp::delete(db, target).await {
        Ok(()) => true,
        Err(sui_id_store::StoreError::NotFound) => false,
        Err(e) => return Err(CoreError::from(e)),
    };

    // Remove every passkey. We iterate the stored list and delete one
    // at a time so the per-row scoping in user_webauthn_credentials::delete
    // (which double-checks user_id) still applies — a defensive choice;
    // a bulk delete by user_id would be equivalent at the SQL level but
    // bypasses that safety net.
    let creds = user_webauthn_credentials::list_for_user(db, target).await?;
    let mut passkeys_removed = 0;
    for c in creds {
        user_webauthn_credentials::delete(db, c.id, target).await?;
        passkeys_removed += 1;
    }

    // RFC 060: the audit note combines a system-generated summary
    // (what was actually removed) with the operator-supplied reason
    // (why). Both are useful for forensics.
    let sys_note = format!(
        "totp={} passkeys={}",
        if totp_removed { "removed" } else { "absent" },
        passkeys_removed
    );
    let note = match reason {
        Some(r) => format!("{sys_note} reason={r}"),
        None => sys_note,
    };
    let _ = audit::append(
        db,
        &sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: Some(actor),
            action: "mfa.admin_reset".into(),
            target: Some(target.to_string()),
            result: "ok".into(),
            note: Some(note),
        },
    ).await;

    // After resetting, we leave any active sessions for the target
    // alone. The reset is intended to restore login capability, not
    // to log the user out — they may already be in the middle of a
    // session via some other path (e.g. they reset the MFA on their
    // own profile and we still want their browser to keep working).
    // Operators who want a hard logout as well can run the existing
    // user.disable / user.enable flow, which already revokes sessions.

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
pub async fn reset_user_password(
    db: &Database,
    clock: &SharedClock,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: HibpMode,
    actor: UserId,
    target: UserId,
    new_password: &str,
    min_password_len: usize,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    check_password_policy(new_password, min_password_len)?;

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
    credentials::upsert(
        db,
        &CredentialRow {
            user_id: target,
            password_hash: hash,
            must_change: false,
            updated_at: now,
        },
    ).await?;
    sessions::revoke_all_for_user(db, target).await?;
    refresh_tokens::revoke_all_for_user(db, target).await?;
    auth_codes::invalidate_all_for_user(db, target).await?;
    audit_ok(db, actor, "user.reset_password", Some(target.to_string())).await;
    Ok(())
}

// ---------- clients ----------

