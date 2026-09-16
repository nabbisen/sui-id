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
use getrandom;
use sui_id_shared::ids::{PendingMfaId, SessionId, UserId};
use sui_id_store::Database;
use sui_id_store::commands::SecondFactorProof;
use sui_id_store::models::{LoginPendingMfaRow, SessionRow};
use sui_id_store::repos::{login_pending_mfa, user_totp};
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
pub async fn is_mfa_enabled(db: &Database, user_id: UserId) -> CoreResult<bool> {
    let totp_on = user_totp::get(db, user_id)
        .await?
        .map(|r| r.enabled)
        .unwrap_or(false);
    if totp_on {
        return Ok(true);
    }
    crate::webauthn::has_credentials(db, user_id).await
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
pub async fn start_enrollment(
    db: &Database,
    issuer: &str,
    user_id: UserId,
    username: &str,
) -> CoreResult<EnrollmentTicket> {
    if let Some(existing) = user_totp::get(db, user_id).await?
        && existing.enabled
    {
        return Err(CoreError::Conflict(
            "MFA is already enabled; disable it before re-enrolling".into(),
        ));
    }
    let mut secret = vec![0u8; TOTP_SECRET_LEN];
    getrandom::fill(&mut secret).map_err(|_| CoreError::Internal)?;
    user_totp::upsert_pending(db, user_id, &secret).await?;
    let uri = totp::otpauth_uri(issuer, username, &secret).await;
    Ok(EnrollmentTicket {
        secret,
        otpauth_uri: uri,
    })
}

/// Verify the user-typed confirmation code against the unconfirmed
/// enrolment, generate recovery codes, and flip the row to confirmed.
/// Returns the plaintext recovery codes for the caller to display
/// **once** to the user.
pub async fn confirm_enrollment(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    supplied_code: u32,
) -> CoreResult<Vec<String>> {
    let row = user_totp::get(db, user_id)
        .await?
        .ok_or_else(|| CoreError::BadRequest("no pending TOTP enrolment".into()))?;
    if row.enabled {
        return Err(CoreError::Conflict(
            "MFA is already enabled; nothing to confirm".into(),
        ));
    }
    let mut secret = user_totp::decrypt_secret(db, &row).await?;
    let now = clock.now().timestamp();
    let step = totp::verify(&secret, now, supplied_code, row.last_used_step).await;
    secret.zeroize();
    let step =
        step.ok_or_else(|| CoreError::BadRequest("verification code is incorrect".into()))?;

    let mut plain_codes: Vec<String> = Vec::with_capacity(RECOVERY_CODE_COUNT);
    for _ in 0..RECOVERY_CODE_COUNT {
        plain_codes.push(generate_recovery_code()?);
    }
    let mut hashed: Vec<String> = Vec::with_capacity(plain_codes.len());
    for c in &plain_codes {
        hashed.push(hash_password(c)?);
    }
    let blob = serde_json::to_vec(&hashed).map_err(|_| CoreError::Internal)?;
    // U12 (RFC 102 B7): enabling TOTP, storing the codes, advancing the
    // replay cursor and `auth.mfa.factor_added` commit together.
    let sealed = user_totp::seal_recovery_codes(db, &blob)?;
    sui_id_store::commands::confirm_totp_enrollment(db, user_id, sealed, step).await?;
    Ok(plain_codes)
}

/// Permanently disable TOTP for the user. The caller layer must ensure
/// the actor is permitted to do so — either it's the user themselves or
/// a sui-id administrator.
pub async fn disable(db: &Database, user_id: UserId) -> CoreResult<()> {
    user_totp::delete(db, user_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::NotFound,
        other => CoreError::from(other),
    })?;
    Ok(())
}

