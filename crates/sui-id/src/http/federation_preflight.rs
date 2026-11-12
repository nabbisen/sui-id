//! RFC 096-B2 S3: the preflight capability (RFC 096 `:409-421`), and RFC
//! 094 `:506-512`'s `ValidatedProviderPreflight`.
//!
//! **The consumer (RFC 096-B2 S4, the C17-seam amendment, `:440`):**
//! [`check_and_enable`] is the *only* function that runs
//! [`preflight_provider`] and moves its capability into
//! [`sui_id_store::commands::enable_federation_provider_from_preflight`].
//! Nothing else calls that store command — the source-scope test
//! (`crates/sui-id/tests/e2e/r096_b2_stage4_c17_scope.rs`) is what makes
//! that more than a comment.
//!
//! Sealed the same way as [`crate::identity_capability::IdentityCapability`]
//! and `sui-id-core`'s federated-MFA candidates: private fields, no public
//! constructor outside [`preflight_provider`], no `Clone`, `Copy`, `Debug`
//! or `Serialize`. Unlike those two, `provider_id`/`config_version`/
//! `activation_generation` carry no secret or PII, so this doc comment
//! states plainly (rather than redacting in a hand-written `Debug`) that
//! the *absence* of `Debug` is the whole mechanism — RFC 094 `:508`'s
//! "non-debuggable" is a derive this type declines to add, not a runtime
//! check.
//!
//! **Why the constructor's parameter types are not the proof here, unlike
//! `IdentityCapability`'s.** `preflight_provider` takes a database handle,
//! an HTTP client and a provider id — all three are ordinary, widely-held
//! types, not sealed intermediates a caller could only get from a prior
//! validation step. What makes this capability trustworthy is not its
//! parameters but its *body*: every field is set from a value this
//! function itself fetched, validated through the production path (RFC
//! 096 `:409-413`), and re-checked unchanged immediately before
//! construction (RFC 096 `:412-413`). There is no public way to assemble
//! one from already-known pieces, because there is no public field to
//! assemble from.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::federation_policy::PolicyDigest;

use crate::discovery;
use crate::jwks;
use crate::state::AppState;

/// RFC 096 `:409-421`'s closed set of ways a preflight can refuse, in
/// operator terms. Carries no upstream body, URL, query, or key
/// material (each variant is a bare unit) — detail for an operator's
/// own troubleshooting is logged separately, with `tracing`, at the
/// call site that actually saw it, the same split
/// [`discovery::FetchDiscoveryError`] already makes between what a log
/// line may say and what a typed signal upward may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightError {
    /// No provider has this id (it was deleted, or the id is stale).
    NotFound,
    /// The provider could not be read from the database.
    Storage,
    /// The provider is already enabled.
    NotDisabled,
    /// No `[[federation_providers]]` block names this provider.
    NotInConfiguration,
    /// The provider's stored trust policy is not usable: `id_token_algs`
    /// not recorded, or a stored field fails validation. Startup's C23
    /// records it from the configuration block.
    PolicyNotRecorded,
    /// The upstream could not be reached at all (including: this host
    /// has no CA certificates to verify TLS with).
    Transport,
    /// The discovery document was invalid, or named no `jwks_uri`.
    Discovery,
    /// The JWKS response was invalid, or has no key that sign-in's own
    /// key selection would accept for an algorithm both sides support.
    Jwks,
    /// The discovery document does not support the policy's token-auth
    /// method.
    AuthMethodUnsupported,
    /// The discovery document's signing algorithms share nothing with
    /// the policy's `id_token_algs`.
    NoCommonAlgorithm,
    /// The provider's version, generation, enabled state, or policy
    /// changed between the capture before network I/O and the re-read
    /// immediately before this function returns.
    ChangedDuringPreflight,
}

