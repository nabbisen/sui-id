//! Step-up authentication: requiring a fresh proof of a strong factor
//! before a sensitive action.
//!
//! ## What "step-up" means here
//!
//! A regular session is fine for routine reads (the dashboard, your
//! own profile, the audit log) but should not, on its own, be enough
//! to authorise a destructive or security-relevant action — change a
//! password, revoke every other session at once, delete a client,
//! force-reset another user's MFA, rotate a signing key. The
//! prevailing risk is **session theft**: someone who has stolen a
//! cookie can navigate the UI, but should not be able to immediately
//! ratchet that into permanent damage.
//!
//! Step-up auth closes that gap by requiring a fresh MFA challenge
//! shortly before the sensitive action, regardless of how the
//! session was originally established. The freshness window is
//! short — five minutes by default — so a stolen cookie is useful
//! for almost no destructive purpose.
//!
//! ## What we *don't* do here
//!
//! - **Force step-up on accounts that don't have MFA enrolled.** If
//!   the user's only factor is a password, a "step-up" challenge
//!   would be a password re-prompt — same factor, same theft model,
//!   no security gain, large UX cost. We document this clearly and
//!   leave it to operators (and to this project's own roadmap of
//!   making MFA mandatory) to close the gap by enrolling MFA.
//! - **Per-action policy customisation.** The freshness window is
//!   one number. If you need different windows for different
//!   actions, the better answer is to require MFA for the user
//!   instead of building a fine-grained policy DSL.
//! - **Storing per-action proof tokens.** Some systems mint a
//!   short-lived "this user proved MFA at 14:07; the action
//!   submitted at 14:09 carries the proof token". We instead just
//!   compare `now - session.last_step_up_at` because the session
//!   is the action's authorisation context already, and an extra
//!   token is one more piece of state to lose.

use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use crate::webauthn;
use chrono::{DateTime, Duration};
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::Database;
use sui_id_store::commands::StepUpProof;
use sui_id_store::repos::user_totp;

/// Default freshness window: a session whose last step-up
/// happened within this many seconds is treated as fresh. Long
/// enough to avoid retyping a TOTP code three times in one
/// admin-cleanup pass, short enough that a session stolen
/// hours after a previous step-up gets re-challenged.
/// 5 minutes. Defined with the gated commands in `sui-id-store`, which
/// re-check it inside their transactions (RFC 102 B4).
pub use sui_id_store::commands::STEP_UP_FRESHNESS_SECS;

/// Outcome of `policy_for_session`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepUpDecision {
    /// The session is allowed through without a challenge — either
    /// because the user has no MFA enrolled (so a step-up would
    /// just be a password re-prompt and not a meaningful gate), or
    /// because the session's `last_step_up_at` is within the
    /// freshness window.
    Allow,
    /// The session must complete a step-up challenge before the
    /// caller proceeds. The handler typically responds with a
    /// redirect to `/me/security/step-up?return_to=...`.
    Challenge,
}

/// Decide whether a session can perform a sensitive action right
/// now, or must first complete a step-up challenge.
///
/// The rule:
///
/// - If the user has *no* MFA factor enrolled (no TOTP, no
///   passkeys), allow. Step-up for a password-only account would
///   be a password re-prompt, which buys nothing against an
///   attacker who already has the cookie *and* the password.
/// - Otherwise, require `last_step_up_at >= now - freshness_secs`.
///   Sessions with `last_step_up_at = None` (password-only
///   established, or pre-migration row) always need to challenge.
///
/// # Factor eligibility (RFC 089)
///
/// Only TOTP codes and WebAuthn assertions satisfy step-up.
/// **Recovery codes are explicitly excluded** — they are for account
/// recovery, not for routine step-up re-authentication — and
/// [`verify_totp_code`] refuses them (RFC 102 B3).
///
/// Two paths set `last_step_up_at`: the step-up challenge handlers
/// (`/me/security/step-up` POST and the WebAuthn finish endpoint), and the
/// sign-in second factor, where RFC 102 L02 decides by method: a TOTP or
/// WebAuthn sign-in is fresh, a recovery-code sign-in is not.
pub async fn policy_for_session(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    last_step_up_at: Option<DateTime<chrono::Utc>>,
    freshness_secs: i64,
) -> CoreResult<StepUpDecision> {
    if !user_has_mfa(db, user_id).await? {
        return Ok(StepUpDecision::Allow);
    }
    let last = match last_step_up_at {
        Some(t) => t,
        None => return Ok(StepUpDecision::Challenge),
    };
    let cutoff = clock.now() - Duration::seconds(freshness_secs);
    if last >= cutoff {
        Ok(StepUpDecision::Allow)
    } else {
        Ok(StepUpDecision::Challenge)
    }
}

