//! RFC 096-B1 stage 6c, F05: terminal attempt failure (RFC 096 `:601-605`).
//!
//! > No attempt returns to pending: transport, token, validation, disabled
//! > provider, or mapping failure changes exchanging to failed or leaves it
//! > safely reconcilable as failed; the browser must start again. Error
//! > responses consume the matching attempt without token exchange.
//!
//! Two things happen here, deliberately as two separate, non-atomic
//! operations — the same reason stage 6b's `emit_must_attempt` is itself
//! non-atomic with anything: a Class-B event has no transaction to be
//! atomic *with*.
//!
//! 1. The guarded `pending|exchanging -> failed` transition
//!    ([`sui_id_store::repos::federation_login_attempt::terminalize_failed`]).
//!    No link, user, session or last-seen mutation — that is F05's own
//!    manifest row, and this module's `terminalize_and_audit` touches
//!    nothing else.
//! 2. The matching Class-B emission, through 6b's `emit_must_attempt`,
//!    using one of exactly four registry-declared descriptors — the
//!    manifest's own *"exact Class-B sum"* — selected by
//!    [`TerminalFailureReason`], a closed enum. Closed means a caller
//!    cannot invent a fifth event name; only this module's own
//!    `descriptor()` method decides which of the four fires.
//!
//! **Every caller discards this function's own return value** (there is
//! none to discard — it returns `()`): A6 (RFC 094 `:120`) requires the
//! caller's own redirect to proceed whether or not either internal step
//! succeeded, the same requirement 6b's P2 site already had to satisfy for
//! its one emission; this module generalizes that to all four events and
//! adds the state transition alongside it.
//!
//! ## The `reason` attribute's own closed set (stage 6c's fix round, R1)
//!
//! RFC 096 `:715`: *"Audit uses internal provider/user/link IDs and
//! bounded reason enums."* [`TerminalFailureReason`] alone bounds the
//! *event name*; the `reason` **attribute** the audit row carries is a
//! separate value, and a free-form `&'static str` there would be bounded
//! for `:714`'s privacy requirement (it cannot carry runtime data) but not
//! for `:715`'s. [`UpstreamFailureDetail`] and [`DeniedDetail`] close that
//! gap the same way `TerminalFailureReason` itself is closed: each is an
//! exhaustive `match` in its own `as_str()`, so a new call site cannot
//! invent a token without adding a variant, and adding one without
//! extending `as_str()` is a compile error (mirroring mutation (d) against
//! `TerminalFailureReason` itself, now also checked against each of
//! these). `TakeoverBlocked` and `LinkRequired` carry no detail type at
//! all — each has exactly one legitimate value today, so there is nothing
//! for a caller to choose and nothing to enumerate beyond the one literal
//! `descriptor()`'s own match already names.
//!
//! **The per-reason pairing is checked, not merely enumerated.** Because
//! each detail is an associated value on its own `TerminalFailureReason`
//! variant (`UpstreamFailure(UpstreamFailureDetail)`, `Denied
//! (DeniedDetail)`) rather than a second, independent parameter, a
//! `Denied` call site cannot pass an `UpstreamFailureDetail` — there is no
//! way to construct `TerminalFailureReason::Denied(UpstreamFailureDetail::
//! ...)` at all; the two types are unrelated. The wrong-pairing mistake
//! `:715`'s fix asked to make impossible is impossible by construction,
//! not by convention.

use chrono::{DateTime, Utc};
use sui_id_shared::ids::{FederationLoginAttemptId, FederationProviderId, UserId};
use sui_id_store::Database;
use sui_id_store::registry::{AuditAttributes, MustAttemptDescriptor};
use sui_id_store::repos::audit::emit_must_attempt;
use sui_id_store::repos::federation_login_attempt;
use sui_id_store::repos::federation_provider::{
    AUDIT_LINK_REQUIRED_DESCRIPTOR, AUDIT_SIGNIN_DENIED_DESCRIPTOR,
    AUDIT_SIGNIN_UPSTREAM_FAILURE_DESCRIPTOR, AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR,
};