/// Each message says what went wrong **and what to do next** (the C17
/// decision, §3 item 6, approved 2026-11-11): an administrator reads this,
/// and a reason with no next step leaves them guessing. Detail an
/// administrator can act on goes to the server log, never here.
impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::NotFound => "this provider no longer exists; reload the provider list",
            Self::Storage => {
                "the provider could not be read from the database; try again, and see the \
                 server log if it persists"
            }
            Self::NotDisabled => {
                "the provider is already enabled; disable it first to check it again"
            }
            Self::NotInConfiguration => {
                "no [[federation_providers]] block names this provider; add one and restart \
                 sui-id"
            }
            Self::PolicyNotRecorded => {
                "the provider's trust policy has not been recorded from its configuration \
                 block yet; restart sui-id, and see the server log if it persists"
            }
            Self::Transport => {
                "the upstream identity provider could not be reached; check this host's \
                 network, DNS and CA certificates, and see the server log"
            }
            Self::Discovery => {
                "the upstream's discovery document was refused; see the server log for the \
                 reason"
            }
            Self::Jwks => {
                "the upstream's signing keys were refused, or none of them is usable; see the \
                 server log for the reason"
            }
            Self::AuthMethodUnsupported => {
                "the upstream does not accept this provider's token-auth method \
                 (client_secret_basic with a client secret, none without one); check \
                 client_secret_env against the upstream's client registration"
            }
            Self::NoCommonAlgorithm => {
                "the upstream signs ID tokens with none of this provider's id_token_algs; add \
                 one it supports (see the server log) and restart sui-id"
            }
            Self::ChangedDuringPreflight => {
                "the provider changed while the check was running; run the check again"
            }
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for PreflightError {}

/// RFC 094 `:506-512`: "Production preflight returns a private
/// non-cloneable/non-debuggable/non-serializable `ValidatedProviderPreflight`
/// only after complete RFC 096 transport, discovery, and JWKS validation.
/// It binds provider ID/version, exact disabled-state activation
/// generation, complete durable-policy digest, trusted observation time,
/// and bounded metadata/key-set fingerprints."
pub struct ValidatedProviderPreflight {
    provider_id: FederationProviderId,
    config_version: i64,
    activation_generation: i64,
    policy_digest: PolicyDigest,
    observed_at: DateTime<Utc>,
    metadata_fingerprint: [u8; 32],
    jwks_fingerprint: [u8; 32],
}

impl ValidatedProviderPreflight {
    pub fn provider_id(&self) -> FederationProviderId {
        self.provider_id
    }
    pub fn config_version(&self) -> i64 {
        self.config_version
    }
    pub fn activation_generation(&self) -> i64 {
        self.activation_generation
    }
    pub fn policy_digest(&self) -> PolicyDigest {
        self.policy_digest
    }
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
    pub fn metadata_fingerprint(&self) -> &[u8; 32] {
        &self.metadata_fingerprint
    }
    pub fn jwks_fingerprint(&self) -> &[u8; 32] {
        &self.jwks_fingerprint
    }
}