/// Whether the user has any MFA factor enrolled (TOTP enabled
/// *or* at least one WebAuthn credential).
pub async fn user_has_mfa(db: &Database, user_id: UserId) -> CoreResult<bool> {
    let totp = user_totp::get(db, user_id)
        .await?
        .map(|r| r.enabled)
        .unwrap_or(false);
    if totp {
        return Ok(true);
    }
    let has_passkey = webauthn::has_credentials(db, user_id).await?;
    Ok(has_passkey)
}

/// Verify a TOTP code entered into a step-up form by an already-signed-in
/// user.
///
/// Unlike [`crate::mfa::verify_pending`], this does **not** create
/// a new session — the user already has one. On success it updates
/// `last_step_up_at` on the supplied session and returns
/// `Ok(())`. On failure it returns `Err(CoreError::InvalidCredentials)`
/// without revealing whether the code was wrong, expired, or never
/// configured: a step-up form should look the same to a user with
/// a typo as to an attacker probing whether MFA is enabled.
///
/// Recovery codes are **not** accepted (RFC 089; RFC 102 B3). Anything
/// that does not parse as a TOTP code is `InvalidCredentials`, and no
/// recovery code is looked at or consumed.
///
/// Error contract, which the handler relies on for RFC 102 A9:
/// `InvalidCredentials` means a wrong factor and is counted (L06); any
/// other error is a storage or internal failure and is not.
pub async fn verify_totp_code(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    session_id: SessionId,
    code_input: &str,
    gate: &str,
) -> CoreResult<()> {
    use crate::totp;
    use zeroize::Zeroize;

    let totp_row = user_totp::get(db, user_id)
        .await?
        .ok_or(CoreError::InvalidCredentials)?;
    if !totp_row.enabled {
        return Err(CoreError::InvalidCredentials);
    }

    let trimmed = code_input.trim();
    let step = if let Ok(digits) = trimmed.parse::<u32>() {
        let mut secret = user_totp::decrypt_secret(db, &totp_row).await?;
        let now = clock.now().timestamp();
        let result = totp::verify(&secret, now, digits, totp_row.last_used_step).await;
        secret.zeroize();
        result
    } else {
        // Not a TOTP code. Recovery codes are refused for step-up.
        None
    };
    let Some(step) = step else {
        return Err(CoreError::InvalidCredentials);
    };

    complete(
        db,
        clock,
        user_id,
        session_id,
        StepUpProof::Totp { step },
        gate,
    )
    .await
}

/// RFC 102 L05: commit a verified step-up. A guard that loses (the session
/// revoked or expired meanwhile, the TOTP step already used, the ceremony
/// already consumed) is `Unauthenticated`; any failure here is not a wrong
/// factor, so the caller must not run L06 for it (A9).
async fn complete(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    session_id: SessionId,
    proof: StepUpProof,
    gate: &str,
) -> CoreResult<()> {
    sui_id_store::commands::complete_step_up(
        db,
        user_id,
        session_id,
        proof,
        gate.to_owned(),
        clock.now(),
    )
    .await
    .map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
        other => other.into(),
    })?;
    Ok(())
}