/// The closed set of `reason` tokens a [`TerminalFailureReason::
/// UpstreamFailure`] emission can carry — every transport/token-level
/// failure talking to the upstream IdP, found after the attempt was
/// claimed. See `handlers/federation.rs`'s own call sites for which check
/// each one names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamFailureDetail {
    UpstreamErrorResponse,
    DiscoveryRejected,
    DiscoveryFailed,
    TokenExchangeFailed,
    TokenParseFailed,
    UserinfoFetchFailed,
}

impl UpstreamFailureDetail {
    fn as_str(self) -> &'static str {
        match self {
            Self::UpstreamErrorResponse => "upstream_error_response",
            Self::DiscoveryRejected => "discovery_rejected",
            Self::DiscoveryFailed => "discovery_failed",
            Self::TokenExchangeFailed => "token_exchange_failed",
            Self::TokenParseFailed => "token_parse_failed",
            Self::UserinfoFetchFailed => "userinfo_fetch_failed",
        }
    }
}

/// The closed set of `reason` tokens a [`TerminalFailureReason::Denied`]
/// emission can carry — every post-claim denial that is none of the other
/// three events' own, more specific case: token-shape, claims-validation,
/// identity-mapping, disabled-provider/user, or sign-in commit failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeniedDetail {
    ClientSecretDecryptFailed,
    /// RFC 096-B1 stage 12a, §2: the sealed-state cookie's own PKCE
    /// verifier no longer has a copy to fall back on -- the sole source is
    /// now the attempt row's own `pkce_verifier_sealed`, opened a second
    /// time with the same AAD `claim`'s own tamper check just used. Not
    /// observed to actually fail in practice (the same inputs that let
    /// `claim` succeed guarantee this open succeeds too), carried for the
    /// same defense-in-depth reason `ClientSecretDecryptFailed` is.
    PkceVerifierDecryptFailed,
    MissingIdToken,
    ConfigDrift,
    SignatureInvalid,
    NonceMismatch,
    IdentityClaimsInvalid,
    TimeClaimsInvalid,
    OptionalClaimsInvalid,
    IdentityResolutionDbError,
    LinkUpsertDbError,
    UnverifiedEmail,
    /// RFC 096-B1 stage 7, F04: the provisioning transaction refused the
    /// write — a lost race against a concurrent provisioning of the same
    /// upstream identity, username or email (`StoreError::Conflict`), or
    /// any other failure committing it. Replaces the two separate,
    /// untransacted DB-error reasons (`provision_shadow_user_db_error`,
    /// `provision_link_db_error`) the inline two-write legacy code used —
    /// one transaction now fails as one thing.
    ProvisionTransactionFailed,
    AccountDisabledOrDeleted,
    UserReadDbError,
    MfaStateReadDbError,
    PendingMfaIssueFailed,
    SigninCommitFailed,
    /// RFC 096-B1 stage 9, F02: the guarded attempt-completion-plus-
    /// pending-row transaction refused the write — a lost race (the
    /// attempt no longer `exchanging`, or the link gone since it was
    /// resolved — `StoreError::Conflict`), or any other failure
    /// committing it. The same one-transaction-fails-as-one-thing shape
    /// `ProvisionTransactionFailed` already uses for F04.
    MfaPendingTransactionFailed,
}

