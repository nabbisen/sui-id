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
use sui_id_shared::ids::{
    FederationMfaPendingId, FederationWebauthnCeremonyId, WebauthnCredentialId,
};
use sui_id_store::Database;
use sui_id_store::models::FederationMfaPendingStatus;
use sui_id_store::repos::{
    federation_mfa_pending, federation_provider, federation_webauthn_ceremony, user_totp,
    user_webauthn_credentials, users,
};
use webauthn_rs::prelude::{Passkey, PasskeyAuthentication, PublicKeyCredential};
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

/// RFC 096-B1 stage 11: the fourth and last candidate family member,
/// added now that F06's ceremony read exists to produce one (stage
/// 10a's own doc, quoted above, named this stage as the one that
/// would). Same treatment as [`ValidTotp`]/[`ValidRecovery`]: private
/// fields, no public constructor, no `Clone`/`Debug`/`Serialize`.
/// `ceremony_id` is what lets F03 durably re-confirm, inside its own
/// transaction, that the ceremony it is about to consume is the *same*
/// one this candidate was built against — the dispatch's own
/// `CeremonyInvalid` description ("a ceremony that is ... bound to
/// something other than what was presented") covers a concurrent F06
/// call replacing it in the window between this struct's construction
/// and F03's own call, not only a mismatched method.
/// `credential_id`/`expected_sealed`/`new_sealed` are the same
/// compare-and-swap shape [`ValidRecovery`] already commits for the
/// recovery-code blob, applied to the matched passkey's own stored
/// blob instead.
pub struct ValidWebAuthn {
    #[allow(dead_code)]
    ceremony_id: FederationWebauthnCeremonyId,
    #[allow(dead_code)]
    credential_id: WebauthnCredentialId,
    #[allow(dead_code)]
    expected_sealed: Vec<u8>,
    #[allow(dead_code)]
    new_sealed: Vec<u8>,
}

/// The outcome of one verification attempt. Not a candidate itself —
/// `Expired` is deliberately not one, see its own doc — but the one
/// type a caller matches on to learn whether binding even reached a
/// proof check.
pub enum FederatedMfaOutcome {
    Totp(ValidTotp),
    Recovery(ValidRecovery),
    /// RFC 096-B1 stage 11: produced only by [`verify_webauthn`], never
    /// by [`verify`] — the two entry points correspond to the two
    /// input shapes RFC 096 `:752-762` never asks one function to
    /// accept at once (a short text code versus a JSON-serialized
    /// `PublicKeyCredential`).
    Webauthn(ValidWebAuthn),
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
/// the property [`ValidTotp`]/[`ValidRecovery`]/[`ValidWebAuthn`]/
/// [`BoundRejected`] must not have. This impl prints only which variant
/// matched, never a candidate's own field — enough for tests and logs
/// to say *that* binding succeeded or a proof was rejected, never
/// *what* was proven.
impl std::fmt::Debug for FederatedMfaOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Totp(_) => "FederatedMfaOutcome::Totp(..)",
            Self::Recovery(_) => "FederatedMfaOutcome::Recovery(..)",
            Self::Webauthn(_) => "FederatedMfaOutcome::Webauthn(..)",
            Self::Rejected(_) => "FederatedMfaOutcome::Rejected(..)",
            Self::Expired => "FederatedMfaOutcome::Expired",
        })
    }
}

// ── The verifier's shared binding gate ──────────────────────────────────

/// What [`bind_pending`] found. Not [`FederatedMfaOutcome`] itself —
/// that type also carries the three candidates and [`BoundRejected`],
/// none of which a binding check alone ever produces.
enum Bound {
    Expired,
    Row(Box<sui_id_store::models::FederationMfaPendingRow>),
}