// ---------- WebAuthn-driven step-up ----------
//
// The TOTP path above covers users with an
// authenticator app. Users whose only second factor is a passkey
// also need a way to satisfy a step-up gate. The webauthn-rs
// assertion flow is already split into pure start / finish halves
// (see `webauthn::start_authentication` / `finish_authentication`),
// so the step-up versions are thin wrappers: same low-level
// ceremony, different bookkeeping. The ceremony-state row is tagged
// with `WebauthnPendingKind::StepUp` so a pending login-MFA row
// can never satisfy a step-up gate (and vice versa) even if a
// pending_id ever leaked across contexts.
//
// The handler-side flow is:
//
//   1. POST /me/security/step-up/webauthn/start
//      → step_up::start_webauthn(...).await → returns (challenge_json, pending_id)
//      → handler streams challenge_json to JS, sets pending_id in
//        a short-lived cookie
//   2. JS calls navigator.credentials.get(...) and POSTs the
//      assertion back to
//      POST /me/security/step-up/webauthn/finish
//      → step_up::finish_webauthn(...).await → on success, last_step_up_at
//        is set
//      → handler clears the pending_id cookie and 303s back to
//        return_to

/// Output of [`start_webauthn`].
pub struct WebauthnStepUpStart {
    /// JSON the browser hands to navigator.credentials.get().
    pub challenge_json: String,
    /// Opaque id of the pending row that holds the auth state.
    /// The handler stuffs this into a short-lived cookie.
    pub pending_id: sui_id_shared::ids::WebauthnPendingId,
}

/// Begin a WebAuthn step-up ceremony for an already-signed-in user.
///
/// Wraps [`crate::webauthn::start_authentication`] with the
/// step-up-specific bookkeeping: the resulting pending row is tagged
/// `kind = StepUp`. The user must already have at least one
/// passkey enrolled; an empty credential list returns
/// `BadRequest` with the same message the login flow uses.
pub async fn start_webauthn(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    user_id: UserId,
) -> CoreResult<WebauthnStepUpStart> {
    use crate::webauthn;
    use sui_id_store::models::WebauthnPendingKind;

    // The ceremony row is created as `StepUp` directly (RFC 102 B-F5).
    let started =
        webauthn::start_authentication(db, clock, issuer_url, user_id, WebauthnPendingKind::StepUp)
            .await?;

    Ok(WebauthnStepUpStart {
        challenge_json: started.challenge_json,
        pending_id: started.pending_id,
    })
}

/// Finish a WebAuthn step-up ceremony.
///
/// [`crate::webauthn::finish_authentication`] verifies the assertion against
/// a ceremony that must be of kind `StepUp` (a sign-in ceremony can never
/// satisfy a step-up gate, RFC 102 B-F5) and this user's. On success L05
/// consumes the ceremony and commits the freshness with its event.
///
/// Failures collapse to `InvalidCredentials` for the same
/// information-hiding reason the TOTP path does — a step-up form
/// must look the same to a typo as to an attacker probing. A storage or
/// internal failure, or a failed L05, is not a wrong factor and is returned
/// as itself (A9).
#[allow(clippy::too_many_arguments)]
pub async fn finish_webauthn(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    user_id: UserId,
    session_id: SessionId,
    pending_id: sui_id_shared::ids::WebauthnPendingId,
    credential_json: &str,
    gate: &str,
) -> CoreResult<()> {
    use crate::webauthn;
    use sui_id_store::models::WebauthnPendingKind;
    use webauthn_rs::prelude::PublicKeyCredential;

    let credential: PublicKeyCredential =
        serde_json::from_str(credential_json).map_err(|_| CoreError::InvalidCredentials)?;

    match webauthn::finish_authentication(
        db,
        clock,
        issuer_url,
        pending_id,
        user_id,
        WebauthnPendingKind::StepUp,
        &credential,
    )
    .await
    {
        Ok(()) => {
            complete(
                db,
                clock,
                user_id,
                session_id,
                StepUpProof::Webauthn { pending_id },
                gate,
            )
            .await
        }
        // A storage or internal failure is not a wrong factor and must not
        // be counted as one (RFC 102 A9); everything else — a failed
        // assertion, an expired, foreign or wrong-kind ceremony — is.
        Err(e @ (CoreError::Store(_) | CoreError::Internal)) => Err(e),
        Err(_) => Err(CoreError::InvalidCredentials),
    }
}

// ---------- step-up failure accounting (RFC 102 L06) ----------

pub use sui_id_store::commands::{STEP_UP_FAILURE_REVOCATION_THRESHOLD, StepUpFailureOutcome};

