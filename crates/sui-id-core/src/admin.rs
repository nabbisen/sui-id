//! Admin-layer domain functions — split by resource type (RFC 075, v0.62.0).
//!
//! All names that were previously `pub` in the flat `admin.rs` are
//! re-exported here, so callers outside this crate need no changes.

mod users;
mod clients;
mod signing_keys;

pub use users::{
    CreateUserSpec, MfaResetReport,
    create_user, list_users, set_user_disabled,
    delete_user, admin_reset_mfa, reset_user_password,
};
pub use clients::{
    CreatedClient, CreateClientSpec,
    create_client, get_client, list_clients,
    update_client, update_client_basic,
    set_client_allowed_scopes, set_client_post_logout_redirect_uris,
    set_client_disabled, delete_client, rotate_client_secret,
};
pub use signing_keys::{
    list_signing_keys, rotate_signing_key, delete_signing_key,
};

use crate::errors::{CoreError, CoreResult};
use chrono::Utc;
use sui_id_shared::ids::UserId;
use sui_id_store::repos::users as users_repo;
use sui_id_store::{models::AuditLogRow, repos::audit};
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
    let user = users_repo::get(db, user_id).await.map_err(|e| match e {
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

