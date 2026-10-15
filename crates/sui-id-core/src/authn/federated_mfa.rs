//! RFC 096-B1 stage 10a: the `FederatedMfaVerifier` (RFC 096 `:752-762`).
//!
//! ## Where this lives, and why
//!
//! RFC 096 `:752-762` requires that a binding failure "cannot construct
//! an F03 input" and that "handlers can neither construct a candidate."
//! F03 (stage 10b) is Class P and must live in `sui-id-store`:
//! `WriteTx::tx()` is `pub(crate)` to that crate, which is why F02
//! (stage 9) had to live there too. The candidate family therefore has
//! two places it could live, and the crate graph decides which one
//! actually works:
//!
//! - `sui-id-core/Cargo.toml` depends on `sui-id-store` and on
//!   `webauthn-rs`; `sui-id-store/Cargo.toml` depends on
//!   `sui-id-shared` only — the edge runs one way, so `sui-id-store`
//!   cannot name a `sui-id-core` type in any function signature.
//! - TOTP verification (`crate::totp::verify`) is pure computation over
//!   a secret and a step — it could move to `sui-id-store` cheaply.
//!   Recovery-code matching cannot move as cheaply: it calls
//!   `crate::password::verify_password` (argon2, `CoreError`-typed,
//!   `sui-id-core`'s own password-hashing surface — the same one the
//!   *local* password path uses), and duplicating or relocating that
//!   surface to satisfy the federated path is a change to
//!   security-sensitive, already-reviewed code this stage did not set
//!   out to touch.
//! - Adding `webauthn-rs` to `sui-id-store` (so the whole verifier could
//!   move down beside F03) is a new dependency in the crate that owns
//!   the database — an owner-visible trade this package does not make
//!   unilaterally, and moot regardless: WebAuthn is stage 11's.
//!
//! So the verifier and the whole candidate family stay in `sui-id-core`,
//! where the TOTP and recovery-code primitives already live. The
//! consequence, named rather than discovered later: **F03 (stage 10b)
//! cannot accept a candidate value as a parameter** — `sui-id-store`
//! has no way to even name the type. Stage 10b's own promotion logic
//! must therefore live on this side of the boundary too (called from
//! here, the same way this module already calls `sui-id-store`'s read
//! functions), passing F03 plain, already-verified primitives rather
//! than the candidate itself. This stage does not write that logic —
//! stage 10a makes no writes at all (`:4` of its own dispatch) — but the
//! shape it leaves for stage 10b to fill in is this one.
//!
//! ## What actually keeps a handler from constructing a candidate
//!
//! Not the crate boundary — Rust has no "public to exactly one
//! dependent crate" visibility, so a type reachable at all from
//! `sui-id-core` is reachable from every crate that depends on it,
//! including `sui-id`. What keeps a handler out is ordinary module
//! privacy: [`ValidTotp`], [`ValidRecovery`] and [`BoundRejected`] are
//! `pub` *types* with **private fields and no public constructor** —
//! the same shape `sui-id`'s own `IdentityCapability`
//! (`identity_capability.rs`) already uses, proven the same way:
//! `crates/sui-id/tests/compile_fail/federated_mfa_valid_totp_cannot_be_constructed_directly.rs`
//! tries the direct route and the crate's own test harness pins the
//! `rustc` diagnostic.
//!
//! This is also the comparison the dispatch asks this module to state:
//! [`sui_id_store::commands::SecondFactorProof`], L02's own analogue for
//! the *local* path, is a `pub enum` with fully public variants and
//! fields — anything that can see `sui-id-store::commands` can
//! construct one, including `crate::mfa`, which does
//! (`match_recovery_code`/`verify_pending`). That was never claimed to
//! be stronger than convention. RFC 096 `:758-762` asks the federated
//! path to be stronger than that, and the private-field candidates here
//! are the whole of the difference.

