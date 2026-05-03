//! TOTP MFA use cases.
//!
//! Two distinct flows:
//!
//! 1. **Enrolment.** A logged-in user wants to turn on TOTP.
//!    `start_enrollment` allocates a random secret and persists it in
//!    `user_totp` with `enabled = 0`. The HTTP layer shows a QR code and
//!    asks for a confirmation code. `confirm_enrollment` checks the code
//!    against the stored secret, generates 8 single-use recovery codes,
//!    and atomically flips the row to `enabled = 1`.
//!
//! 2. **Login.** After a successful password check, the bin layer asks
//!    `is_mfa_enabled`. If it is, the user gets a `login_pending_mfa`
//!    row and the MFA challenge page; otherwise a session is issued
//!    immediately. `verify_pending_with_code` redeems the pending row
//!    against a TOTP code (or a recovery code), creating a real session
//!    and deleting the pending row.

use crate::errors::{CoreError, CoreResult};
use crate::password::{hash_password, verify_password};
use crate::time::SharedClock;
use crate::tokens::random_token;
use crate::totp;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Duration;
use rand::{rngs::OsRng, RngCore};
use sui_id_shared::ids::{PendingMfaId, SessionId, UserId};
use sui_id_store::models::{LoginPendingMfaRow, SessionRow};
use sui_id_store::repos::{login_pending_mfa, sessions, user_totp};
use sui_id_store::Database;
use zeroize::Zeroize;

const TOTP_SECRET_LEN: usize = 20; // RFC 6238: 160 bits.
const RECOVERY_CODE_COUNT: usize = 8;
/// Length of the URL-safe base64 part of a recovery code (encodes 12 bytes).
const RECOVERY_CODE_BYTES: usize = 12;
const PENDING_MFA_TTL_SECS: i64 = 5 * 60;
const SESSION_LIFETIME_HOURS: i64 = 12;

/// True if the user must complete a second factor before a session is
/// issued. Either TOTP enrolment or at least one registered WebAuthn
/// credential counts; the user picks which factor to present at the
/// challenge page.
pub fn is_mfa_enabled(db: &Database, user_id: UserId) -> CoreResult<bool> {
    let totp_on = user_totp::get(db, user_id)?
        .map(|r| r.enabled)
        .unwrap_or(false);
    if totp_on {
        return Ok(true);
    }
    crate::webauthn::has_credentials(db, user_id)
}

// ----- enrolment ---------------------------------------------------------

pub struct EnrollmentTicket {
    /// Bytes the authenticator needs (raw, not Base32). The caller is
    /// responsible for zeroing once the QR is rendered.
    pub secret: Vec<u8>,
    pub otpauth_uri: String,
}

/// Allocate a fresh TOTP secret and persist it in the unconfirmed
/// (`enabled = 0`) state. Subsequent calls **replace** any prior
/// unconfirmed enrolment, so a user can scan again if they botched the
/// first attempt. If a confirmed enrolment already exists, returns
/// `Conflict` so the caller can guide the user to disable first.
pub fn start_enrollment(
    db: &Database,
    issuer: &str,
    user_id: UserId,
    username: &str,
) -> CoreResult<EnrollmentTicket> {
    if let Some(existing) = user_totp::get(db, user_id)? {
        if existing.enabled {
            return Err(CoreError::Conflict(
                "MFA is already enabled; disable it before re-enrolling".into(),
            ));
        }
    }
    let mut secret = vec![0u8; TOTP_SECRET_LEN];
    OsRng.fill_bytes(&mut secret);
    user_totp::upsert_pending(db, user_id, &secret)?;
    let uri = totp::otpauth_uri(issuer, username, &secret);
    Ok(EnrollmentTicket {
        secret,
        otpauth_uri: uri,
    })
}

