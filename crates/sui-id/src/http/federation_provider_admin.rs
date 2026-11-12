//! RFC 096-B2 S6: the two `sui-id`-side callers C18 (delete) and C20
//! (unlink) need — the same seam S4's `check_and_enable` established for
//! C17: a store command that needs context only `sui-id` has (here,
//! "does a `[[federation_providers]]` block still name this provider,"
//! for C18) gets a thin wrapper in this crate, and nothing else may call
//! the store command at all — a source-scope test pins that for both
//! (`crates/sui-id/tests/e2e/r096_b2_stage6_c18_c20_scope.rs`), the same
//! way stage 4's own test pins `check_and_enable`.
//!
//! `unlink_user_from_provider` needs no `sui-id`-only context at all —
//! C20 has no configuration-dependent precondition — but the scope test
//! still pins it as C18's own caller, for the same defense-in-depth
//! reason every admin-mutating sealed command gets one: a future second
//! caller skipping the admin surface's step-up gate is a compile error,
//! not a review miss.

use crate::state::AppState;
use sui_id_core::actor::AdminActor;
use sui_id_shared::ids::{FederationProviderId, UserId};

/// RFC 096-B2 S6 §A/§D: every way [`delete_provider`] can refuse. No
/// issuer, client id, slug, or trust-policy value in any variant — same
/// rule as the store's own [`sui_id_store::commands::
/// DeleteFederationProviderError`], which this wraps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteProviderError {
    /// See [`sui_id_store::commands::DeleteFederationProviderError::NotFound`].
    NotFound,
    /// See [`sui_id_store::commands::DeleteFederationProviderError::StillEnabled`].
    StillEnabled,
    /// Precondition 3 (A3 as narrowed 2026-11-12): a
    /// `[[federation_providers]]` block still names this provider's
    /// slug. The store cannot see the configuration, so this is the one
    /// refusal this wrapper adds rather than forwards.
    StillConfigured,
    /// See [`sui_id_store::commands::DeleteFederationProviderError::StepUpRequired`].
    StepUpRequired,
    /// See [`sui_id_store::commands::DeleteFederationProviderError::Storage`].
    Storage,
}

impl std::fmt::Display for DeleteProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::NotFound => "the provider no longer exists; reload the provider list",
            Self::StillEnabled => "this provider is still enabled; disable it first",
            Self::StillConfigured => {
                "a [[federation_providers]] block still names this provider; remove it and \
                 restart sui-id first, or it would be created again on restart"
            }
            Self::StepUpRequired => {
                "your identity confirmation has expired; confirm it again, then retry"
            }
            Self::Storage => {
                "the provider could not be deleted; try again, and see the server log if it \
                 persists"
            }
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for DeleteProviderError {}

impl From<sui_id_store::commands::DeleteFederationProviderError> for DeleteProviderError {
    fn from(e: sui_id_store::commands::DeleteFederationProviderError) -> Self {
        use sui_id_store::commands::DeleteFederationProviderError as E;
        match e {
            E::NotFound => Self::NotFound,
            E::StillEnabled => Self::StillEnabled,
            E::StepUpRequired => Self::StepUpRequired,
            E::Storage => Self::Storage,
        }
    }
}

/// RFC 096-B2 S6 §A, precondition 3: refuses before the store command is
/// ever reached if a `[[federation_providers]]` block still names this
/// provider's slug — startup's own config, immutable for the process's
/// lifetime (no hot-reload exists), so there is no race between this
/// read and the delete's own commit to guard against. Preconditions 1
/// (admin UI, step-up) are the caller's; precondition 2 (not enabled)
/// and the transaction itself are the store command's own.
pub async fn delete_provider(
    app: &AppState,
    admin: &AdminActor,
    provider_id: FederationProviderId,
) -> Result<(), DeleteProviderError> {
    let row = sui_id_store::repos::federation_provider::get(&app.db, provider_id)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => DeleteProviderError::NotFound,
            other => {
                tracing::error!(provider_id = %provider_id, error = %other,
                    "delete_provider: could not read the provider");
                DeleteProviderError::Storage
            }
        })?;
    if app
        .config
        .federation_providers
        .iter()
        .any(|c| c.slug == row.slug)
    {
        return Err(DeleteProviderError::StillConfigured);
    }
    sui_id_store::commands::delete_federation_provider(
        &app.db,
        admin.user_id(),
        admin.session_id(),
        provider_id,
        chrono::Utc::now(),
    )
    .await
    .map(|audited| audited.into_inner())
    .map_err(DeleteProviderError::from)
}

/// RFC 096-B2 S6 §B: the one legitimate caller of
/// [`sui_id_store::commands::unlink_federation_provider`] — see this
/// module's own doc for why a wrapper exists even though C20 needs no
/// extra context.
pub async fn unlink_user_from_provider(
    app: &AppState,
    admin: &AdminActor,
    user_id: UserId,
    provider_id: FederationProviderId,
) -> Result<(), sui_id_store::commands::UnlinkFederationProviderError> {
    sui_id_store::commands::unlink_federation_provider(
        &app.db,
        admin.user_id(),
        admin.session_id(),
        user_id,
        provider_id,
        chrono::Utc::now(),
    )
    .await
    .map(|audited| audited.into_inner())
}
