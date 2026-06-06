//! Admin-side use cases.
//!
//! These functions are higher-level than the storage repos: they enforce
//! domain rules ("only an admin may suspend a user", "client deletion must
//! also revoke its outstanding refresh tokens") and emit audit log entries.

use crate::errors::{CoreError, CoreResult};
use crate::hibp::{self, HibpClient, HibpEnforcement};
use crate::password::{check_password_policy, hash_password};
use crate::time::SharedClock;
use crate::tokens;
use chrono::Utc;
use sui_id_shared::ids::{ClientId, UserId};
use sui_id_store::models::{AuditLogRow, ClientRow, CredentialRow, HibpMode, UserRow};
use sui_id_store::repos::{
    audit, auth_codes, clients, credentials, refresh_tokens, sessions, user_totp,
    user_webauthn_credentials, users,
};
use crate::cache::Caches;
use sui_id_store::Database;

async fn audit_ok(db: &Database, actor: UserId, action: &str, target: Option<String>) {
    audit_with_note(db, actor, action, target, None).await;
}

async fn audit_with_note(
    db: &Database,
    actor: UserId,
    action: &str,
    target: Option<String>,
    note: Option<String>,
) {
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: Utc::now(),
            actor: Some(actor),
            action: action.to_owned(),
            target,
            result: "ok".into(),
            note,
        },
    ).await;
}

pub async fn require_admin(db: &Database, user_id: UserId) -> CoreResult<()> {
    let user = users::get(db, user_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Forbidden,
        other => CoreError::from(other),
    })?;
    if user.is_admin && !user.is_disabled && !user.is_deleted {
        Ok(())
    } else {
        Err(CoreError::Forbidden)
    }
}

// ---------- users ----------

pub struct CreateUserSpec<'a> {
    pub username: &'a str,
    pub password: &'a str,
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
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: sui_id_store::models::HibpMode,
    actor: UserId,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    require_admin(db, actor).await?;
    if spec.username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    check_password_policy(spec.password)?;
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
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    check_password_policy(new_password)?;

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

pub struct CreatedClient {
    pub row: ClientRow,
    pub generated_secret: Option<String>,
}

/// Input to `create_client`. Supplied as a single struct so the call site
/// reads as a labelled record rather than a long positional list.
pub struct CreateClientSpec<'a> {
    pub name: &'a str,
    pub redirect_uris: &'a [String],
    pub confidential: bool,
    /// Space-separated list of allowed scopes. Empty → permit any scope.
    pub allowed_scopes: &'a str,
    /// RP-initiated logout return URIs. Empty → fall back to
    /// `redirect_uris` at logout time (with a deprecation warning logged).
    pub post_logout_redirect_uris: &'a [String],
}

pub async fn create_client(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    spec: CreateClientSpec<'_>,
    _caches: &Caches,
) -> CoreResult<CreatedClient> {
    require_admin(db, actor).await?;
    if spec.name.trim().is_empty() {
        return Err(CoreError::BadRequest("client name must not be empty".into()));
    }
    if spec.redirect_uris.is_empty() {
        return Err(CoreError::BadRequest(
            "at least one redirect_uri must be provided".into(),
        ));
    }
    for uri in spec.redirect_uris {
        validate_redirect_uri(uri)?;
    }
    for uri in spec.post_logout_redirect_uris {
        validate_redirect_uri(uri)?;
    }
    // Empty scope policy is allowed (means "permit any") — but if a list
    // is given, sanity-check that scope tokens look reasonable. RFC 6749
    // §3.3 restricts scope tokens to a printable subset.
    for tok in spec.allowed_scopes.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(CoreError::BadRequest(format!(
                "invalid character in scope token {tok:?}"
            )));
        }
    }

    let secret_plain = if spec.confidential {
        Some(tokens::random_token(32))
    } else {
        None
    };
    let secret_hash = match secret_plain.as_deref() {
        Some(s) => Some(hash_password(s)?),
        None => None,
    };

    let now = clock.now();
    let row = ClientRow {
        id: ClientId::new(),
        name: spec.name.to_owned(),
        confidential: spec.confidential,
        secret_hash,
        redirect_uris: spec.redirect_uris.to_vec(),
        allowed_scopes: spec.allowed_scopes.to_owned(),
        post_logout_redirect_uris: spec.post_logout_redirect_uris.to_vec(),
        is_disabled: false,
        is_deleted: false,
        consent_policy: sui_id_store::models::ConsentPolicy::default(),
        created_at: now,
        updated_at: now,
    };
    clients::create(db, &row).await?;
    audit_ok(db, actor, "client.create", Some(row.id.to_string())).await;
    Ok(CreatedClient {
        row,
        generated_secret: secret_plain,
    })
}