/// Verify the user-typed confirmation code against the unconfirmed
/// enrolment, generate recovery codes, and flip the row to confirmed.
/// Returns the plaintext recovery codes for the caller to display
/// **once** to the user.
pub fn confirm_enrollment(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    supplied_code: u32,
) -> CoreResult<Vec<String>> {
    let row = user_totp::get(db, user_id)?
        .ok_or_else(|| CoreError::BadRequest("no pending TOTP enrolment".into()))?;
    if row.enabled {
        return Err(CoreError::Conflict(
            "MFA is already enabled; nothing to confirm".into(),
        ));
    }
    let mut secret = user_totp::decrypt_secret(db, &row)?;
    let now = clock.now().timestamp();
    let step = totp::verify(&secret, now, supplied_code, row.last_used_step);
    secret.zeroize();
    let step = step.ok_or_else(|| CoreError::BadRequest("verification code is incorrect".into()))?;

    let plain_codes: Vec<String> = (0..RECOVERY_CODE_COUNT)
        .map(|_| generate_recovery_code())
        .collect();
    let hashed: Vec<String> = plain_codes
        .iter()
        .map(|c| hash_password(c))
        .collect::<CoreResult<_>>()?;
    let blob = serde_json::to_vec(&hashed)
        .map_err(|_| CoreError::Internal)?;
    user_totp::confirm_with_recovery(db, user_id, &blob)?;
    user_totp::set_last_used_step(db, user_id, step)?;
    Ok(plain_codes)
}

