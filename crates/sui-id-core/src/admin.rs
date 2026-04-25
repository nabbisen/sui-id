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
use sui_id_store::repos::{audit, clients, credentials, refresh_tokens, sessions, users};
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
        created_at: now,
        updated_at: now,
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

pub fn create_client(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    name: &str,
    redirect_uris: &[String],
    confidential: bool,
) -> CoreResult<CreatedClient> {
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

    let secret_plain = if confidential {
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
        name: name.to_owned(),
        confidential,
        secret_hash,
        redirect_uris: redirect_uris.to_vec(),
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
