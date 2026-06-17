//! Initial setup state machine.
//!
//! The system starts "uninitialized": only the setup status endpoint and the
//! single-shot create-initial-admin endpoint should be exposed at the HTTP
//! layer. Once the first admin is created we mark the system initialized;
//! subsequent calls to that endpoint must fail.

use zeroize::Zeroizing;
use crate::errors::{CoreError, CoreResult};
use crate::password::{check_password_policy, hash_password};
use crate::security::SecurityLevel;
use crate::time::SharedClock;
use chrono::Utc;
use ed25519_dalek::SigningKey;
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
pub async fn create_initial_admin(
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
    // Setup always runs at Standard level — dev mode seeds the DB directly
    // via dev_mode.rs and never goes through the setup wizard.
    check_password_policy(password, SecurityLevel::Standard.password_min_len())?;

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
        email_normalized: email
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(sui_id_shared::normalize_email),
        email_verified_at: None,
        // No language preference yet — admin can set one on
        // /me/profile after sign-in. NULL falls through to
        // server_settings.default_lang.
        preferred_lang: None,
        is_admin: true,
        role: sui_id_store::models::Role::Admin,
        last_login_at: None,
        is_disabled: false,
        is_deleted: false,
     user_uuid: uuid::Uuid::new_v4(),
        created_at: now,
        updated_at: now,
        failed_login_count: 0,
        locked_until: None,
    };
    users::create(db, &user).await.map_err(|e| match e {
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
    credentials::upsert(db, &cred).await?;

    // 3. Bootstrap an Ed25519 signing key if one isn't there yet.
    if signing_keys::active(db).await.is_err() {
        // RFC 069: getrandom + from_bytes replaces SigningKey::generate(&mut OsRng).
        // Semantically equivalent: secret key material from OS RNG; memory
        // zeroized on drop via Zeroizing<>.
        let mut secret = Zeroizing::new([0u8; 32]);
        getrandom::fill(secret.as_mut()).expect("system RNG unavailable");
        let sk = SigningKey::from_bytes(&secret);
        let pk = sk.verifying_key();
        signing_keys::insert_with_plaintext(
            db,
            SigningKeyId::new(),
            "EdDSA",
            sk.to_bytes().as_ref(),
            pk.to_bytes().as_ref(),
            true,
        ).await?;
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
    ).await?;

    Ok(CreatedInitialAdmin {
        user_id: user.id,
        username: user.username,
    })
}