/// Count one failed step-up re-authentication on `session_id`, revoking
/// the session at [`STEP_UP_FAILURE_REVOCATION_THRESHOLD`]. Call it only
/// for a wrong factor or password (`InvalidCredentials`), never for a
/// storage failure on the success path (RFC 102 A9).
pub async fn record_step_up_failure(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    session_id: SessionId,
) -> CoreResult<StepUpFailureOutcome> {
    let audited =
        sui_id_store::commands::record_step_up_failure(db, user_id, session_id, clock.now())
            .await?;
    Ok(audited.into_inner())
}

// ---------- adding a factor (RFC 102 B7) ----------

/// How a user with **no** second factor proves themselves before adding
/// their first one (RFC 102 B7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstFactorProof {
    /// A local account: the current password, re-entered.
    LocalPassword,
    /// An account from a directory user source: the directory password,
    /// verified by re-binding.
    DirectoryPassword,
    /// A federated account. Upstream re-authentication (`prompt=login`,
    /// `max_age=0`) is not built yet, so enrolment is refused.
    Unavailable,
}

/// Decide which [`FirstFactorProof`] applies to `user_id`. A user with a
/// federation link is federated, whatever `source` says: federation
/// provisioning stores its users with `source = ldap`.
pub async fn first_factor_proof(db: &Database, user_id: UserId) -> CoreResult<FirstFactorProof> {
    use sui_id_store::models::UserSource;
    use sui_id_store::repos::{federation_link, users};
    if !federation_link::list_for_user(db, user_id)
        .await?
        .is_empty()
    {
        return Ok(FirstFactorProof::Unavailable);
    }
    let user = users::get(db, user_id).await?;
    Ok(match user.source {
        UserSource::Local => FirstFactorProof::LocalPassword,
        UserSource::Ldap => FirstFactorProof::DirectoryPassword,
    })
}