/// Permanently disable TOTP for the user. The caller layer must ensure
/// the actor is permitted to do so — either it's the user themselves or
/// a sui-id administrator.
pub fn disable(db: &Database, user_id: UserId) -> CoreResult<()> {
    user_totp::delete(db, user_id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

/// Regenerate recovery codes (the user lost their copy). Requires that
/// MFA is already enabled. Returns the new plaintext codes.
pub fn regenerate_recovery_codes(db: &Database, user_id: UserId) -> CoreResult<Vec<String>> {
    let row = user_totp::get(db, user_id)?.ok_or(CoreError::NotFound)?;
    if !row.enabled {
        return Err(CoreError::BadRequest("MFA is not enabled".into()));
    }
    let plain: Vec<String> = (0..RECOVERY_CODE_COUNT)
        .map(|_| generate_recovery_code())
        .collect();
    let hashed: Vec<String> = plain
        .iter()
        .map(|c| hash_password(c))
        .collect::<CoreResult<_>>()?;
    let blob = serde_json::to_vec(&hashed).map_err(|_| CoreError::Internal)?;
    user_totp::set_recovery_codes(db, user_id, &blob)?;
    Ok(plain)
}

// ----- login --------------------------------------------------------------

/// Create a "password verified, MFA pending" record. The caller hands
/// the resulting `id` to the user as a short-lived cookie.
pub fn issue_pending_mfa(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
) -> CoreResult<LoginPendingMfaRow> {
    let now = clock.now();
    let row = LoginPendingMfaRow {
        id: PendingMfaId::new(),
        user_id,
        expires_at: now + Duration::seconds(PENDING_MFA_TTL_SECS),
        created_at: now,
    };
    login_pending_mfa::insert(db, &row)?;
    Ok(row)
}

/// Promote a pending-MFA record into a real session, given a correct
/// TOTP code (preferred) or a recovery code.
///
/// `code_input` is whatever the user typed. We try to interpret it as
/// digits first; if that fails, as a recovery code.
pub fn verify_pending(
    db: &Database,
    clock: &SharedClock,
    pending_id: PendingMfaId,
    code_input: &str,
) -> CoreResult<SessionRow> {
    let pending = login_pending_mfa::get(db, pending_id)?
        .ok_or(CoreError::Unauthenticated)?;
    if pending.expires_at < clock.now() {
        let _ = login_pending_mfa::delete(db, pending_id);
        return Err(CoreError::Unauthenticated);
    }
    let totp_row = user_totp::get(db, pending.user_id)?
        .ok_or(CoreError::Unauthenticated)?;
    if !totp_row.enabled {
        return Err(CoreError::Unauthenticated);
    }

    let trimmed = code_input.trim();
    let (accepted, method_used) = if let Ok(digits) = trimmed.parse::<u32>() {
        let mut secret = user_totp::decrypt_secret(db, &totp_row)?;
        let now = clock.now().timestamp();
        let result = totp::verify(&secret, now, digits, totp_row.last_used_step);
        secret.zeroize();
        match result {
            Some(step) => {
                user_totp::set_last_used_step(db, pending.user_id, step)?;
                (true, sui_id_shared::AuthMethod::Totp)
            }
            None => (false, sui_id_shared::AuthMethod::Totp),
        }
    } else {
        // Recovery-code path. Match against any stored hash; on hit,
        // remove that hash from the list so the code is single-use.
        let ok = consume_recovery_code(db, pending.user_id, &totp_row, trimmed)?;
        (ok, sui_id_shared::AuthMethod::RecoveryCode)
    };

    if !accepted {
        return Err(CoreError::InvalidCredentials);
    }

    // Promote into a session.
    let now = clock.now();
    let session = SessionRow {
        id: SessionId::new(),
        user_id: pending.user_id,
        expires_at: now + Duration::hours(SESSION_LIFETIME_HOURS),
        created_at: now,
        revoked_at: None,
        // Two factors were used: the password (which produced the
        // pending-MFA row) and whichever second factor the user just
        // verified. The session's `acr` will be "2" and `amr` will
        // include `pwd`, `otp`, and `mfa`.
        auth_methods: vec![sui_id_shared::AuthMethod::Pwd, method_used],
        // The user just completed a strong-factor challenge as part
        // of login. Record `now` so step-up-gated actions don't
        // immediately ask the user to re-prove themselves on a
        // session that's seconds old.
        last_step_up_at: Some(now),
            last_used_at: None,
    };
    sessions::insert(db, &session)?;
    crate::session::enforce_concurrent_session_cap(db, clock, session.user_id);
    let _ = login_pending_mfa::delete(db, pending_id);
    Ok(session)
}

/// Promote a pending-MFA record into a real session, treating a successful
/// WebAuthn authentication as the second factor.
///
/// The caller is responsible for having already invoked
/// `crate::webauthn::finish_authentication` against this pending row's
/// user — this function only consumes the pending row and issues the
/// session. Splitting it like this keeps webauthn-rs out of session.rs
/// and lets the HTTP layer audit "auth.mfa.success" once at the end of
/// either branch (TOTP or WebAuthn).
pub fn verify_pending_webauthn(
    db: &Database,
    clock: &SharedClock,
    pending_id: sui_id_shared::ids::PendingMfaId,
    expected_user_id: UserId,
) -> CoreResult<SessionRow> {
    let pending = login_pending_mfa::get(db, pending_id)?
        .ok_or(CoreError::Unauthenticated)?;
    if pending.expires_at < clock.now() {
        let _ = login_pending_mfa::delete(db, pending_id);
        return Err(CoreError::Unauthenticated);
    }
    if pending.user_id != expected_user_id {
        return Err(CoreError::Unauthenticated);
    }
    let now = clock.now();
    let session = SessionRow {
        id: SessionId::new(),
        user_id: pending.user_id,
        expires_at: now + Duration::hours(SESSION_LIFETIME_HOURS),
        created_at: now,
        revoked_at: None,
        // Password established the pending row; WebAuthn was the
        // second factor. The session's `acr` will be "3" (phishing-
        // resistant hardware-bound key) and `amr` will include
        // `pwd`, `hwk`, and `mfa`.
        auth_methods: vec![
            sui_id_shared::AuthMethod::Pwd,
            sui_id_shared::AuthMethod::Webauthn,
        ],
        // Phishing-resistant step-up just succeeded.
        last_step_up_at: Some(now),
            last_used_at: None,
    };
    sessions::insert(db, &session)?;
    crate::session::enforce_concurrent_session_cap(db, clock, session.user_id);
    let _ = login_pending_mfa::delete(db, pending_id);
    Ok(session)
}

pub(crate) fn consume_recovery_code(
    db: &Database,
    user_id: UserId,
    totp_row: &sui_id_store::models::UserTotpRow,
    candidate: &str,
) -> CoreResult<bool> {
    let blob = match user_totp::decrypt_recovery_codes(db, totp_row)? {
        Some(b) => b,
        None => return Ok(false),
    };
    let mut hashes: Vec<String> =
        serde_json::from_slice(&blob).map_err(|_| CoreError::Internal)?;
    let mut hit_idx: Option<usize> = None;
    for (i, h) in hashes.iter().enumerate() {
        if verify_password(candidate, h).is_ok() {
            hit_idx = Some(i);
            break;
        }
    }
    if let Some(i) = hit_idx {
        hashes.remove(i);
        let new_blob = serde_json::to_vec(&hashes).map_err(|_| CoreError::Internal)?;
        user_totp::set_recovery_codes(db, user_id, &new_blob)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// ----- helpers ------------------------------------------------------------

/// Generate a single recovery code. Format: `xxxxx-xxxxx-xxxxx` where
/// each chunk is 5 base64url chars. Easy to type, hard to predict.
fn generate_recovery_code() -> String {
    let _ = random_token; // signal we considered the existing helper.
    let mut bytes = [0u8; RECOVERY_CODE_BYTES];
    OsRng.fill_bytes(&mut bytes);
    let mut buf = [0u8; 32];
    let n = Base64UrlUnpadded::encode(&bytes, &mut buf)
        .map(str::len)
        .unwrap_or(0);
    let s = std::str::from_utf8(&buf[..n]).unwrap_or("");
    // 12 raw bytes → 16 base64url chars. Group as 5-5-6 separated by '-'.
    let s: String = s.chars().take(15).collect();
    let mut out = String::with_capacity(17);
    for (i, c) in s.chars().enumerate() {
        if i == 5 || i == 10 {
            out.push('-');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_code_format() {
        let c = generate_recovery_code();
        assert_eq!(c.len(), 17);
        assert_eq!(c.as_bytes()[5], b'-');
        assert_eq!(c.as_bytes()[11], b'-');
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::time::system_clock;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::users;
    use sui_id_store::Database;

    fn fresh_db_with_user() -> (Database, UserId) {
        let key = MasterKey::generate();
        let db = Database::open_in_memory(key).expect("db");
        let uid = UserId::new();
        users::create(
            &db,
            &UserRow {
                id: uid,
                username: "alice".into(),
                display_name: None,
                is_admin: true,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                failed_login_count: 0,
                locked_until: None,
                email: None,
                preferred_lang: None,
            },
        )
        .expect("insert user");
        (db, uid)
    }

    #[test]
    fn enroll_then_confirm_completes_and_returns_8_recovery_codes() {
        let (db, uid) = fresh_db_with_user();
        let clock = system_clock();
        let ticket = start_enrollment(&db, "sui-id", uid, "alice").expect("start");
        assert_eq!(ticket.secret.len(), 20);
        let now = clock.now().timestamp();
        let step = now / 30;
        let code = crate::totp::code_for_step(&ticket.secret, step);
        let codes = confirm_enrollment(&db, &clock, uid, code).expect("confirm");
        assert_eq!(codes.len(), 8);
        // The user should now report MFA enabled.
        assert!(is_mfa_enabled(&db, uid).unwrap());
    }

    #[test]
    fn confirm_with_wrong_code_returns_bad_request() {
        let (db, uid) = fresh_db_with_user();
        let clock = system_clock();
        let _ = start_enrollment(&db, "sui-id", uid, "alice").expect("start");
        let r = confirm_enrollment(&db, &clock, uid, 000000);
        assert!(matches!(r, Err(crate::CoreError::BadRequest(_))));
    }

    #[test]
    fn disable_then_re_enroll_works() {
        let (db, uid) = fresh_db_with_user();
        let clock = system_clock();
        let ticket = start_enrollment(&db, "sui-id", uid, "alice").expect("start");
        let step = clock.now().timestamp() / 30;
        let code = crate::totp::code_for_step(&ticket.secret, step);
        let _ = confirm_enrollment(&db, &clock, uid, code).expect("confirm");
        disable(&db, uid).expect("disable");
        assert!(!is_mfa_enabled(&db, uid).unwrap());
        // Re-enrol from scratch should succeed.
        let _ = start_enrollment(&db, "sui-id", uid, "alice").expect("re-start");
    }
}