/// Update the per-client scope policy. Empty string means "permit any".
pub async fn set_client_allowed_scopes(
    db: &Database,
    actor: UserId,
    target: ClientId,
    scopes: &str,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    for tok in scopes.split_whitespace() {
        if !tok
            .chars()
            .all(|c| c == '!' || ('#'..='[').contains(&c) || (']'..='~').contains(&c))
        {
            return Err(CoreError::BadRequest(format!(
                "invalid character in scope token {tok:?}"
            )));
        }
    }
    clients::set_allowed_scopes(db, target, scopes).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor, "client.set_allowed_scopes", Some(target.to_string())).await;
    Ok(())
}

/// Replace the `post_logout_redirect_uris` for a client.
pub async fn set_client_post_logout_redirect_uris(
    db: &Database,
    actor: UserId,
    target: ClientId,
    uris: &[String],
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    for uri in uris {
        validate_redirect_uri(uri)?;
    }
    clients::set_post_logout_redirect_uris(db, target, uris).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(
        db,
        actor,
        "client.set_post_logout_redirect_uris",
        Some(target.to_string()),
    ).await;
    Ok(())
}

/// Update the basic client metadata: human-readable name and the
/// authorization redirect URIs. The id, type (confidential vs public),
/// and `secret_hash` are immutable.
pub async fn update_client_basic(
    db: &Database,
    actor: UserId,
    target: ClientId,
    name: &str,
    redirect_uris: &[String],
    caches: &Caches,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    if name.trim().is_empty() {
        return Err(CoreError::BadRequest("client name must not be empty".into()));
    }
    if redirect_uris.is_empty() {
        return Err(CoreError::BadRequest(
            "at least one redirect_uri must be provided".into(),
        ));
    }
    for uri in redirect_uris {
        validate_redirect_uri(uri)?;
    }
    clients::update_basic(db, target, Some(name.trim()), Some(redirect_uris)).await.map_err(|e| {
        match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        }
    })?;
    audit_ok(db, actor, "client.update", Some(target.to_string())).await;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after update_client");
    }
    Ok(())
}

/// Convenience: fetch a single client (admin-gated).
pub async fn get_client(db: &Database, actor: UserId, target: ClientId) -> CoreResult<ClientRow> {
    require_admin(db, actor).await?;
    clients::get(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })
}

pub async fn list_clients(db: &Database, actor: UserId) -> CoreResult<Vec<ClientRow>> {
    require_admin(db, actor).await?;
    Ok(clients::list(db).await?)
}

pub async fn update_client(
    db: &Database,
    actor: UserId,
    target: ClientId,
    name: Option<&str>,
    redirect_uris: Option<&[String]>,
    _caches: &Caches,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    if let Some(uris) = redirect_uris {
        if uris.is_empty() {
            return Err(CoreError::BadRequest(
                "at least one redirect_uri must remain".into(),
            ));
        }
        for u in uris {
            validate_redirect_uri(u)?;
        }
    }
    clients::update_basic(db, target, name, redirect_uris).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor, "client.update", Some(target.to_string())).await;
    Ok(())
}

pub async fn set_client_disabled(
    db: &Database,
    _clock: &SharedClock,
    actor: UserId,
    target: ClientId,
    disabled: bool,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    clients::set_disabled(db, target, disabled).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if disabled {
        refresh_tokens::revoke_all_for_client(db, target).await?;
    }
    audit_with_note(
        db,
        actor,
        if disabled { "client.disable" } else { "client.enable" },
        Some(target.to_string()),
        if disabled { reason } else { None },
    ).await;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after set_client_disabled");
    }
    Ok(())
}

pub async fn delete_client(
    db: &Database,
    actor: UserId,
    target: ClientId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    clients::soft_delete(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if let Err(e) = caches.redirect_origins.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after delete_client");
    }
    refresh_tokens::revoke_all_for_client(db, target).await?;
    audit_with_note(db, actor, "client.delete", Some(target.to_string()), reason).await;
    Ok(())
}

// ---------- signing keys ----------

pub async fn list_signing_keys(
    db: &Database,
    actor: UserId,
) -> CoreResult<Vec<sui_id_store::models::SigningKeyRow>> {
    require_admin(db, actor).await?;
    Ok(sui_id_store::repos::signing_keys::list_published(db).await?)
}