/// The only way to produce a [`ValidatedProviderPreflight`]. RFC 096
/// `:409-421`, built clause by clause
/// (`096-b2-stage3-the-preflight-capability-2026-11-11.md` §B):
///
/// 1. Reads the stored row; refuses before any network I/O if it is
///    already enabled, has no usable stored policy, or has no matching
///    `[[federation_providers]]` block.
/// 2. Captures `config_version`/`activation_generation`/the policy
///    digest before any network I/O.
/// 3. Fetches discovery and JWKS through the exact production path the
///    callback uses (`discovery::fetch_discovery_bytes`,
///    `jwks::fetch_jwks_bytes`, the egress client from
///    `app.federation_http_client()`) — no second validation path.
/// 4. Checks compatibility: the discovery document must support the
///    policy's token-auth method, and share at least one signing
///    algorithm with it.
/// 5. Re-reads the row; returns no capability unless version,
///    generation, enabled state, and the policy digest are all exactly
///    unchanged from step 2.
/// 6. Only then builds the capability, from the re-read digest and the
///    fingerprints of the exact bytes that passed validation.
pub async fn preflight_provider(
    app: &AppState,
    provider_id: FederationProviderId,
) -> Result<ValidatedProviderPreflight, PreflightError> {
    let row = sui_id_store::repos::federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => PreflightError::NotFound,
            other => {
                tracing::error!(provider_id = %provider_id, error = %other,
                    "preflight: could not read the provider");
                PreflightError::Storage
            }
        })?;

    if row.enabled {
        return Err(PreflightError::NotDisabled);
    }
    // The configuration block first: without one, its fix (add a block) is
    // the operator's next step, whatever state the stored policy is in.
    if !app
        .config
        .federation_providers
        .iter()
        .any(|c| c.slug == row.slug)
    {
        return Err(PreflightError::NotInConfiguration);
    }
    let policy = match sui_id_store::federation_policy::from_stored(&row) {
        Ok(policy) => policy,
        Err(e) => {
            tracing::warn!(provider_id = %provider_id, field = e.field(),
                "preflight: the stored trust policy is not usable");
            return Err(PreflightError::PolicyNotRecorded);
        }
    };

    // Captured before any network I/O (clause 2).
    let config_version = row.config_version;
    let activation_generation = row.activation_generation;
    let initial_digest = sui_id_store::federation_policy::compute_policy_digest(
        &policy,
        row.client_secret_enc.as_deref(),
    );

    let Some(client) = app.federation_http_client() else {
        tracing::warn!(provider_id = %provider_id, "preflight: no CA certificates on this host");
        return Err(PreflightError::Transport);
    };

    let (discovery_bytes, discovery) = discovery::fetch_discovery_bytes(
        client,
        policy.issuer(),
        &policy.allowed_origins_canonical(),
    )
    .await
    .map_err(|e| {
        tracing::warn!(provider_id = %provider_id, error = %e, "preflight: discovery failed");
        match e {
            discovery::FetchDiscoveryError::Network(_) => PreflightError::Transport,
            discovery::FetchDiscoveryError::Invalid(_) => PreflightError::Discovery,
        }
    })?;

    if !discovery.supports_token_endpoint_auth_method(policy.token_auth_method().as_str()) {
        tracing::warn!(provider_id = %provider_id,
            method = policy.token_auth_method().as_str(),
            "preflight: the upstream's discovery does not list this token-auth method");
        return Err(PreflightError::AuthMethodUnsupported);
    }
    let common_algs = discovery.id_token_algs_intersection(policy.id_token_algs());
    if common_algs.is_empty() {
        tracing::warn!(provider_id = %provider_id,
            configured = %policy.id_token_algs().join(","),
            "preflight: the upstream signs with none of the configured id_token_algs");
        return Err(PreflightError::NoCommonAlgorithm);
    }
    let Some(jwks_uri) = discovery.jwks_uri() else {
        tracing::warn!(provider_id = %provider_id, "preflight: discovery names no jwks_uri");
        return Err(PreflightError::Discovery);
    };

    let (jwks_bytes, jwks_doc) = jwks::fetch_jwks_bytes(client, jwks_uri)
        .await
        .map_err(|e| {
            tracing::warn!(provider_id = %provider_id, error = %e, "preflight: JWKS failed");
            PreflightError::Jwks
        })?;
    // Complete JWKS validation (RFC 094 `:508-509`), not only its structure:
    // at least one key must pass sign-in's own key selection
    // (`jwks::select_key`) for an algorithm both sides support. Otherwise a
    // provider whose every key is unusable (a short RSA modulus, a private
    // member, the wrong curve) would pass the check, be enabled, and fail
    // every sign-in.
    let usable = jwks_doc
        .keys
        .iter()
        .filter_map(|k| k.get("kid").and_then(serde_json::Value::as_str))
        .any(|kid| {
            common_algs
                .iter()
                .any(|alg| jwks::select_key(&jwks_doc, kid, alg).is_ok())
        });
    if !usable {
        tracing::warn!(provider_id = %provider_id,
            "preflight: no JWKS key is usable for an algorithm both sides support");
        return Err(PreflightError::Jwks);
    }

    // Clause 6: the fingerprints are SHA-256 over the exact bytes that
    // passed validation, nothing re-serialized.
    let metadata_fingerprint: [u8; 32] = Sha256::digest(&discovery_bytes).into();
    let jwks_fingerprint: [u8; 32] = Sha256::digest(&jwks_bytes).into();

    // The store's own clock -- the same source C17's `guard_at` will
    // read (`096-b2-c17-consumes-the-capability-decision-2026-11-11.md`
    // §4 item 5): with two clock sources, the 600-second window would
    // measure clock skew, not the check's age. Read once, after
    // validation and before the re-read.
    let observed_at = chrono::Utc::now();

    // Clause 5: the re-read. Nothing outside this block may construct
    // the capability.
    let fresh = sui_id_store::repos::federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| match e {
            // Deleted while the check ran: a change, and the fix is the same.
            sui_id_store::StoreError::NotFound => PreflightError::ChangedDuringPreflight,
            other => {
                tracing::error!(provider_id = %provider_id, error = %other,
                    "preflight: could not re-read the provider");
                PreflightError::Storage
            }
        })?;
    if fresh.enabled
        || fresh.config_version != config_version
        || fresh.activation_generation != activation_generation
    {
        return Err(PreflightError::ChangedDuringPreflight);
    }
    let Ok(fresh_policy) = sui_id_store::federation_policy::from_stored(&fresh) else {
        return Err(PreflightError::ChangedDuringPreflight);
    };
    let fresh_digest = sui_id_store::federation_policy::compute_policy_digest(
        &fresh_policy,
        fresh.client_secret_enc.as_deref(),
    );
    if !initial_digest.ct_eq(&fresh_digest) {
        return Err(PreflightError::ChangedDuringPreflight);
    }

    Ok(ValidatedProviderPreflight {
        provider_id,
        config_version,
        activation_generation,
        policy_digest: fresh_digest,
        observed_at,
        metadata_fingerprint,
        jwks_fingerprint,
    })
}

