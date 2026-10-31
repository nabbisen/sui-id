//! Initial setup state machine.
//!
//! The system starts "uninitialized": only the setup status endpoint and the
//! single-shot create-initial-admin endpoint should be exposed at the HTTP
//! layer. Once the first admin is created we mark the system initialized;
//! subsequent calls to that endpoint must fail.

use crate::errors::{CoreError, CoreResult};
use crate::password::{check_password_policy, hash_password};
use crate::security::SecurityLevel;
use crate::time::SharedClock;
use ed25519_dalek::SigningKey;
use subtle::ConstantTimeEq;
use sui_id_shared::ids::{SigningKeyId, UserId};
use sui_id_store::Database;
use sui_id_store::commands::{self, InitialSigningKeyMaterial, SetupAuthorizedBy};
use sui_id_store::models::{CredentialRow, UserRow};
#[cfg(test)]
use sui_id_store::repos::credentials;
use sui_id_store::repos::{signing_keys, state};
use zeroize::Zeroizing;

pub struct CreatedInitialAdmin {
    pub user_id: UserId,
    pub username: String,
}

/// Create the first administrator and bootstrap a signing key. Fails if the
/// system is already initialized or the supplied setup token is wrong.
///
/// This is the **web wizard** path: the setup token printed at boot must
/// match (constant-time comparison) because the endpoint is
/// network-reachable before the instance has an owner.
#[allow(clippy::too_many_arguments)]
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

    if !bool::from(
        supplied_setup_token
            .as_bytes()
            .ct_eq(expected_setup_token.as_bytes()),
    ) {
        return Err(CoreError::Forbidden);
    }

    create_initial_admin_inner(
        db,
        clock,
        username,
        password,
        display_name,
        email,
        SetupAuthorizedBy::SetupToken,
    )
    .await
}

/// Create the first administrator from the CLI (RFC 077) — **headless** path.
///
/// No setup token: this path requires filesystem access to the database
/// and master key, and an actor with that access already controls the
/// instance. (Same trust model as the `admin unlock-user` subcommand.)
///
/// There is no "rotate after first login" flag (RFC 115 D12): the column that
/// pretended to record one was read by nothing and is gone. A password the
/// operator generated is printed once; it is the operator's to change.
pub async fn create_initial_admin_headless(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    email: Option<&str>,
) -> CoreResult<CreatedInitialAdmin> {
    if state::is_initialized(db)? {
        return Err(CoreError::AlreadyInitialized);
    }

    create_initial_admin_inner(
        db,
        clock,
        username,
        password,
        display_name,
        email,
        SetupAuthorizedBy::Filesystem,
    )
    .await
}

/// Generate a random initial-admin password: 24 chars from `[A-Za-z0-9]`
/// (≈143 bits of entropy) using the OS RNG — the same primitive that
/// generates signing keys. Rejection sampling avoids modulo bias.
///
/// Returned in `Zeroizing` so the plaintext is wiped from memory when the
/// caller drops it after printing.
#[allow(clippy::expect_used)]
pub fn generate_admin_password() -> Zeroizing<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const LEN: usize = 24;
    // Rejection threshold: largest multiple of 62 below 256.
    const LIMIT: u8 = (256 / ALPHABET.len() * ALPHABET.len() - 1) as u8; // 247

    let mut out = Zeroizing::new(String::with_capacity(LEN));
    let mut buf = Zeroizing::new([0u8; 64]);
    while out.len() < LEN {
        getrandom::fill(buf.as_mut()).expect("system RNG unavailable");
        for &b in buf.iter() {
            if out.len() == LEN {
                break;
            }
            if b <= LIMIT {
                out.push(ALPHABET[(b as usize) % ALPHABET.len()] as char);
            }
        }
    }
    out
}

/// Shared body for both setup paths: user row, credential, signing-key
/// bootstrap, initialized flag and the `admin.setup.completed` event, all
/// committed in one transaction by `commands::complete_initial_setup`
/// (S10). Callers have already done their path-specific authorization
/// (token check / filesystem trust) and the early, cheap
/// already-initialized check — the authoritative one is the command's
/// own, inside its transaction.
#[allow(clippy::too_many_arguments)]
async fn create_initial_admin_inner(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    email: Option<&str>,
    authorized_by: SetupAuthorizedBy,
) -> CoreResult<CreatedInitialAdmin> {
    if username.trim().is_empty() {
        return Err(CoreError::BadRequest("username must not be empty".into()));
    }
    // Setup always runs at Standard level — dev mode seeds the DB directly
    // via dev_mode.rs and never goes through the setup wizard.
    check_password_policy(password, SecurityLevel::Standard.password_min_len())?;

    let now = clock.now();

    let user = UserRow {
        source: sui_id_store::models::UserSource::default(),
        external_stable_id: None,
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

    // The Argon2 hash is deliberately slow (RFC 126) and must not run
    // inside a write transaction — computed here, before the command.
    let hash = hash_password(password).await?;
    let cred = CredentialRow {
        user_id: user.id,
        password_hash: hash,
        updated_at: now,
    };

    // Signing-key material, generated and sealed unconditionally before
    // the transaction (RFC 069: getrandom + from_bytes, OS RNG; RFC 094:
    // crypto work stays outside the write lock) — the command decides,
    // inside its transaction, whether a key is actually needed, and
    // discards this material otherwise. Generating it is cheap, unlike
    // `hash_password` above.
    let mut secret = Zeroizing::new([0u8; 32]);
    getrandom::fill(secret.as_mut()).map_err(|_| CoreError::Internal)?;
    let sk = SigningKey::from_bytes(&secret);
    let pk = sk.verifying_key();
    let signing_key = InitialSigningKeyMaterial {
        id: SigningKeyId::new(),
        algorithm: "EdDSA",
        private_key_sealed: signing_keys::seal_private_key(db, sk.to_bytes().as_ref())?,
        public_key: pk.to_bytes().to_vec(),
    };

    commands::complete_initial_setup(db, user.clone(), cred, signing_key, authorized_by, now)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::AlreadyInitialized => CoreError::AlreadyInitialized,
            sui_id_store::StoreError::Conflict => {
                CoreError::Conflict("username already in use".into())
            }
            other => other.into(),
        })?;

    Ok(CreatedInitialAdmin {
        user_id: user.id,
        username: user.username,
    })
}

#[cfg(test)]
mod tests;