use crate::password::verify_password;
use crate::time::SharedClock;
use crate::totp;
use serde_json;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use sui_id_shared::ids::FederationMfaPendingId;
use sui_id_store::Database;
use sui_id_store::models::FederationMfaPendingStatus;
use sui_id_store::repos::{federation_mfa_pending, federation_provider, user_totp, users};
use zeroize::Zeroize;

// ── The one binding-failure value ───────────────────────────────────────

/// RFC 096 `:754-755`: "Binding failure returns only an unbound generic
/// error and cannot construct an F03 input or touch the row." No
/// fields, no variants — every one of a wrong browser-binding cookie, a
/// wrong CSRF value, an unparseable or unknown pending id, a pending id
/// bound to a different user than the one it names, a disabled user, a
/// disabled provider, and a row that is no longer `pending` produces
/// this exact value. `tests.rs` asserts several of these are literally
/// `==` to each other, not merely that each is *an* error — a unit
/// struct makes that trivially true, which is itself part of the proof:
/// there is no field left over for two call sites to disagree about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingFailure;

impl std::fmt::Display for BindingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("federated MFA binding check failed")
    }
}

impl std::error::Error for BindingFailure {}

/// [`verify`]'s own error type (stage 10a fix round, R3). Before this,
/// every store-level failure — a database outage, a decryption
/// failure, a corrupt recovery-code blob, a `FromSql` conversion
/// failure, seven call sites in all — was laundered into
/// [`BindingFailure`] exactly like a wrong cookie. RFC 096 `:754-755`
/// asks for one indistinguishable value on *binding* failure; an
/// internal error is not one, and the local path this module otherwise
/// copies faithfully already draws the same line —
/// `mfa::verify_pending` (`mfa.rs:215-227`) propagates with `?` and
/// reserves its generic refusal for absence. Fail-closed does not
/// require discarding the distinction: the attacker-visible response
/// is identical either way, because a caller maps both variants to the
/// same page; what the distinction buys is an operator signal that
/// [`Binding`](Self::Binding) alone cannot carry.
#[derive(Debug)]
pub enum VerifyError {
    /// One of the six binding refusals [`BindingFailure`]'s own doc
    /// names, or the row's own `status` no longer being `pending`. A
    /// caller maps this, and only this, to RFC 096 `:754-755`'s generic
    /// MFA-failed response.
    Binding(BindingFailure),
    /// Everything else a store call can fail with — not a verdict on
    /// the presented proof at all. A caller maps this to an internal-
    /// error response and logs the wrapped [`StoreError`](sui_id_store::StoreError).
    Internal(sui_id_store::StoreError),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Internal(_) => f.write_str("internal failure during federated MFA verification"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// `StoreError::NotFound` is an absence, the same shape every other
/// binding refusal in [`verify`] already takes with `.ok_or(..)` —
/// folded in here rather than treated as internal. Every other
/// `StoreError` variant is exactly what [`VerifyError::Internal`]'s own
/// doc describes: not a verdict on the presented proof, something an
/// operator needs to see. The same two-way split
/// `complete_second_factor`'s own caller (`mfa.rs:320-323`) already
/// makes for the local path.
fn classify_store_error(e: sui_id_store::StoreError) -> VerifyError {
    match e {
        sui_id_store::StoreError::NotFound => VerifyError::Binding(BindingFailure),
        other => VerifyError::Internal(other),
    }
}

// ── The candidate family ─────────────────────────────────────────────────

/// RFC 096 `:798-799`'s closed rejection-reason set for F03 (stage 10b
/// writes with it; this stage only selects one). Unlike the candidates
/// below, a reason carries no secret and is safe to derive/compare —
/// what must stay closed is *who selects one*, not whether the value
/// itself can be copied or printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectedReason {
    Totp,
    Recovery,
    Webauthn,
    Malformed,
    MethodMismatch,
}

/// A TOTP code matched `step`, strictly after the row's own stored
/// `last_used_step` — not re-checked here; stage 10b's F03 re-reads and
/// compares durably under its own transaction, the same two-step shape
/// the *local* path's `SecondFactorProof::Totp` already uses. Private
/// field, no public constructor, no `Clone`/`Debug`/`Serialize` derive —
/// RFC 096 `:757-758`'s "sealed, non-cloneable, non-debuggable,
/// non-serializable" is the absence of those three derives, not a
/// runtime check; `tests/compile_fail/` proves the construction half,
/// and a second fixture proves a handler cannot even print one.
pub struct ValidTotp {
    /// Read by stage 10b's own promotion logic (also in `sui-id-core`,
    /// per this module's placement argument) once F03 exists to commit
    /// it — `#[allow(dead_code)]` for the same reason
    /// `registry::Audited::receipt` carries it: a field this stage's own
    /// tests construct and match on, but have no durable write to feed
    /// yet.
    #[allow(dead_code)]
    step: i64,
}

/// A recovery code matched one stored hash; `expected_sealed` is the
/// blob it was matched against and `new_sealed` is that blob with the
/// matched hash removed — the same pair `SecondFactorProof::RecoveryCode`
/// already commits for the local path, sealed here instead.
pub struct ValidRecovery {
    #[allow(dead_code)]
    expected_sealed: Vec<u8>,
    #[allow(dead_code)]
    new_sealed: Vec<u8>,
}

/// Binding succeeded — the row, the user, the provider and the browser/
/// CSRF cookies all checked out — but the submitted proof itself was
/// wrong, unsupported, or substituted a different method/ceremony than
/// the row is bound to (RFC 096 `:741-743`). Carries the one closed
/// reason the verifier itself selected — stage 10b reads it to pick
/// which of `RejectedStillPending`/`AttemptsExhausted` to write and
/// what note to carry; a handler can never construct a `BoundRejected`
/// bearing a reason of its own choosing, the same private-field shape
/// as the two success candidates, proven the same way.
pub struct BoundRejected {
    #[allow(dead_code)]
    reason: RejectedReason,
}

/// `ValidWebAuthn` is deliberately **not** declared here. Stage 11 owns
/// F06 and the `federation_webauthn_ceremony` table this stage has no
/// reader for at all — a variant nothing can ever produce would be
/// exactly the "half-built WebAuthn path" the dispatch asks this stage
/// not to leave. Stage 11 adds the variant when it adds the ceremony
/// read that could produce one.
///
/// The outcome of one verification attempt. Not a candidate itself —
/// `Expired` is deliberately not one, see its own doc — but the one
/// type a caller matches on to learn whether binding even reached a
/// proof check.
pub enum FederatedMfaOutcome {
    Totp(ValidTotp),
    Recovery(ValidRecovery),
    Rejected(BoundRejected),
    /// RFC 096 `:733-734`: `expires_at <= guard_at` is expired. Not
    /// folded into [`BindingFailure`]: `:740-741` assigns `Invalidated`
    /// to authority drift/expiry, and `Invalidated` is a *write* — this
    /// stage makes none, so the only honest thing to do with expiry is
    /// hand it on unexamined to whichever stage does write (10b's F03).
    /// That is the whole of the justification; it is not an instance of
    /// a wider "nothing left to hide once identity has resolved"
    /// principle — disabled user, disabled provider and a non-`pending`
    /// `status` are all checked after the same two binding digests pass,
    /// with the row's identity equally resolved by then, and all three
    /// *are* [`BindingFailure`] (RFC 096 `:752-753` puts the user and the
    /// provider inside the binding gate itself). This stage must not act
    /// on expiry itself.
    Expired,
}

/// A hand-written `Debug`, deliberately **not** derived: a derive would
/// require every variant's payload to be `Debug` too, which is exactly
/// the property [`ValidTotp`]/[`ValidRecovery`]/[`BoundRejected`] must
/// not have. This impl prints only which variant matched, never a
/// candidate's own field — enough for tests and logs to say *that*
/// binding succeeded or a proof was rejected, never *what* was proven.
impl std::fmt::Debug for FederatedMfaOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Totp(_) => "FederatedMfaOutcome::Totp(..)",
            Self::Recovery(_) => "FederatedMfaOutcome::Recovery(..)",
            Self::Rejected(_) => "FederatedMfaOutcome::Rejected(..)",
            Self::Expired => "FederatedMfaOutcome::Expired",
        })
    }
}