/// RFC 096-B2 S4 §D: every way [`check_and_enable`] can refuse — S3's
/// [`PreflightError`] variants (the check itself refused), plus C17
/// enable's own (the provider changed, or the check went stale, between
/// the check finishing and the commit). Flat and closed, like
/// `PreflightError` itself: no fingerprint, digest, URL, or upstream
/// body in any variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckAndEnableError {
    /// See [`PreflightError::NotFound`].
    NotFound,
    /// See [`PreflightError::Storage`].
    Storage,
    /// See [`PreflightError::NotDisabled`].
    NotDisabled,
    /// See [`PreflightError::NotInConfiguration`].
    NotInConfiguration,
    /// See [`PreflightError::PolicyNotRecorded`].
    PolicyNotRecorded,
    /// See [`PreflightError::Transport`].
    Transport,
    /// See [`PreflightError::Discovery`].
    Discovery,
    /// See [`PreflightError::Jwks`].
    Jwks,
    /// See [`PreflightError::AuthMethodUnsupported`].
    AuthMethodUnsupported,
    /// See [`PreflightError::NoCommonAlgorithm`].
    NoCommonAlgorithm,
    /// See [`PreflightError::ChangedDuringPreflight`].
    ChangedDuringPreflight,
    /// The provider's version, generation, enabled state, or trust
    /// policy digest changed between the check finishing and the
    /// enable command's own commit.
    ChangedSinceCheck,
    /// `guard_at - observed_at` was negative or `>= 600` seconds by the
    /// time the enable command's transaction ran.
    CheckTooOld,
    /// The provider was enabled by something else between the check
    /// finishing and the commit.
    AlreadyEnabled,
    /// The administrator's step-up lapsed while the check ran: it does
    /// network I/O between the admin surface's gate and the commit. Not a
    /// storage failure, and its fix is different: confirm identity again.
    StepUpRequired,
}

/// Same rule as [`PreflightError::fmt`]: the reason **and** the next
/// step, nothing an administrator cannot act on.
impl std::fmt::Display for CheckAndEnableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "{}", PreflightError::NotFound),
            Self::Storage => write!(f, "{}", PreflightError::Storage),
            Self::NotDisabled => write!(f, "{}", PreflightError::NotDisabled),
            Self::NotInConfiguration => write!(f, "{}", PreflightError::NotInConfiguration),
            Self::PolicyNotRecorded => write!(f, "{}", PreflightError::PolicyNotRecorded),
            Self::Transport => write!(f, "{}", PreflightError::Transport),
            Self::Discovery => write!(f, "{}", PreflightError::Discovery),
            Self::Jwks => write!(f, "{}", PreflightError::Jwks),
            Self::AuthMethodUnsupported => {
                write!(f, "{}", PreflightError::AuthMethodUnsupported)
            }
            Self::NoCommonAlgorithm => write!(f, "{}", PreflightError::NoCommonAlgorithm),
            Self::ChangedDuringPreflight => {
                write!(f, "{}", PreflightError::ChangedDuringPreflight)
            }
            // Same operator-facing meaning as `NotDisabled` (a race landed
            // on it at the very end instead of at the start): same text.
            Self::AlreadyEnabled => write!(f, "{}", PreflightError::NotDisabled),
            Self::ChangedSinceCheck => {
                write!(f, "the provider changed since the check; run it again")
            }
            Self::CheckTooOld => {
                write!(f, "the check is more than 10 minutes old; run it again")
            }
            Self::StepUpRequired => write!(
                f,
                "your identity confirmation expired while the check ran; confirm it again, \
                 then run the check again"
            ),
        }
    }
}