/// Generate a fresh Ed25519 signing key, persist it as the new active key,
/// and retire the previous one. The previous key's row stays in the table
/// (and therefore in JWKS) so that tokens already issued under it can still
/// be verified during their lifetime — a "grace window" of one access-token
/// lifetime is sufficient. The retired key can be deleted afterwards by an
/// administrator.
///
/// Returns the new key id.
pub async fn rotate_signing_key(
    db: &Database,
    clock: &SharedClock,
    keyring_path: &str,
    actor: UserId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<sui_id_shared::ids::SigningKeyId> {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use sui_id_shared::ids::SigningKeyId;
    use sui_id_store::repos::signing_keys;

    require_admin(db, actor).await?;

    // Generate the new key material first (outside the DB lock).
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let pk = sk.verifying_key();
    let new_id = SigningKeyId::new();

    // Delegate the retire-then-insert to the store layer. Migration 0021
    // adds a partial unique index (at most one is_active=1 row), so the
    // old insert-then-retire order would violate the constraint. The new
    // order retires first and inserts second inside one transaction.
    signing_keys::rotate_atomic(
        db,
        new_id,
        "EdDSA",
        sk.to_bytes().as_ref(),
        pk.to_bytes().as_ref(),
    ).await?;
    if let Err(e) = caches.jwks.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after rotate_signing_key");
    }
    let _ = clock;
    let _ = keyring_path;
    audit_with_note(db, actor, "signing_key.rotate", Some(new_id.to_string()), reason).await;
    Ok(new_id)
}

/// Permanently delete a retired signing key. Refuses to delete the
/// currently active key.
pub async fn delete_signing_key(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    target: sui_id_shared::ids::SigningKeyId,
    reason: Option<String>,
    caches: &Caches,
) -> CoreResult<()> {
    require_admin(db, actor).await?;
    sui_id_store::repos::signing_keys::delete(db, target).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        sui_id_store::StoreError::Conflict => CoreError::Conflict(
            "cannot delete the active signing key; rotate first".into(),
        ),
        other => CoreError::from(other),
    })?;
    let _ = clock;
    audit_with_note(db, actor, "signing_key.delete", Some(target.to_string()), reason).await;
    if let Err(e) = caches.jwks.rebuild(db).await {
        tracing::warn!(error = %e, "cache rebuild failed after delete_signing_key");
    }
    Ok(())
}

fn validate_redirect_uri(uri: &str) -> CoreResult<()> {
    let parsed = url::Url::parse(uri).map_err(|_| {
        CoreError::BadRequest(format!("redirect_uri is not a valid URL: {uri}"))
    })?;
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or("");
    // Permit https everywhere; permit http only on loopback addresses for
    // local development.
    let ok = match scheme {
        "https" => true,
        "http" => matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1"),
        _ => false,
    };
    if !ok {
        return Err(CoreError::BadRequest(format!(
            "redirect_uri must use https (http permitted only on loopback): {uri}"
        )));
    }
    if parsed.fragment().is_some() {
        return Err(CoreError::BadRequest(
            "redirect_uri must not contain a fragment".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_redirect_is_accepted() {
        validate_redirect_uri("https://app.example.com/callback").expect("ok");
    }

    #[test]
    fn http_loopback_is_accepted() {
        validate_redirect_uri("http://localhost:8080/cb").expect("ok");
        validate_redirect_uri("http://127.0.0.1/cb").expect("ok");
    }

    #[test]
    fn http_non_loopback_is_rejected() {
        let r = validate_redirect_uri("http://example.com/cb");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }

    #[test]
    fn fragment_is_rejected() {
        let r = validate_redirect_uri("https://x/cb#frag");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }

    #[test]
    fn non_http_scheme_is_rejected() {
        let r = validate_redirect_uri("javascript:alert(1)");
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
    }
}

/// Rotate the client secret for a confidential client.
///
/// Returns the new plaintext secret (shown to the operator once).
/// The plaintext is never stored — only the Argon2id hash is persisted.
/// Returns `Err(CoreError::BadRequest)` if the client is a public client.
pub async fn rotate_client_secret(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    client_id: ClientId,
    reason: Option<String>,
) -> CoreResult<String> {
    require_admin(db, actor).await?;
    let client = clients::get(db, client_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if !client.confidential {
        return Err(CoreError::BadRequest(
            "cannot rotate secret for a public (PKCE-only) client".into(),
        ));
    }
    let new_secret = tokens::random_token(32);
    let new_hash = crate::password::hash_password(&new_secret)?;
    clients::set_secret_hash(db, client_id, Some(&new_hash), clock.now()).await
        .map_err(CoreError::from)?;
    audit_with_note(
        db, actor, "client.rotate_secret", Some(client_id.to_string()), reason
    ).await;
    Ok(new_secret)
}