/// Regenerate recovery codes (the user lost their copy). Requires that
/// MFA is already enabled. Returns the new plaintext codes.
pub async fn regenerate_recovery_codes(db: &Database, user_id: UserId) -> CoreResult<Vec<String>> {
    let row = user_totp::get(db, user_id)
        .await?
        .ok_or(CoreError::NotFound)?;
    if !row.enabled {
        return Err(CoreError::BadRequest("MFA is not enabled".into()));
    }
    let mut plain: Vec<String> = Vec::with_capacity(RECOVERY_CODE_COUNT);
    for _ in 0..RECOVERY_CODE_COUNT {
        plain.push(generate_recovery_code()?);
    }
    let mut hashed: Vec<String> = Vec::with_capacity(plain.len());
    for c in &plain {
        hashed.push(hash_password(c)?);
    }
    let blob = serde_json::to_vec(&hashed).map_err(|_| CoreError::Internal)?;
    // U14 (RFC 102 B7): the replacement codes and `auth.mfa.factor_added`
    // commit together.
    let sealed = user_totp::seal_recovery_codes(db, &blob)?;
    sui_id_store::commands::regenerate_recovery_codes(db, user_id, sealed).await?;
    Ok(plain)
}

// ----- login --------------------------------------------------------------

/// Create a "password verified, MFA pending" record. The caller hands
/// the resulting `id` to the user as a short-lived cookie.
pub async fn issue_pending_mfa(
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
    login_pending_mfa::insert(db, &row).await?;
    Ok(row)
}

/// Promote a pending-MFA record into a real session, given a correct
/// TOTP code (preferred) or a recovery code.
///
/// `code_input` is whatever the user typed. We try to interpret it as
/// digits first; if that fails, as a recovery code.
///
/// RFC 102 L02/L07: the code is checked outside any transaction. A wrong
/// code runs L07 (counted on the user) and returns `InvalidCredentials`.
/// A right code runs L02, which commits the session, the factor's guard
/// and `auth.mfa.success` together; if L02 fails, L07 is not run (A9). A
/// guard that loses (the pending row already consumed, the TOTP step
/// already used, the recovery codes already changed, the user no longer
/// active) is `Unauthenticated`.
pub async fn verify_pending(
    db: &Database,
    clock: &SharedClock,
    pending_id: PendingMfaId,
    code_input: &str,
    max_lockout_secs: i64,
) -> CoreResult<SessionRow> {
    let pending = login_pending_mfa::get(db, pending_id)
        .await?
        .ok_or(CoreError::Unauthenticated)?;
    if pending.expires_at < clock.now() {
        let _ = login_pending_mfa::delete(db, pending_id).await;
        return Err(CoreError::Unauthenticated);
    }
    let totp_row = user_totp::get(db, pending.user_id)
        .await?
        .ok_or(CoreError::Unauthenticated)?;
    if !totp_row.enabled {
        return Err(CoreError::Unauthenticated);
    }

    let trimmed = code_input.trim();
    let (proof, method_used) = if let Ok(digits) = trimmed.parse::<u32>() {
        let mut secret = user_totp::decrypt_secret(db, &totp_row).await?;
        let now = clock.now().timestamp();
        let result = totp::verify(&secret, now, digits, totp_row.last_used_step).await;
        secret.zeroize();
        (
            result.map(|step| SecondFactorProof::Totp { step }),
            sui_id_shared::AuthMethod::Totp,
        )
    } else {
        // Recovery-code path. Match against any stored hash; on a hit the
        // blob without that hash replaces the one matched, in L02.
        (
            match_recovery_code(db, &totp_row, trimmed).await?,
            sui_id_shared::AuthMethod::RecoveryCode,
        )
    };

    let Some(proof) = proof else {
        record_second_factor_failure(db, pending.user_id, max_lockout_secs).await?;
        return Err(CoreError::InvalidCredentials);
    };
    complete(db, clock, pending_id, pending.user_id, method_used, proof).await
}