impl std::error::Error for CheckAndEnableError {}

impl From<PreflightError> for CheckAndEnableError {
    fn from(e: PreflightError) -> Self {
        match e {
            PreflightError::NotFound => Self::NotFound,
            PreflightError::Storage => Self::Storage,
            PreflightError::NotDisabled => Self::NotDisabled,
            PreflightError::NotInConfiguration => Self::NotInConfiguration,
            PreflightError::PolicyNotRecorded => Self::PolicyNotRecorded,
            PreflightError::Transport => Self::Transport,
            PreflightError::Discovery => Self::Discovery,
            PreflightError::Jwks => Self::Jwks,
            PreflightError::AuthMethodUnsupported => Self::AuthMethodUnsupported,
            PreflightError::NoCommonAlgorithm => Self::NoCommonAlgorithm,
            PreflightError::ChangedDuringPreflight => Self::ChangedDuringPreflight,
        }
    }
}

impl From<sui_id_store::commands::EnableFromPreflightError> for CheckAndEnableError {
    fn from(e: sui_id_store::commands::EnableFromPreflightError) -> Self {
        use sui_id_store::commands::EnableFromPreflightError as E;
        match e {
            E::ChangedSinceCheck => Self::ChangedSinceCheck,
            E::CheckTooOld => Self::CheckTooOld,
            E::AlreadyEnabled => Self::AlreadyEnabled,
            E::NotFound => Self::NotFound,
            // Its own refusal, not `Storage`: the check's network I/O makes
            // a lapse between S5's gate and this commit a real case, and
            // "try again, see the log" would send the administrator the
            // wrong way. S5 renders it as the step-up redirect.
            E::StepUpRequired => Self::StepUpRequired,
            E::Storage => Self::Storage,
        }
    }
}

/// RFC 096-B2 S4 §A row 1, the C17-seam amendment's *"consumes that
/// capability"*: the **only** function that runs [`preflight_provider`]
/// and moves its [`ValidatedProviderPreflight`] into
/// [`sui_id_store::commands::enable_federation_provider_from_preflight`].
/// `admin` is an [`sui_id_core::actor::AdminActor`] — proof of write
/// privilege (RFC 081) — not a raw id, so there is no way to call this
/// with an actor that has not passed the role check.
pub async fn check_and_enable(
    app: &AppState,
    admin: &sui_id_core::actor::AdminActor,
    provider_id: FederationProviderId,
) -> Result<(), CheckAndEnableError> {
    let evidence = preflight_provider(app, provider_id).await?;
    let now = chrono::Utc::now();
    sui_id_store::commands::enable_federation_provider_from_preflight(
        &app.db,
        admin.user_id(),
        admin.session_id(),
        sui_id_store::commands::PreflightEvidence {
            provider_id: evidence.provider_id(),
            config_version: sui_id_store::commands::ConfigVersion(evidence.config_version()),
            activation_generation: sui_id_store::commands::ActivationGeneration(
                evidence.activation_generation(),
            ),
            policy_digest: evidence.policy_digest(),
            observed_at: evidence.observed_at(),
            metadata_fingerprint: sui_id_store::commands::MetadataFingerprint(
                *evidence.metadata_fingerprint(),
            ),
            jwks_fingerprint: sui_id_store::commands::JwksFingerprint(*evidence.jwks_fingerprint()),
        },
        now,
    )
    .await
    .map(|audited| audited.into_inner())
    .map_err(CheckAndEnableError::from)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "federation_preflight/tests.rs"]
mod tests;