impl DeniedDetail {
    fn as_str(self) -> &'static str {
        match self {
            Self::ClientSecretDecryptFailed => "client_secret_decrypt_failed",
            Self::PkceVerifierDecryptFailed => "pkce_verifier_decrypt_failed",
            Self::MissingIdToken => "missing_id_token",
            Self::ConfigDrift => "config_drift",
            Self::SignatureInvalid => "signature_invalid",
            Self::NonceMismatch => "nonce_mismatch",
            Self::IdentityClaimsInvalid => "identity_claims_invalid",
            Self::TimeClaimsInvalid => "time_claims_invalid",
            Self::OptionalClaimsInvalid => "optional_claims_invalid",
            Self::IdentityResolutionDbError => "identity_resolution_db_error",
            Self::LinkUpsertDbError => "link_upsert_db_error",
            Self::UnverifiedEmail => "unverified_email",
            Self::ProvisionTransactionFailed => "provision_transaction_failed",
            Self::AccountDisabledOrDeleted => "account_disabled_or_deleted",
            Self::UserReadDbError => "user_read_db_error",
            Self::MfaStateReadDbError => "mfa_state_read_db_error",
            Self::PendingMfaIssueFailed => "pending_mfa_issue_failed",
            Self::SigninCommitFailed => "signin_commit_failed",
            Self::MfaPendingTransactionFailed => "mfa_pending_transaction_failed",
        }
    }
}

/// F05's closed reason enum — the manifest's own *"exact Class-B sum"*.
/// Picks which of the four registry-declared descriptors
/// [`terminalize_and_audit`] emits. `UpstreamFailure` and `Denied` each
/// carry their own closed detail (module doc, above); `TakeoverBlocked`
/// and `LinkRequired` carry none, because each has exactly one legitimate
/// `reason` value today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalFailureReason {
    /// A transport/token-level failure talking to the upstream IdP, found
    /// after the attempt was claimed (discovery, token exchange, token
    /// response parsing).
    UpstreamFailure(UpstreamFailureDetail),
    /// RFC 096 `:707-708`'s P2: an upstream identity's email collides with
    /// an existing, unlinked local account under `provision_on_first_login`.
    TakeoverBlocked,
    /// RFC 096 `:699-701`: the generic `link_only` denial for any unlinked
    /// upstream identity.
    LinkRequired,
    /// Every other post-claim denial: validation, mapping, disabled
    /// provider/user, or sign-in commit failure.
    Denied(DeniedDetail),
}

impl TerminalFailureReason {
    fn descriptor(self) -> &'static MustAttemptDescriptor {
        match self {
            Self::UpstreamFailure(_) => &AUDIT_SIGNIN_UPSTREAM_FAILURE_DESCRIPTOR,
            Self::TakeoverBlocked => &AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR,
            Self::LinkRequired => &AUDIT_LINK_REQUIRED_DESCRIPTOR,
            Self::Denied(_) => &AUDIT_SIGNIN_DENIED_DESCRIPTOR,
        }
    }

    /// The `reason` attribute's own value — closed the same way the event
    /// name is, via each sub-enum's own exhaustive `as_str()`.
    fn reason_token(self) -> &'static str {
        match self {
            Self::UpstreamFailure(d) => d.as_str(),
            Self::TakeoverBlocked => "email_collision",
            Self::LinkRequired => "unlinked_identity",
            Self::Denied(d) => d.as_str(),
        }
    }
}

/// F05 itself. `actor` is `None` for every call site before a local user is
/// resolved, and `Some(user_id)` for the few after (`complete_federated_
/// signin`'s own checks) — only [`TerminalFailureReason::Denied`]'s
/// descriptor declares actor as anything but `None`-required, so passing
/// `Some` for the other three would be a mismatch with their own
/// registry declaration (unenforced by `emit_must_attempt` itself, but
/// wrong regardless).
pub async fn terminalize_and_audit(
    db: &Database,
    attempt_id: FederationLoginAttemptId,
    reason: TerminalFailureReason,
    at: DateTime<Utc>,
    provider_id: FederationProviderId,
    actor: Option<UserId>,
) {
    let _ = federation_login_attempt::terminalize_failed(db, attempt_id, at).await;
    let attributes = AuditAttributes::builder()
        .attribute("provider_id", provider_id.to_string())
        .attribute("reason", reason.reason_token())
        .build();
    let _ = emit_must_attempt(
        db,
        reason.descriptor(),
        at,
        actor,
        None,
        "denied",
        attributes,
    )
    .await;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation_attempt_terminalize/tests.rs"]
mod tests;