// ── The verifier's one entry point ──────────────────────────────────────

/// RFC 096 `:752-762`, the whole of it. Validates, in this order: the
/// pending id parses and names a real row; the browser-binding cookie
/// and the CSRF body value, both constant-time against the row's own
/// digests (`subtle::ConstantTimeEq`, the same mechanism
/// `federation_login_attempt::claim` already uses for its own browser
/// binding); the row's user is still active; the row's provider is
/// still enabled; the row is still `pending`. Every one of those six
/// refusals is [`BindingFailure`] — the same value, see its own doc.
/// Only after all six does expiry get its own answer
/// ([`FederatedMfaOutcome::Expired`]), and only after *that* does a
/// supported-method submission get checked against the row's bound
/// identity.
///
/// **The CSRF value must be the request-body value, not the cookie
/// read back.** `repos::federation_mfa_pending`'s own module doc states
/// why: the cookie is `HttpOnly`, so the only legitimate way its value
/// reaches a submission is a hidden field the MFA page renders
/// server-side — a cross-site submission cannot reproduce that even
/// though the cookie itself would ride along automatically. This
/// function trusts its caller to have sourced `presented_csrf_body_value`
/// from the request body; it has no way to enforce that itself, and
/// says so rather than silently assuming it.
///
/// **Provider version/generation**: RFC 096-B2/M2b's own durable
/// columns do not exist yet (`handlers/federation.rs`'s own
/// `placeholder_provider_version_and_generation`, `sui-id` crate,
/// confirms this directly — every provider's version/generation is
/// fixed at `(0, 0)` today). The row's own stored triple is therefore
/// re-checked against nothing live; only `provider.enabled` is a
/// meaningful recheck until M2b lands. Stated here rather than silently
/// matching stage 9's own recheck division.
///
/// `code_input` is interpreted the same way the local path's
/// `mfa::verify_pending` already does: digits parse as a TOTP code,
/// anything else is tried as a recovery code. The decrypted TOTP secret
/// is zeroized immediately after the comparison, on every path
/// including a wrong code — the same point the local path zeroizes its
/// own copy.
///
/// **Errors** (stage 10a fix round, R3): [`VerifyError::Binding`] for
/// the six refusals above, [`VerifyError::Internal`] for everything
/// else a store call can fail with — see [`VerifyError`]'s own doc for
/// why the two must not collapse into one.
pub async fn verify(
    db: &Database,
    clock: &SharedClock,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    code_input: &str,
) -> Result<FederatedMfaOutcome, VerifyError> {
    let pending_id: FederationMfaPendingId = presented_pending_id
        .parse()
        .map_err(|_| VerifyError::Binding(BindingFailure))?;
    let row = federation_mfa_pending::find_by_id(db, pending_id)
        .await
        .map_err(classify_store_error)?
        .ok_or(VerifyError::Binding(BindingFailure))?;

    let presented_browser_digest = Sha256::digest(presented_browser_binding_cookie.as_bytes());
    let browser_ok: bool = row
        .browser_binding_sha256
        .ct_eq(presented_browser_digest.as_slice())
        .into();
    let presented_csrf_digest = Sha256::digest(presented_csrf_body_value.as_bytes());
    let csrf_ok: bool = row
        .csrf_binding_sha256
        .ct_eq(presented_csrf_digest.as_slice())
        .into();
    if !browser_ok || !csrf_ok {
        return Err(VerifyError::Binding(BindingFailure));
    }

    let user = users::get(db, row.user_id)
        .await
        .map_err(classify_store_error)?;
    if user.is_disabled || user.is_deleted {
        return Err(VerifyError::Binding(BindingFailure));
    }

    let provider = federation_provider::get(db, row.provider_id)
        .await
        .map_err(classify_store_error)?;
    if !provider.enabled {
        return Err(VerifyError::Binding(BindingFailure));
    }

    if row.status != FederationMfaPendingStatus::Pending {
        return Err(VerifyError::Binding(BindingFailure));
    }

    if row.expires_at <= clock.now() {
        return Ok(FederatedMfaOutcome::Expired);
    }

    let totp_row = user_totp::get(db, row.user_id)
        .await
        .map_err(classify_store_error)?
        .ok_or(VerifyError::Binding(BindingFailure))?;
    if !totp_row.enabled {
        return Err(VerifyError::Binding(BindingFailure));
    }

    let trimmed = code_input.trim();
    // RFC 096 :741-743's "malformed" half of the one-failure sentence:
    // empty input is not a wrong TOTP guess and not a wrong recovery
    // code -- it is nothing to check at all. Without this, it would
    // fall through to the recovery-code branch below and come back as
    // `RejectedReason::Recovery`, which is the wrong reason for input
    // that was never a recovery-code attempt in the first place.
    if trimmed.is_empty() {
        return Ok(FederatedMfaOutcome::Rejected(BoundRejected {
            reason: RejectedReason::Malformed,
        }));
    }
    if let Ok(digits) = trimmed.parse::<u32>() {
        let mut secret = user_totp::decrypt_secret(db, &totp_row)
            .await
            .map_err(classify_store_error)?;
        let now = clock.now().timestamp();
        let result = totp::verify(&secret, now, digits, totp_row.last_used_step).await;
        secret.zeroize();
        Ok(match result {
            Some(step) => FederatedMfaOutcome::Totp(ValidTotp { step }),
            None => FederatedMfaOutcome::Rejected(BoundRejected {
                reason: RejectedReason::Totp,
            }),
        })
    } else {
        match match_recovery_code(db, &totp_row, trimmed)
            .await
            .map_err(classify_store_error)?
        {
            Some((expected_sealed, new_sealed)) => {
                Ok(FederatedMfaOutcome::Recovery(ValidRecovery {
                    expected_sealed,
                    new_sealed,
                }))
            }
            None => Ok(FederatedMfaOutcome::Rejected(BoundRejected {
                reason: RejectedReason::Recovery,
            })),
        }
    }
}

