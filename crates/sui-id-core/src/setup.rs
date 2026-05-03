//! Initial setup state machine.
//!
//! The system starts "uninitialized": only the setup status endpoint and the
//! single-shot create-initial-admin endpoint should be exposed at the HTTP
//! layer. Once the first admin is created we mark the system initialized;
//! subsequent calls to that endpoint must fail.

use crate::errors::{CoreError, CoreResult};
use crate::password::{check_password_policy, hash_password};
use crate::time::SharedClock;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use sui_id_shared::ids::{SigningKeyId, UserId};
use sui_id_store::models::{CredentialRow, UserRow};
use sui_id_store::repos::{audit, credentials, signing_keys, state, users};
use sui_id_store::Database;
use sui_id_store::models::AuditLogRow;
use subtle::ConstantTimeEq;

pub struct CreatedInitialAdmin {
    pub user_id: UserId,
    pub username: String,
}

/// Create the first administrator and bootstrap a signing key. Fails if the
/// system is already initialized or the supplied setup token is wrong.
pub fn create_initial_admin(
    db: &Database,
    clock: &SharedClock,
    expected_setup_token: &str,
    supplied_setup_token: &str,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    email: Option<&str>,
) -> CoreResult<CreatedInitialAdmin> {
    if state::is_initialized(db)? {
        return Err(CoreError::AlreadyInitialized);
    }

    if !bool::from(supplied_setup_token.as_bytes().ct_eq(expected_setup_token.as_bytes())) {
        return Err(CoreError::Forbidden);
    }

    if username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    check_password_policy(password)?;

    let now = clock.now();

    // 1. Create user.
    let user = UserRow {
        id: UserId::new(),
        username: username.to_owned(),
        display_name: display_name.map(str::to_owned),
        email: email
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        is_admin: true,
        is_disabled: false,
        is_deleted: false,
     user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
    };
    users::create(db, &user).map_err(|e| match e {
        sui_id_store::StoreError::Conflict => CoreError::Conflict("username already in use".into()),
        other => other.into(),
    })?;

    // 2. Persist password hash.
    let hash = hash_password(password)?;
    let cred = CredentialRow {
        user_id: user.id,
        password_hash: hash,
        must_change: false,
        updated_at: now,
    };
    credentials::upsert(db, &cred)?;

    // 3. Bootstrap an Ed25519 signing key if one isn't there yet.
    if signing_keys::active(db).is_err() {
        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let pk = sk.verifying_key();
        signing_keys::insert_with_plaintext(
            db,
            SigningKeyId::new(),
            "EdDSA",
            sk.to_bytes().as_ref(),
            pk.to_bytes().as_ref(),
            true,
        )?;
    }

    // 4. Mark system initialized.
    state::mark_initialized(db)?;

    // 5. Audit log entry.
    audit::append(
        db,
        &AuditLogRow {
            at: Utc::now(),
            actor: Some(user.id),
            action: "setup.create_initial_admin".into(),
            target: Some(user.id.to_string()),
            result: "ok".into(),
            note: None,
        },
    )?;

    Ok(CreatedInitialAdmin {
        user_id: user.id,
        username: user.username,
    })
}
