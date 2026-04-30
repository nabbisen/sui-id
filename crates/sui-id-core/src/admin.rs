//! Admin-side use cases.
//!
//! These functions are higher-level than the storage repos: they enforce
//! domain rules ("only an admin may suspend a user", "client deletion must
//! also revoke its outstanding refresh tokens") and emit audit log entries.

use crate::errors::{CoreError, CoreResult};
use crate::password::{check_password_policy, hash_password};
use crate::time::SharedClock;
use crate::tokens;
use chrono::Utc;
use sui_id_shared::ids::{ClientId, UserId};
use sui_id_store::models::{AuditLogRow, ClientRow, CredentialRow, UserRow};
use sui_id_store::repos::{
    audit, clients, credentials, refresh_tokens, sessions, user_totp, user_webauthn_credentials,
    users,
};
use sui_id_store::Database;

fn audit_ok(db: &Database, actor: UserId, action: &str, target: Option<String>) {
    let _ = audit::append(
        db,
        &AuditLogRow {
            at: Utc::now(),
            actor: Some(actor),
            action: action.to_owned(),
            target,
            result: "ok".into(),
            note: None,
        },
    );
}

pub fn require_admin(db: &Database, user_id: UserId) -> CoreResult<()> {
    let user = users::get(db, user_id).map_err(|e| match e {
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
    pub is_admin: bool,
}

pub fn create_user(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    require_admin(db, actor)?;
    if spec.username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    check_password_policy(spec.password)?;

    let now = clock.now();
    let row = UserRow {
        id: UserId::new(),
        username: spec.username.to_owned(),
        display_name: spec.display_name.map(str::to_owned),
        is_admin: spec.is_admin,
        is_disabled: false,
        is_deleted: false,
     user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
    };
    users::create(db, &row).map_err(|e| match e {
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
    )?;
    audit_ok(db, actor, "user.create", Some(row.id.to_string()));
    Ok(row)
}

pub fn list_users(db: &Database, actor: UserId) -> CoreResult<Vec<UserRow>> {
    require_admin(db, actor)?;
    Ok(users::list(db)?)
}

pub fn set_user_disabled(
    db: &Database,
    actor: UserId,
    target: UserId,
    disabled: bool,
) -> CoreResult<()> {
    require_admin(db, actor)?;
    if actor == target && disabled {
        return Err(CoreError::BadRequest(
            "cannot disable your own account; have another administrator do it".into(),
        ));
    }
    users::set_disabled(db, target, disabled).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if disabled {
        sessions::revoke_all_for_user(db, target)?;
        refresh_tokens::revoke_all_for_user(db, target)?;
    }
    audit_ok(
        db,
        actor,
        if disabled { "user.disable" } else { "user.enable" },
        Some(target.to_string()),
    );
    Ok(())
}

pub fn delete_user(db: &Database, actor: UserId, target: UserId) -> CoreResult<()> {
    require_admin(db, actor)?;
    if actor == target {
        return Err(CoreError::BadRequest(
            "cannot delete your own account".into(),
        ));
    }
    users::soft_delete(db, target).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    sessions::revoke_all_for_user(db, target)?;
    refresh_tokens::revoke_all_for_user(db, target)?;
    audit_ok(db, actor, "user.delete", Some(target.to_string()));
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
pub fn admin_reset_mfa(
    db: &Database,
    actor: UserId,
    target: UserId,
) -> CoreResult<MfaResetReport> {
    require_admin(db, actor)?;
    // Check the target exists and is not soft-deleted, to give a
    // clear error rather than a silently-no-op outcome.
    let _user = users::get(db, target).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;

    // Remove TOTP if present. user_totp::delete returns NotFound when
    // the user has no row at all; we treat that as "nothing to do".
    let totp_removed = match user_totp::delete(db, target) {
        Ok(()) => true,
        Err(sui_id_store::StoreError::NotFound) => false,
        Err(e) => return Err(CoreError::from(e)),
    };

    // Remove every passkey. We iterate the stored list and delete one
    // at a time so the per-row scoping in user_webauthn_credentials::delete
    // (which double-checks user_id) still applies — a defensive choice;
    // a bulk delete by user_id would be equivalent at the SQL level but
    // bypasses that safety net.
    let creds = user_webauthn_credentials::list_for_user(db, target)?;
    let mut passkeys_removed = 0;
    for c in creds {
        user_webauthn_credentials::delete(db, c.id, target)?;
        passkeys_removed += 1;
    }

    let note = format!(
        "totp={} passkeys={}",
        if totp_removed { "removed" } else { "absent" },
        passkeys_removed
    );
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
    );

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

pub fn reset_user_password(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    target: UserId,
    new_password: &str,
) -> CoreResult<()> {
    require_admin(db, actor)?;
    check_password_policy(new_password)?;
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
    )?;
    sessions::revoke_all_for_user(db, target)?;
    refresh_tokens::revoke_all_for_user(db, target)?;
    audit_ok(db, actor, "user.reset_password", Some(target.to_string()));
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

pub fn create_client(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    spec: CreateClientSpec<'_>,
) -> CoreResult<CreatedClient> {
    require_admin(db, actor)?;
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
        created_at: now,
        updated_at: now,
    };
    clients::create(db, &row)?;
    audit_ok(db, actor, "client.create", Some(row.id.to_string()));
    Ok(CreatedClient {
        row,
        generated_secret: secret_plain,
    })
}

/// Update the per-client scope policy. Empty string means "permit any".
pub fn set_client_allowed_scopes(
    db: &Database,
    actor: UserId,
    target: ClientId,
    scopes: &str,
) -> CoreResult<()> {
    require_admin(db, actor)?;
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
    clients::set_allowed_scopes(db, target, scopes).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor, "client.set_allowed_scopes", Some(target.to_string()));
    Ok(())
}

/// Replace the `post_logout_redirect_uris` for a client.
pub fn set_client_post_logout_redirect_uris(
    db: &Database,
    actor: UserId,
    target: ClientId,
    uris: &[String],
) -> CoreResult<()> {
    require_admin(db, actor)?;
    for uri in uris {
        validate_redirect_uri(uri)?;
    }
    clients::set_post_logout_redirect_uris(db, target, uris).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(
        db,
        actor,
        "client.set_post_logout_redirect_uris",
        Some(target.to_string()),
    );
    Ok(())
}

/// Update the basic client metadata: human-readable name and the
/// authorization redirect URIs. The id, type (confidential vs public),
/// and `secret_hash` are immutable.
pub fn update_client_basic(
    db: &Database,
    actor: UserId,
    target: ClientId,
    name: &str,
    redirect_uris: &[String],
) -> CoreResult<()> {
    require_admin(db, actor)?;
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
    clients::update_basic(db, target, Some(name.trim()), Some(redirect_uris)).map_err(|e| {
        match e {
            sui_id_store::StoreError::NotFound => CoreError::NotFound,
            other => CoreError::from(other),
        }
    })?;
    audit_ok(db, actor, "client.update", Some(target.to_string()));
    Ok(())
}

/// Convenience: fetch a single client (admin-gated).
pub fn get_client(db: &Database, actor: UserId, target: ClientId) -> CoreResult<ClientRow> {
    require_admin(db, actor)?;
    clients::get(db, target).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })
}