/// Promote a pending-MFA record into a real session, treating a successful
/// WebAuthn authentication as the second factor.
///
/// The caller is responsible for having already invoked
/// `crate::webauthn::finish_authentication` against this pending row's
/// user — this function only runs L02. A failed assertion is the caller's
/// to count with [`record_second_factor_failure`].
pub async fn verify_pending_webauthn(
    db: &Database,
    clock: &SharedClock,
    pending_id: sui_id_shared::ids::PendingMfaId,
    expected_user_id: UserId,
) -> CoreResult<SessionRow> {
    complete(
        db,
        clock,
        pending_id,
        expected_user_id,
        sui_id_shared::AuthMethod::Webauthn,
        SecondFactorProof::Webauthn,
    )
    .await
}

/// RFC 102 L07: count one wrong second factor on the user. At the
/// threshold every pending-MFA row for the user is deleted and the account
/// is locked with the password path's backoff.
pub async fn record_second_factor_failure(
    db: &Database,
    user_id: UserId,
    max_lockout_secs: i64,
) -> CoreResult<sui_id_store::commands::SecondFactorFailureOutcome> {
    let audited = sui_id_store::commands::record_second_factor_failure(db, user_id, move |count| {
        crate::session::lockout_backoff(count, max_lockout_secs)
    })
    .await?;
    Ok(audited.into_inner())
}

async fn complete(
    db: &Database,
    clock: &SharedClock,
    pending_id: PendingMfaId,
    user_id: UserId,
    method_used: sui_id_shared::AuthMethod,
    proof: SecondFactorProof,
) -> CoreResult<SessionRow> {
    let now = clock.now();
    let session = SessionRow {
        id: SessionId::new(),
        user_id,
        expires_at: now + Duration::hours(SESSION_LIFETIME_HOURS),
        created_at: now,
        revoked_at: None,
        // Two factors were used: the password (which produced the
        // pending-MFA row) and the second factor just verified. The
        // session's `acr` and `amr` follow from these.
        auth_methods: vec![sui_id_shared::AuthMethod::Pwd, method_used],
        // L02 decides freshness by method: TOTP and WebAuthn make the new
        // session fresh; a recovery code does not (RFC 102 N5).
        last_step_up_at: None,
        last_used_at: None,
    };
    let audited = sui_id_store::commands::complete_second_factor(db, pending_id, session, proof)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
            other => other.into(),
        })?;
    Ok(audited.into_inner())
}

/// Returns the number of unused recovery codes for `user_id` (RFC 056).
///
/// The count is the post-decryption length of the recovery-codes
/// JSON array; this is the canonical representation since
/// `consume_recovery_code` removes hashes from the array when used,
/// and `regenerate_recovery_codes` replaces the whole array. A return
/// of 0 means either (a) the user has no TOTP enrolled, (b) the user
/// has TOTP but recovery codes have never been issued, or (c) every
/// issued code has been consumed.
///
/// Errors only on database / decryption failure. The caller is
/// expected to `unwrap_or(0)` for display purposes, since failing
/// the count shouldn't fail the surrounding render.
pub async fn count_recovery_codes_remaining(db: &Database, user_id: UserId) -> CoreResult<usize> {
    let Some(row) = user_totp::get(db, user_id).await? else {
        return Ok(0);
    };
    let Some(blob) = user_totp::decrypt_recovery_codes(db, &row).await? else {
        return Ok(0);
    };
    let hashes: Vec<String> = serde_json::from_slice(&blob).map_err(|_| CoreError::Internal)?;
    Ok(hashes.len())
}

/// Find the recovery code `candidate` among the user's stored hashes. On a
/// hit, returns the L02 proof: the sealed blob matched against, and the
/// sealed blob without that hash. Nothing is written here.
async fn match_recovery_code(
    db: &Database,
    totp_row: &sui_id_store::models::UserTotpRow,
    candidate: &str,
) -> CoreResult<Option<SecondFactorProof>> {
    let (Some(expected_sealed), Some(blob)) = (
        totp_row.recovery_codes_enc.clone(),
        user_totp::decrypt_recovery_codes(db, totp_row).await?,
    ) else {
        return Ok(None);
    };
    let mut hashes: Vec<String> = serde_json::from_slice(&blob).map_err(|_| CoreError::Internal)?;
    let Some(i) = hashes
        .iter()
        .position(|h| verify_password(candidate, h).is_ok())
    else {
        return Ok(None);
    };
    hashes.remove(i);
    let new_blob = serde_json::to_vec(&hashes).map_err(|_| CoreError::Internal)?;
    let new_sealed = user_totp::seal_recovery_codes(db, &new_blob)?;
    Ok(Some(SecondFactorProof::RecoveryCode {
        expected_sealed,
        new_sealed,
    }))
}