/// RFC 096 `:752-762`'s binding gate, shared by every entry point this
/// module has (RFC 096-B1 stage 11 extracts it from stage 10a's own
/// `verify`, unchanged in order or behavior, so that [`verify`] (TOTP/
/// recovery), [`verify_webauthn`] and [`begin_webauthn_ceremony`]'s own
/// core-side half all check it exactly once, the same way, rather than
/// three call sites drifting apart over time). Validates, in this
/// order: the pending id parses and names a real row; the browser-
/// binding cookie and the CSRF body value, both constant-time against
/// the row's own digests (`subtle::ConstantTimeEq`, the same mechanism
/// `federation_login_attempt::claim` already uses for its own browser
/// binding); the row's user is still active; the row's provider is
/// still enabled; the row is still `pending`. Every one of those six
/// refusals is [`BindingFailure`] — the same value, see its own doc.
/// Only after all six does expiry get its own answer ([`Bound::Expired`]).
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
async fn bind_pending(
    db: &Database,
    clock: &SharedClock,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
) -> Result<Bound, VerifyError> {
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
        return Ok(Bound::Expired);
    }

    Ok(Bound::Row(Box::new(row)))
}

// ── The verifier's TOTP/recovery entry point ────────────────────────────

/// RFC 096 `:752-762`, the text/code half. Runs [`bind_pending`] first;
/// only after that does a supported-method submission get checked
/// against the row's bound identity.
///
/// **`MethodMismatch`, direction 2** (RFC 096-B1 stage 11, `:802`): a
/// live WebAuthn ceremony already bound to this row means the row is
/// committed to that method for this attempt — a code submission here
/// is the wrong method, not a wrong code, and is rejected before the
/// TOTP row is even read (no TOTP enrolment is required to reach this
/// check, unlike everything below it).
///
/// `code_input` is interpreted the same way the local path's
/// `mfa::verify_pending` already does: digits parse as a TOTP code,
/// anything else is tried as a recovery code. The decrypted TOTP secret
/// is zeroized immediately after the comparison, on every path
/// including a wrong code — the same point the local path zeroizes its
/// own copy.
///
/// **Errors** (stage 10a fix round, R3): [`VerifyError::Binding`] for
/// the six [`bind_pending`] refusals, [`VerifyError::Internal`] for
/// everything else a store call can fail with — see [`VerifyError`]'s
/// own doc for why the two must not collapse into one.
pub async fn verify(
    db: &Database,
    clock: &SharedClock,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    code_input: &str,
) -> Result<FederatedMfaOutcome, VerifyError> {
    let row = match bind_pending(
        db,
        clock,
        presented_pending_id,
        presented_browser_binding_cookie,
        presented_csrf_body_value,
    )
    .await?
    {
        Bound::Expired => return Ok(FederatedMfaOutcome::Expired),
        Bound::Row(row) => *row,
    };

    if federation_webauthn_ceremony::find_by_pending_id(db, row.id)
        .await
        .map_err(classify_store_error)?
        .is_some()
    {
        return Ok(FederatedMfaOutcome::Rejected(BoundRejected {
            reason: RejectedReason::MethodMismatch,
        }));
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

// ── The verifier's WebAuthn entry point ─────────────────────────────────

/// RFC 096 `:752-762`, the WebAuthn half — a separate entry point from
/// [`verify`] (RFC 096-B1 stage 11's own §3: the input shape is a
/// JSON-serialized `PublicKeyCredential`, not a short text code, so
/// there is no single signature both could share the way TOTP/recovery
/// already share one). Runs [`bind_pending`] first, the same as
/// [`verify`].
///
/// **`MethodMismatch`, direction 1** (`:802`): no live
/// `federation_webauthn_ceremony` row for this pending id means this
/// row was never committed to WebAuthn for this attempt — F06 is what
/// creates one, and nothing here tries to tell "never called F06"
/// apart from "a ceremony already consumed by an earlier completion";
/// both are the same wrong-method signal against this row, the mirror
/// image of direction 2 in [`verify`].
///
/// **Revalidating the sealed ceremony state**: `ceremony_state_sealed`
/// is opened with the ceremony's own AAD
/// (`federation_webauthn_ceremony::build_aad`) — a tamper check on the
/// row's own identity, the same mechanism `continuation_sealed` already
/// uses (stage 9), not re-checked a second time by F03: F03's own
/// durable recheck (RFC 096 `:794-798`) reopens it again inside its own
/// transaction, independently, the same "verifier's read is never
/// trusted a second time" discipline stage 10b already established for
/// `continuation_sealed`.
///
/// **Matching the credential, and updating its counter** mirrors
/// `webauthn::finish_authentication`'s own local-path sequence
/// exactly: resolve which credential matched
/// (`result.cred_id()`/`find_by_credential_id`), confirm it belongs to
/// the row's own user (an impersonation refusal, [`BindingFailure`],
/// not a counted wrong proof — the same shape the local path's own
/// `row.user_id != expected_user_id` check already takes), decrypt the
/// current blob, call `Passkey::update_credential`, reseal. The
/// *durable, race-safe* write of the result is F03's job
/// (`ValidWebAuthn`'s own `expected_sealed`/`new_sealed` compare-and-
/// swap pair) — this function only computes what to write, the same
/// division [`verify`]'s own `match_recovery_code` already draws for
/// the recovery-code blob.
///
/// **Errors**: [`VerifyError::Binding`] for the six [`bind_pending`]
/// refusals plus the impersonation check above; [`VerifyError::Internal`]
/// for everything else, including a failed ceremony-state open (data
/// corruption, not a verdict on the presented assertion) and a failed
/// `webauthn::build` (a config problem, not the submitter's fault —
/// `CoreError`'s own text is folded into [`sui_id_store::StoreError::InvalidData`]
/// since [`VerifyError::Internal`] cannot name a `sui-id-core` error
/// type any more than `sui-id-store` itself can, the same seam
/// `convert_rejection_reason`'s own doc names in the other direction).
/// A cryptographically wrong assertion is not an error at all: it is
/// `Ok(FederatedMfaOutcome::Rejected(BoundRejected { reason: Webauthn }))`,
/// the same "a verdict, not a failure" treatment a wrong TOTP code
/// already gets in [`verify`].
pub async fn verify_webauthn(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    credential: &PublicKeyCredential,
) -> Result<FederatedMfaOutcome, VerifyError> {
    let row = match bind_pending(
        db,
        clock,
        presented_pending_id,
        presented_browser_binding_cookie,
        presented_csrf_body_value,
    )
    .await?
    {
        Bound::Expired => return Ok(FederatedMfaOutcome::Expired),
        Bound::Row(row) => *row,
    };

    let Some(ceremony) = federation_webauthn_ceremony::find_by_pending_id(db, row.id)
        .await
        .map_err(classify_store_error)?
    else {
        return Ok(FederatedMfaOutcome::Rejected(BoundRejected {
            reason: RejectedReason::MethodMismatch,
        }));
    };

    let aad = federation_webauthn_ceremony::build_aad(
        row.id,
        ceremony.provider_id,
        ceremony.provider_config_version,
        ceremony.provider_activation_generation,
        ceremony.user_id,
    );
    let state_plain = sui_id_store::crypto::open(db.key(), &ceremony.ceremony_state_sealed, &aad)
        .map_err(|_| {
        VerifyError::Internal(sui_id_store::StoreError::Integrity(
            "federated webauthn ceremony state failed to open".into(),
        ))
    })?;
    let auth_state: PasskeyAuthentication = serde_json::from_slice(&state_plain).map_err(|_| {
        VerifyError::Internal(sui_id_store::StoreError::InvalidData(
            "corrupt federated webauthn ceremony state".into(),
        ))
    })?;

    let webauthn = crate::webauthn::build(issuer_url)
        .await
        .map_err(|e| VerifyError::Internal(sui_id_store::StoreError::InvalidData(e.to_string())))?;

    let result = match webauthn.finish_passkey_authentication(credential, &auth_state) {
        Ok(result) => result,
        Err(_) => {
            return Ok(FederatedMfaOutcome::Rejected(BoundRejected {
                reason: RejectedReason::Webauthn,
            }));
        }
    };

    let cred_row = user_webauthn_credentials::find_by_credential_id(db, result.cred_id().as_ref())
        .await
        .map_err(classify_store_error)?
        .ok_or(VerifyError::Binding(BindingFailure))?;
    if cred_row.user_id != row.user_id {
        return Err(VerifyError::Binding(BindingFailure));
    }
    let expected_sealed = cred_row.passkey_enc.clone();
    let mut passkey: Passkey = {
        let blob = user_webauthn_credentials::decrypt_passkey(db, &cred_row)
            .await
            .map_err(classify_store_error)?;
        serde_json::from_slice(&blob).map_err(|_| {
            VerifyError::Internal(sui_id_store::StoreError::InvalidData(
                "corrupt passkey blob".into(),
            ))
        })?
    };
    let _changed = passkey.update_credential(&result);
    let new_plain = serde_json::to_vec(&passkey).map_err(|_| {
        VerifyError::Internal(sui_id_store::StoreError::InvalidData(
            "passkey re-encode failed".into(),
        ))
    })?;
    let new_sealed =
        user_webauthn_credentials::seal_passkey(db, &new_plain).map_err(classify_store_error)?;

    Ok(FederatedMfaOutcome::Webauthn(ValidWebAuthn {
        ceremony_id: ceremony.id,
        credential_id: cred_row.id,
        expected_sealed,
        new_sealed,
    }))
}

/// RFC 096-B1 stage 11, F06's `sui-id-core`-side half: this is the only
/// place `webauthn-rs` interaction for the federated ceremony can live
/// (stage 11 dispatch `:49-50`: "webauthn-rs is a sui-id-core
/// dependency, not sui-id-store's"). Mirrors
/// `webauthn::start_authentication`'s own local-path sequence —
/// build the `Webauthn` instance, decrypt every enrolled passkey,
/// `start_passkey_authentication` — then seals the resulting
/// `PasskeyAuthentication` state directly with
/// `sui_id_store::crypto::seal`/`db.key()` (both already `pub`, the
/// same route `communication::mail::outbox` already uses from this
/// crate) before handing the sealed bytes to the store-side F06
/// command, which never opens them (its own doc explains why).
///
/// Runs [`bind_pending`] first, the same as [`verify`]/[`verify_webauthn`]:
/// F06 "grants no authentication authority by itself" (RFC 096
/// `:822-824`), but issuing a browser challenge at all is still
/// something only a bound, unexpired, authority-intact request should
/// be able to trigger — the same reasoning [`verify`]/[`verify_webauthn`]
/// already apply to reading a proof.
///
/// Returns the challenge JSON the browser passes to
/// `navigator.credentials.get()`, mirroring
/// `webauthn::AuthenticationStart::challenge_json`.
///
/// **Errors**: [`VerifyError::Binding`] for the [`bind_pending`]
/// refusals and "no WebAuthn credentials enrolled" (RFC 096
/// `:753-755`'s own binding-gate shape, not a different kind of
/// refusal — a caller cannot distinguish "wrong cookie" from "no
/// passkey to challenge" any more than it could distinguish any other
/// `BindingFailure`); `VerifyError::Internal` for a store failure or a
/// failed `webauthn::build`/`start_passkey_authentication`, and for
/// [`sui_id_store::commands::begin_federation_webauthn_ceremony`]'s own
/// `StoreError::Conflict` — by the time this function's own `bind_pending`
/// has already passed, a `Conflict` from F06 itself can only be the
/// narrow window race F06's own doc names (authority drifted between
/// this read and F06's transaction), not a verdict on anything the
/// caller presented.
#[allow(clippy::too_many_arguments)]
pub async fn begin_webauthn_ceremony(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    rp_id: &str,
    origin: &str,
) -> Result<String, VerifyError> {
    let row = match bind_pending(
        db,
        clock,
        presented_pending_id,
        presented_browser_binding_cookie,
        presented_csrf_body_value,
    )
    .await?
    {
        Bound::Expired => {
            return Err(VerifyError::Binding(BindingFailure));
        }
        Bound::Row(row) => *row,
    };

    let webauthn = crate::webauthn::build(issuer_url)
        .await
        .map_err(|e| VerifyError::Internal(sui_id_store::StoreError::InvalidData(e.to_string())))?;
    let creds = user_webauthn_credentials::list_for_user(db, row.user_id)
        .await
        .map_err(classify_store_error)?;
    if creds.is_empty() {
        return Err(VerifyError::Binding(BindingFailure));
    }
    let mut passkeys: Vec<Passkey> = Vec::with_capacity(creds.len());
    for c in &creds {
        let blob = user_webauthn_credentials::decrypt_passkey(db, c)
            .await
            .map_err(classify_store_error)?;
        let pk: Passkey = serde_json::from_slice(&blob).map_err(|_| {
            VerifyError::Internal(sui_id_store::StoreError::InvalidData(
                "corrupt passkey blob".into(),
            ))
        })?;
        passkeys.push(pk);
    }
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| VerifyError::Internal(sui_id_store::StoreError::InvalidData(e.to_string())))?;
    let state_plain = serde_json::to_vec(&auth_state).map_err(|_| {
        VerifyError::Internal(sui_id_store::StoreError::InvalidData(
            "ceremony state re-encode failed".into(),
        ))
    })?;
    let aad = federation_webauthn_ceremony::build_aad(
        row.id,
        row.provider_id,
        row.provider_config_version,
        row.provider_activation_generation,
        row.user_id,
    );
    let ceremony_state_sealed =
        sui_id_store::crypto::seal(db.key(), &state_plain, &aad).map_err(classify_store_error)?;

    sui_id_store::commands::begin_federation_webauthn_ceremony(
        db,
        row.id,
        ceremony_state_sealed,
        rp_id.to_owned(),
        origin.to_owned(),
        clock.now(),
    )
    .await
    .map_err(classify_f06_error)?;

    let challenge_json = serde_json::to_string(&rcr).map_err(|_| {
        VerifyError::Internal(sui_id_store::StoreError::InvalidData(
            "challenge re-encode failed".into(),
        ))
    })?;
    Ok(challenge_json)
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
        FederatedMfaOutcome::Webauthn(_) => {
            unreachable!(
                "verify() never produces FederatedMfaOutcome::Webauthn; only verify_webauthn() does"
            )
        }
    };

    sui_id_store::commands::complete_federated_mfa(db, pending_id, proof, clock.now())
        .await
        .map_err(classify_f03_error)
}