/// The recovery-code half of [`verify`], independent of `crate::mfa`'s
/// own private `match_recovery_code` (same algorithm, same primitives —
/// `user_totp::decrypt_recovery_codes`/`seal_recovery_codes`,
/// `crate::password::verify_password` — but this module does not reach
/// into `mfa.rs`'s own internals, and does not return
/// `sui_id_store::commands::SecondFactorProof`, the very type this
/// module's own doc argues against reusing for the federated path).
async fn match_recovery_code(
    db: &Database,
    totp_row: &sui_id_store::models::UserTotpRow,
    candidate: &str,
) -> sui_id_store::StoreResult<Option<(Vec<u8>, Vec<u8>)>> {
    let (Some(expected_sealed), Some(blob)) = (
        totp_row.recovery_codes_enc.clone(),
        user_totp::decrypt_recovery_codes(db, totp_row).await?,
    ) else {
        return Ok(None);
    };
    let mut hashes: Vec<String> = serde_json::from_slice(&blob)
        .map_err(|_| sui_id_store::StoreError::InvalidData("malformed recovery codes".into()))?;
    let mut matched = None;
    for (i, h) in hashes.iter().enumerate() {
        if verify_password(candidate, h).await.is_ok() {
            matched = Some(i);
            break;
        }
    }
    let Some(i) = matched else {
        return Ok(None);
    };
    hashes.remove(i);
    let new_blob = serde_json::to_vec(&hashes)
        .map_err(|_| sui_id_store::StoreError::InvalidData("recovery codes re-encode".into()))?;
    let new_sealed = user_totp::seal_recovery_codes(db, &new_blob)?;
    Ok(Some((expected_sealed, new_sealed)))
}