// ----- helpers ------------------------------------------------------------

/// Generate a single recovery code. Format: `xxxxx-xxxxx-xxxxx` where
/// each chunk is 5 base64url chars. Easy to type, hard to predict.
fn generate_recovery_code() -> CoreResult<String> {
    let _ = random_token; // signal we considered the existing helper.
    let mut bytes = [0u8; RECOVERY_CODE_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| CoreError::Internal)?;
    let s = Base64UrlUnpadded::encode_string(&bytes);
    // 12 raw bytes → 16 base64url chars. Group as 5-5-6 separated by '-'.
    let s: String = s.chars().take(15).collect();
    let mut out = String::with_capacity(17);
    for (i, c) in s.chars().enumerate() {
        if i == 5 || i == 10 {
            out.push('-');
        }
        out.push(c);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recovery_code_format() {
        let c = generate_recovery_code().expect("recovery code");
        assert_eq!(c.len(), 17);
        assert_eq!(c.as_bytes()[5], b'-');
        assert_eq!(c.as_bytes()[11], b'-');
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::time::system_clock;
    use sui_id_store::Database;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::UserRow;
    use sui_id_store::repos::users;

    async fn fresh_db_with_user() -> (Database, UserId) {
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
                role: if true {
                    sui_id_store::models::Role::Admin
                } else {
                    sui_id_store::models::Role::User
                },
                last_login_at: None,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                failed_login_count: 0,
                locked_until: None,
                source: sui_id_store::models::UserSource::Local,
                external_stable_id: None,
                email: None,
                preferred_lang: None,
                email_normalized: None,
                email_verified_at: None,
            },
        )
        .await
        .expect("insert user");
        (db, uid)
    }

    #[tokio::test]
    async fn enroll_then_confirm_completes_and_returns_8_recovery_codes() {
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let ticket = start_enrollment(&db, "sui-id", uid, "alice")
            .await
            .expect("start");
        assert_eq!(ticket.secret.len(), 20);
        let now = clock.now().timestamp();
        let step = now / 30;
        let code = crate::totp::code_for_step(&ticket.secret, step).await;
        let codes = confirm_enrollment(&db, &clock, uid, code)
            .await
            .expect("confirm");
        assert_eq!(codes.len(), 8);
        // The user should now report MFA enabled.
        assert!(is_mfa_enabled(&db, uid).await.unwrap());
    }

    #[tokio::test]
    async fn confirm_with_wrong_code_returns_bad_request() {
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let _ = start_enrollment(&db, "sui-id", uid, "alice")
            .await
            .expect("start");
        let r = confirm_enrollment(&db, &clock, uid, 000000).await;
        assert!(matches!(r, Err(crate::CoreError::BadRequest(_))));
    }

    #[tokio::test]
    async fn disable_then_re_enroll_works() {
        let (db, uid) = fresh_db_with_user().await;
        let clock = system_clock();
        let ticket = start_enrollment(&db, "sui-id", uid, "alice")
            .await
            .expect("start");
        let step = clock.now().timestamp() / 30;
        let code = crate::totp::code_for_step(&ticket.secret, step).await;
        let _ = confirm_enrollment(&db, &clock, uid, code)
            .await
            .expect("confirm");
        disable(&db, uid).await.expect("disable");
        assert!(!is_mfa_enabled(&db, uid).await.unwrap());
        // Re-enrol from scratch should succeed.
        let _ = start_enrollment(&db, "sui-id", uid, "alice")
            .await
            .expect("re-start");
    }
}