/// The WebAuthn twin of [`promote`] — same shape, [`verify_webauthn`]
/// in place of [`verify`], for the same reason the two have separate
/// entry points at all (the input shapes do not share a signature).
pub async fn promote_webauthn(
    db: &Database,
    clock: &SharedClock,
    issuer_url: &str,
    presented_pending_id: &str,
    presented_browser_binding_cookie: &str,
    presented_csrf_body_value: &str,
    credential: &PublicKeyCredential,
) -> Result<sui_id_store::commands::F03Outcome, VerifyError> {
    let outcome = verify_webauthn(
        db,
        clock,
        issuer_url,
        presented_pending_id,
        presented_browser_binding_cookie,
        presented_csrf_body_value,
        credential,
    )
    .await?;

    let pending_id: FederationMfaPendingId = presented_pending_id
        .parse()
        .map_err(|_| VerifyError::Binding(BindingFailure))?;

    let proof = match outcome {
        FederatedMfaOutcome::Webauthn(valid) => Some(sui_id_store::commands::F03Proof::Webauthn {
            ceremony_id: valid.ceremony_id,
            credential_id: valid.credential_id,
            expected_sealed: valid.expected_sealed,
            new_sealed: valid.new_sealed,
        }),
        FederatedMfaOutcome::Rejected(bound) => Some(sui_id_store::commands::F03Proof::Rejected {
            reason: convert_rejection_reason(bound.reason),
        }),
        FederatedMfaOutcome::Expired => None,
        FederatedMfaOutcome::Totp(_) | FederatedMfaOutcome::Recovery(_) => {
            unreachable!(
                "verify_webauthn() never produces FederatedMfaOutcome::Totp/Recovery; only verify() does"
            )
        }
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

/// The same two-case mapping [`classify_f03_error`] uses, named
/// separately rather than shared: F06's own `Conflict` is a different
/// set of guard failures (authority drift, expiry, or no WebAuthn
/// credential enrolled — `commands::begin_federation_webauthn_ceremony`'s
/// own doc), but by the time [`begin_webauthn_ceremony`] calls F06,
/// [`bind_pending`] has already passed, so a `Conflict` here is the
/// same kind of narrow, already-bound-but-now-stale race, folded into
/// the same indistinguishable [`BindingFailure`] value every other
/// refusal in this module already is.
fn classify_f06_error(e: sui_id_store::StoreError) -> VerifyError {
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
