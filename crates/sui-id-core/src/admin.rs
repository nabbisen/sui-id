//! Admin-layer domain functions — split by resource type (RFC 075, v0.62.0).
//!
//! All names that were previously `pub` in the flat `admin.rs` are
//! re-exported here, so callers outside this crate need no changes.

pub mod clients;
mod signing_keys;
mod users;

pub use clients::{
    CreateClientSpec, CreatedClient, create_client, delete_client, get_client, list_clients,
    rotate_client_secret, set_client_allowed_scopes, set_client_disabled,
    set_client_post_logout_redirect_uris, update_client, update_client_basic,
};
pub use signing_keys::{delete_signing_key, list_signing_keys, rotate_signing_key};
pub use users::{
    CreateUserSpec, MfaResetReport, admin_reset_mfa, create_user, delete_user, list_users,
    reset_user_password, set_user_disabled,
};

use crate::errors::{CoreError, CoreResult};
use chrono::Utc;
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::repos::users as users_repo;
use sui_id_store::{models::AuditLogRow, repos::audit};

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
    )
    .await;
}

/// Check if a user has admin rights by fetching their record.
///
/// **Deprecated by RFC 081.** New code must use [`crate::actor::Actor::into_admin`]
/// instead; the capability type provides compile-time proof of admin privilege.
/// This function is retained for the binary crate's Axum extractors, which need
/// to verify session roles before constructing an `Actor`.
#[deprecated(
    since = "0.66.0",
    note = "Use Actor::into_admin (RFC 081) for domain functions"
)]
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