/// RFC 096-B1 stage 10b: the promotion path, on this side of the F03
/// seam (RFC 096 `:784-789`) — `sui-id-store` cannot name this module's
/// own candidate types, so F03 is a `pub` store command taking plain
/// primitives, and the code that converts a sealed candidate into one
/// lives here, beside the verifier that produced it, the same way
/// stage 10a's own placement argument already concluded it must.
///
/// Runs [`verify`] first, then [`sui_id_store::commands::complete_federated_mfa`]
/// with the candidate reduced to a primitive [`sui_id_store::commands::F03Proof`].
/// `FederatedMfaOutcome::Expired` becomes `proof: None` — the verifier's
/// own read already decided there was nothing to evaluate, and F03's
/// own durable recheck (not this function) is what actually terminalizes
/// it, inside its own transaction, with its own Class-B emission.
///
/// **Errors fold into the same two-case shape [`verify`] already uses**,
/// via [`classify_f03_error`]: `StoreError::NotFound` (the row vanished
/// between `verify`'s read and F03's write — the parent link cascaded it
/// away) and `StoreError::Conflict` (a concurrent call already decided
/// this row — RFC 096-B1 stage 10b dispatch `:55-58`'s equality
/// boundary) both become [`BindingFailure`], the same indistinguishable
/// value every other refusal in this module already is; every other
/// store error becomes [`VerifyError::Internal`], for the same reason
/// R3 (stage 10a's own fix round) drew that line in `verify` itself.
pub async fn promote(
    db: &Database,
    clock: &SharedClock,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    code_input: &str,
) -> Result<sui_id_store::commands::F03Outcome, VerifyError> {
    let outcome = verify(
        db,
        clock,
        presented_pending_id,
        presented_browser_binding_cookie,
        presented_csrf_body_value,
        code_input,
    )
    .await?;

    let pending_id: FederationMfaPendingId = presented_pending_id
        .parse()
        .map_err(|_| VerifyError::Binding(BindingFailure))?;

    let proof = match outcome {
        FederatedMfaOutcome::Totp(valid) => {
            Some(sui_id_store::commands::F03Proof::Totp { step: valid.step })
        }
        FederatedMfaOutcome::Recovery(valid) => Some(sui_id_store::commands::F03Proof::Recovery {
            expected_sealed: valid.expected_sealed,
            new_sealed: valid.new_sealed,
        }),
        FederatedMfaOutcome::Rejected(bound) => Some(sui_id_store::commands::F03Proof::Rejected {
            reason: convert_rejection_reason(bound.reason),
        }),
        FederatedMfaOutcome::Expired => None,
    };

    sui_id_store::commands::complete_federated_mfa(db, pending_id, proof, clock.now())
        .await
        .map_err(classify_f03_error)
}

