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
///
/// This is the **web wizard** path: the setup token printed at boot must
/// match (constant-time comparison) because the endpoint is
/// network-reachable before the instance has an owner.
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

    create_initial_admin_inner(
        db, clock, username, password, display_name, email,
        /* must_change */ false,
        /* headless */ false,
    ).await
}

/// Create the first administrator from the CLI (RFC 077) — **headless** path.
///
/// No setup token: this path requires filesystem access to the database
/// and master key, and an actor with that access already controls the
/// instance. (Same trust model as the `admin unlock-user` subcommand.)
///
/// `must_change` records that the password should be rotated after first
/// login — set `true` when the password was machine-generated and printed
/// to the console. Login-time enforcement is a future RFC; the flag makes
/// the intent durable today.
pub async fn create_initial_admin_headless(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    email: Option<&str>,
    must_change: bool,
) -> CoreResult<CreatedInitialAdmin> {
    if state::is_initialized(db)? {
        return Err(CoreError::AlreadyInitialized);
    }

    create_initial_admin_inner(
        db, clock, username, password, display_name, email,
        must_change,
        /* headless */ true,
    ).await
}

/// Generate a random initial-admin password: 24 chars from `[A-Za-z0-9]`
/// (≈143 bits of entropy) using the OS RNG — the same primitive that
/// generates signing keys. Rejection sampling avoids modulo bias.
///
/// Returned in `Zeroizing` so the plaintext is wiped from memory when the
/// caller drops it after printing.
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
/// bootstrap, initialized flag, audit entry. Callers have already done
/// their path-specific authorization (token check / filesystem trust)
/// and the already-initialized check.
async fn create_initial_admin_inner(
    db: &Database,
    clock: &SharedClock,
    username: &str,
    password: &str,
    display_name: Option<&str>,
    email: Option<&str>,
    must_change: bool,
    headless: bool,
) -> CoreResult<CreatedInitialAdmin> {
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
        must_change,
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
            note: if headless { Some("headless".into()) } else { None },
        },
    ).await?;

    Ok(CreatedInitialAdmin {
        user_id: user.id,
        username: user.username,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_id_store::crypto::MasterKey;

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    // ---------- generate_admin_password (RFC 077) ----------

    #[test]
    fn generated_password_is_24_alphanumeric_chars() {
        let pw = generate_admin_password();
        assert_eq!(pw.len(), 24);
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn generated_passwords_differ_across_calls() {
        let a = generate_admin_password();
        let b = generate_admin_password();
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn generated_password_satisfies_standard_policy() {
        let pw = generate_admin_password();
        check_password_policy(&pw, SecurityLevel::Standard.password_min_len())
            .expect("generated password must pass Standard policy");
    }

    // ---------- create_initial_admin_headless (RFC 077) ----------

    #[tokio::test]
    async fn headless_setup_creates_admin_and_marks_initialized() {
        let db = fresh_db();
        let clock = crate::time::system_clock();

        let created = create_initial_admin_headless(
            &db, &clock,
            "first-admin",
            "a-long-enough-password",
            Some("First Admin"),
            Some("admin@example.com"),
            /* must_change */ true,
        ).await.expect("headless setup");

        assert_eq!(created.username, "first-admin");
        assert!(state::is_initialized(&db).expect("state read"));

        // must_change persisted as passed.
        let cred = credentials::get(&db, created.user_id).await.expect("cred");
        assert!(cred.must_change, "generated-password intent must be recorded");
    }

    #[tokio::test]
    async fn headless_setup_fails_when_already_initialized() {
        let db = fresh_db();
        let clock = crate::time::system_clock();

        create_initial_admin_headless(
            &db, &clock, "first-admin", "a-long-enough-password",
            None, None, false,
        ).await.expect("first setup");

        let second = create_initial_admin_headless(
            &db, &clock, "second-admin", "another-long-password",
            None, None, false,
        ).await;
        assert!(matches!(second, Err(CoreError::AlreadyInitialized)));
    }

    #[tokio::test]
    async fn headless_setup_enforces_standard_password_policy() {
        let db = fresh_db();
        let clock = crate::time::system_clock();

        // 8 chars passes Development but must fail here: setup is always Standard.
        let r = create_initial_admin_headless(
            &db, &clock, "first-admin", "changeme", None, None, false,
        ).await;
        assert!(matches!(r, Err(CoreError::BadRequest(_))));
        assert!(!state::is_initialized(&db).expect("state read"));
    }

    #[tokio::test]
    async fn web_wizard_path_still_requires_matching_token() {
        let db = fresh_db();
        let clock = crate::time::system_clock();

        let r = create_initial_admin(
            &db, &clock,
            "expected-token", "wrong-token",
            "first-admin", "a-long-enough-password",
            None, None,
        ).await;
        assert!(matches!(r, Err(CoreError::Forbidden)));

        let ok = create_initial_admin(
            &db, &clock,
            "expected-token", "expected-token",
            "first-admin", "a-long-enough-password",
            None, None,
        ).await;
        assert!(ok.is_ok());
        // Wizard-created credential is NOT flagged must_change.
        let cred = credentials::get(&db, ok.unwrap().user_id).await.expect("cred");
        assert!(!cred.must_change);
    }
}
