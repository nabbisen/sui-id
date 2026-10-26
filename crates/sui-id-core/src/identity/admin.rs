//! Admin-layer domain functions — split by resource type (RFC 075, v0.62.0).
//!
//! All names that were previously `pub` in the flat `admin.rs` are
//! re-exported here, so callers outside this crate need no changes.

#[path = "admin/clients.rs"]
pub mod clients;
#[path = "admin/signing_keys.rs"]
mod signing_keys;
#[path = "admin/users.rs"]
mod users;

pub use clients::{
    CreateClientSpec, CreatedClient, create_client, delete_client, get_client, list_clients,
    rotate_client_secret, set_client_disabled, set_dev_client_secret, update_client_basic,
    update_client_form,
};
pub use signing_keys::{delete_signing_key, list_signing_keys, rotate_signing_key};
pub use users::{
    CreateUserSpec, MfaResetReport, admin_reset_mfa, create_user, delete_user, list_users,
    operator_reset_mfa, set_user_disabled,
};

use crate::errors::{CoreError, CoreResult};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::repos::users as users_repo;

// RFC 094 M2b wave 4 stage 3: `audit_ok`/`audit_with_note`, the
// best-effort audit helpers every command in this module used before
// conversion, are removed here -- this stage converts their last
// callers (C01, C02, C07, K03). Stage 4's remaining rows (C10, C22,
// K02) write through a different path (`clients::update_app_identity`,
// `client_registration_token::revoke`, `signing_keys::retire`), none
// of which reaches this module, so nothing here was left depending on
// them.

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