/// The verifier's own closed reason, converted exhaustively to the
/// plain, store-level enum F03 actually takes (RFC 096 `:784-789`'s F03
/// seam: `sui-id-store` cannot name [`RejectedReason`] itself). This is
/// the one place that conversion happens — not F03, which has no way to
/// construct a [`RejectedReason`] to convert *from*.
fn convert_rejection_reason(
    reason: RejectedReason,
) -> sui_id_store::repos::federation_mfa_pending::MfaRejectionReason {
    use sui_id_store::repos::federation_mfa_pending::MfaRejectionReason as Store;
    match reason {
        RejectedReason::Totp => Store::Totp,
        RejectedReason::Recovery => Store::Recovery,
        RejectedReason::Webauthn => Store::Webauthn,
        RejectedReason::Malformed => Store::Malformed,
        RejectedReason::MethodMismatch => Store::MethodMismatch,
    }
}

/// `StoreError::NotFound`/`Conflict` are both "this submission reached a
/// dead end that is not a database malfunction" — folded into
/// [`BindingFailure`], the same indistinguishable value every other
/// refusal already is. Everything else is [`VerifyError::Internal`].
fn classify_f03_error(e: sui_id_store::StoreError) -> VerifyError {
    match e {
        sui_id_store::StoreError::NotFound | sui_id_store::StoreError::Conflict => {
            VerifyError::Binding(BindingFailure)
        }
        other => VerifyError::Internal(other),
    }
}

#[cfg(test)]
#[path = "federated_mfa/tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;
