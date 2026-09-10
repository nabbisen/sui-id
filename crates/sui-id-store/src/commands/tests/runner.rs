use super::*;
use crate::crypto::MasterKey;
use crate::models::{EmailOutboxRow, EmailOutboxState, SessionRow, UserRow};
use crate::repos;
use crate::{Database, StoreError};
use chrono::{TimeDelta, Utc};

fn fresh_db() -> Database {
    Database::open_in_memory(MasterKey::generate()).expect("db")
}

/// A stand-in for the authorizing admin's `UserId` in tests that
/// call `create_user` — not a real `AdminActor` (this crate
/// can't name that type), just the `UserId` its caller-discipline
/// contract asks for.
fn an_admin() -> UserId {
    UserId::new()
}

fn a_user() -> UserRow {
    UserRow {
        id: UserId::new(),
        username: format!("user-{}", uuid::Uuid::new_v4()),
        display_name: None,
        email: None,
        email_normalized: None,
        email_verified_at: None,
        preferred_lang: None,
        is_admin: false,
        role: crate::models::Role::User,
        is_disabled: false,
        is_deleted: false,
        last_login_at: None,
        user_uuid: uuid::Uuid::new_v4(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        failed_login_count: 0,
        locked_until: None,
        source: crate::models::UserSource::Local,
        external_stable_id: None,
    }
}

fn a_client() -> crate::models::ClientRow {
    crate::models::ClientRow {
        id: ClientId::new(),
        name: format!("client-{}", uuid::Uuid::new_v4()),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: crate::models::ConsentPolicy::default(),
        registered_via: crate::models::RegistrationSource::default(),
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

async fn latest_audit_action(db: &Database) -> Option<String> {
    repos::audit::recent(db, 1)
        .await
        .expect("audit tail")
        .into_iter()
        .next()
        .map(|row| row.action)
}

async fn seed_active_session(db: &Database, user_id: UserId) -> sui_id_shared::ids::SessionId {
    let id = sui_id_shared::ids::SessionId::new();
    repos::sessions::insert(
        db,
        &SessionRow {
            id,
            user_id,
            expires_at: Utc::now() + TimeDelta::hours(1),
            created_at: Utc::now(),
            revoked_at: None,
            auth_methods: vec![],
            last_step_up_at: None,
            last_used_at: None,
        },
    )
    .await
    .expect("seed session");
    id
}

mod chain_integrity;
mod key_rotation;
mod lockout;
mod mfa;
mod passwords;
mod policy_markers;
mod refresh;
mod user_admin;