/// Check a local user's current password for B7. `Ok(false)` for a wrong
/// password or a user without a local credential.
pub async fn verify_current_password(
    db: &Database,
    user_id: UserId,
    password: &str,
) -> CoreResult<bool> {
    use sui_id_store::repos::credentials;
    let cred = match credentials::get(db, user_id).await {
        Ok(c) => c,
        Err(sui_id_store::StoreError::NotFound) => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    Ok(crate::password::verify_password(password, &cred.password_hash).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password;
    use chrono::Utc;
    use sui_id_shared::ids::UserId;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::{CredentialRow, UserRow};
    use sui_id_store::repos::{credentials, user_totp, users};

    fn fresh_db() -> Database {
        Database::open_in_memory(MasterKey::generate()).expect("db")
    }

    async fn create_user(db: &Database) -> UserId {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "u".into(),
                display_name: None,
                is_admin: false,
                role: if false {
                    sui_id_store::models::Role::Admin
                } else {
                    sui_id_store::models::Role::User
                },
                last_login_at: None,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
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
        .expect("create user");
        let phc = password::hash_password("the-tester-password").expect("hash");
        credentials::upsert(
            db,
            &CredentialRow {
                user_id: id,
                password_hash: phc,
                must_change: false,
                updated_at: now,
            },
        )
        .await
        .expect("cred");
        id
    }

    async fn enrol_totp(db: &Database, user_id: UserId) {
        // Enrolment goes through pending-then-confirm; for tests we
        // just need an "MFA enrolled" row, so do both halves.
        user_totp::upsert_pending(db, user_id, b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09")
            .await
            .expect("upsert pending");
        user_totp::confirm_with_recovery(db, user_id, b"[]")
            .await
            .expect("confirm");
    }

    #[tokio::test]
    async fn user_with_no_mfa_is_always_allowed() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS)
            .await
            .unwrap();
        assert_eq!(r, StepUpDecision::Allow);
        let r = policy_for_session(
            &db,
            &clock,
            uid,
            Some(Utc::now() - Duration::days(7)),
            STEP_UP_FRESHNESS_SECS,
        )
        .await
        .unwrap();
        assert_eq!(r, StepUpDecision::Allow);
    }

    #[tokio::test]
    async fn mfa_user_with_no_step_up_must_challenge() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        enrol_totp(&db, uid).await;
        let r = policy_for_session(&db, &clock, uid, None, STEP_UP_FRESHNESS_SECS)
            .await
            .unwrap();
        assert_eq!(r, StepUpDecision::Challenge);
    }

    #[tokio::test]
    async fn mfa_user_with_fresh_step_up_is_allowed() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        enrol_totp(&db, uid).await;
        let now = clock.now();
        let r = policy_for_session(&db, &clock, uid, Some(now), STEP_UP_FRESHNESS_SECS)
            .await
            .unwrap();
        assert_eq!(r, StepUpDecision::Allow);
    }

    #[tokio::test]
    async fn mfa_user_with_stale_step_up_must_challenge_again() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        enrol_totp(&db, uid).await;
        let stale = clock.now() - Duration::seconds(STEP_UP_FRESHNESS_SECS + 60);
        let r = policy_for_session(&db, &clock, uid, Some(stale), STEP_UP_FRESHNESS_SECS)
            .await
            .unwrap();
        assert_eq!(r, StepUpDecision::Challenge);
    }

    async fn fresh_session(db: &Database, clock: &SharedClock, uid: UserId) -> SessionId {
        use sui_id_store::models::SessionRow;
        let session_id = SessionId::new();
        let now = clock.now();
        // RFC 102 A4: a session is created by signing in (L01).
        sui_id_store::commands::sign_in_with_password(
            db,
            SessionRow {
                id: session_id,
                user_id: uid,
                expires_at: now + Duration::hours(8),
                created_at: now,
                revoked_at: None,
                auth_methods: vec![sui_id_shared::AuthMethod::Pwd],
                last_step_up_at: None,
                last_used_at: None,
            },
        )
        .await
        .expect("sign in");
        session_id
    }

    #[tokio::test]
    async fn verify_totp_code_with_correct_code_marks_session_fresh() {
        use crate::totp;
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        // Enrol TOTP with a known secret so we can compute the
        // expected code locally.
        let secret = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";
        user_totp::upsert_pending(&db, uid, secret)
            .await
            .expect("pending");
        user_totp::confirm_with_recovery(&db, uid, b"[]")
            .await
            .expect("confirm");

        let session_id = fresh_session(&db, &clock, uid).await;
        let now = clock.now().timestamp();
        let step = now / 30;
        let code = totp::code_for_step(secret, step).await;

        verify_totp_code(
            &db,
            &clock,
            uid,
            session_id,
            &code.to_string(),
            "/me/security/mfa",
        )
        .await
        .expect("verify ok");

        let row = sui_id_store::repos::sessions::get(&db, session_id)
            .await
            .expect("get");
        assert!(row.last_step_up_at.is_some(), "session should be fresh");
    }

    #[tokio::test]
    async fn verify_totp_code_with_wrong_code_does_not_touch_session() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        let secret = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";
        user_totp::upsert_pending(&db, uid, secret)
            .await
            .expect("pending");
        user_totp::confirm_with_recovery(&db, uid, b"[]")
            .await
            .expect("confirm");

        let session_id = fresh_session(&db, &clock, uid).await;

        // Pass a code that's almost certainly wrong (a fixed value
        // that's unlikely to coincide with the real one — and even
        // if it did, the next pass would still be wrong).
        let result =
            verify_totp_code(&db, &clock, uid, session_id, "000000", "/me/security/mfa").await;
        assert!(matches!(
            result,
            Err(crate::errors::CoreError::InvalidCredentials)
        ));

        let row = sui_id_store::repos::sessions::get(&db, session_id)
            .await
            .expect("get");
        assert!(
            row.last_step_up_at.is_none(),
            "session must NOT be marked fresh on a failed verify"
        );
    }

    #[tokio::test]
    async fn verify_totp_code_for_user_without_totp_returns_invalid_credentials() {
        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        let session_id = fresh_session(&db, &clock, uid).await;

        let result =
            verify_totp_code(&db, &clock, uid, session_id, "123456", "/me/security/mfa").await;
        // Same error shape as a wrong code: a step-up form should
        // not leak whether MFA is enrolled.
        assert!(matches!(
            result,
            Err(crate::errors::CoreError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn finish_webauthn_refuses_pending_with_wrong_kind() {
        // A pending row tagged `Authenticate` (i.e. a login-MFA
        // ceremony) must NOT satisfy a step-up gate, even if the
        // user_id matches and the row hasn't expired. This test
        // pins that invariant — the kind check is the *whole*
        // reason migration 0013 widened the CHECK constraint.
        use sui_id_shared::ids::WebauthnPendingId;
        use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
        use sui_id_store::repos::webauthn_pending;

        let db = fresh_db();
        let clock = crate::time::system_clock();
        let uid = create_user(&db).await;
        let session_id = fresh_session(&db, &clock, uid).await;
        let pending_id = WebauthnPendingId::new();
        let now = clock.now();
        webauthn_pending::insert(
            &db,
            &WebauthnPendingRow {
                id: pending_id,
                kind: WebauthnPendingKind::Authenticate, // wrong kind
                user_id: Some(uid),
                state_json: "{}".into(),
                expires_at: now + Duration::seconds(60),
                created_at: now,
            },
        )
        .await
        .expect("insert");

        let issuer = "https://test.example";
        // The credential JSON is moot — the kind check fails first.
        let result = finish_webauthn(
            &db,
            &clock,
            issuer,
            uid,
            session_id,
            pending_id,
            r#"{"id":"x","rawId":"x","type":"public-key","response":{}}"#,
            "/me/security/mfa",
        )
        .await;
        assert!(matches!(
            result,
            Err(crate::errors::CoreError::InvalidCredentials)
        ));

        // The pending row is intact — refusing a step-up finish on
        // an Authenticate row must not consume it, so the legitimate
        // login-MFA flow that owns the row can still complete.
        let still_there = webauthn_pending::get(&db, pending_id)
            .await
            .expect("query")
            .expect("row preserved");
        assert_eq!(still_there.kind, WebauthnPendingKind::Authenticate);
    }

    #[tokio::test]
    async fn finish_webauthn_refuses_pending_for_other_user() {
        // Even a kind = StepUp pending must be refused if it
        // belongs to a different user. Prevents pending-id
        // smuggling across sessions.
        use sui_id_shared::ids::WebauthnPendingId;
        use sui_id_store::models::{WebauthnPendingKind, WebauthnPendingRow};
        use sui_id_store::repos::webauthn_pending;

        let db = fresh_db();
        let clock = crate::time::system_clock();
        let real_owner = create_user(&db).await;
        // Create a *different* user we'll pretend is the one
        // signed in.
        let imposter = {
            let id = UserId::new();
            let now = Utc::now();
            users::create(
                &db,
                &sui_id_store::models::UserRow {
                    id,
                    username: "imposter".into(),
                    display_name: None,
                    is_admin: false,
                    role: if false {
                        sui_id_store::models::Role::Admin
                    } else {
                        sui_id_store::models::Role::User
                    },
                    last_login_at: None,
                    is_disabled: false,
                    is_deleted: false,
                    user_uuid: uuid::Uuid::new_v4(),
                    created_at: now,
                    updated_at: now,
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
            .expect("imposter");
            id
        };
        let session_id = fresh_session(&db, &clock, imposter).await;
        let pending_id = WebauthnPendingId::new();
        let now = clock.now();
        webauthn_pending::insert(
            &db,
            &WebauthnPendingRow {
                id: pending_id,
                kind: WebauthnPendingKind::StepUp,
                user_id: Some(real_owner), // the rightful owner
                state_json: "{}".into(),
                expires_at: now + Duration::seconds(60),
                created_at: now,
            },
        )
        .await
        .expect("insert");

        let result = finish_webauthn(
            &db,
            &clock,
            "https://test.example",
            imposter,
            session_id,
            pending_id,
            r#"{"id":"x","rawId":"x","type":"public-key","response":{}}"#,
            "/me/security/mfa",
        )
        .await;
        assert!(matches!(
            result,
            Err(crate::errors::CoreError::InvalidCredentials)
        ));

        // Pending row was NOT consumed — owner can still complete.
        assert!(
            webauthn_pending::get(&db, pending_id)
                .await
                .expect("query")
                .is_some()
        );
    }
}