pub fn list_clients(db: &Database, actor: UserId) -> CoreResult<Vec<ClientRow>> {
    require_admin(db, actor)?;
    Ok(clients::list(db)?)
}

pub fn update_client(
    db: &Database,
    actor: UserId,
    target: ClientId,
    name: Option<&str>,
    redirect_uris: Option<&[String]>,
) -> CoreResult<()> {
    require_admin(db, actor)?;
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
    clients::update_basic(db, target, name, redirect_uris).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor, "client.update", Some(target.to_string()));
    Ok(())
}

pub fn set_client_disabled(
    db: &Database,
    actor: UserId,
    target: ClientId,
    disabled: bool,
) -> CoreResult<()> {
    require_admin(db, actor)?;
    clients::set_disabled(db, target, disabled).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    if disabled {
        refresh_tokens::revoke_all_for_client(db, target)?;
    }
    audit_ok(
        db,
        actor,
        if disabled { "client.disable" } else { "client.enable" },
        Some(target.to_string()),
    );
    Ok(())
}

pub fn delete_client(db: &Database, actor: UserId, target: ClientId) -> CoreResult<()> {
    require_admin(db, actor)?;
    clients::soft_delete(db, target).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    refresh_tokens::revoke_all_for_client(db, target)?;
    audit_ok(db, actor, "client.delete", Some(target.to_string()));
    Ok(())
}

// ---------- signing keys ----------

pub fn list_signing_keys(
    db: &Database,
    actor: UserId,
) -> CoreResult<Vec<sui_id_store::models::SigningKeyRow>> {
    require_admin(db, actor)?;
    Ok(sui_id_store::repos::signing_keys::list_published(db)?)
}

/// Generate a fresh Ed25519 signing key, persist it as the new active key,
/// and retire the previous one. The previous key's row stays in the table
/// (and therefore in JWKS) so that tokens already issued under it can still
/// be verified during their lifetime — a "grace window" of one access-token
/// lifetime is sufficient. The retired key can be deleted afterwards by an
/// administrator.
///
/// Returns the new key id.
pub fn rotate_signing_key(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
) -> CoreResult<sui_id_shared::ids::SigningKeyId> {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use sui_id_shared::ids::SigningKeyId;
    use sui_id_store::repos::signing_keys;

    require_admin(db, actor)?;
    let previous = signing_keys::active(db).ok();

    // Generate the new key.
    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let pk = sk.verifying_key();
    let new_id = SigningKeyId::new();
    signing_keys::insert_with_plaintext(
        db,
        new_id,
        "EdDSA",
        sk.to_bytes().as_ref(),
        pk.to_bytes().as_ref(),
        true,
    )?;

    // Retire the previous active key, if any. We do this *after* the new
    // key is in place so that — even with a crash mid-flight — there is
    // never a window with zero active keys.
    if let Some(prev) = previous {
        signing_keys::retire(db, prev.id)?;
    }
    let _ = clock;
    audit_ok(db, actor, "signing_key.rotate", Some(new_id.to_string()));
    Ok(new_id)
}

/// Permanently delete a retired signing key. Refuses to delete the
/// currently active key.
pub fn delete_signing_key(
    db: &Database,
    actor: UserId,
    target: sui_id_shared::ids::SigningKeyId,
) -> CoreResult<()> {
    require_admin(db, actor)?;
    sui_id_store::repos::signing_keys::delete(db, target).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        sui_id_store::StoreError::Conflict => CoreError::Conflict(
            "cannot delete the active signing key; rotate first".into(),
        ),
        other => CoreError::from(other),
    })?;
    audit_ok(db, actor, "signing_key.delete", Some(target.to_string()));
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
