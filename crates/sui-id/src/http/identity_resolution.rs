//! RFC 096-B1 stage 5: consume the capability stage 4 builds and resolve
//! it to a local identity — RFC 096 `:693-711`. **Resolves and decides; does
//! not act.** No session, no link row, no provisioning transaction (stage
//! 6's job, F04's amended Class-A commit) — this module's only output is
//! which of five named states applies, so stage 6 matches on a decision
//! rather than re-deriving the reasoning.
//!
//! `(provider_id, sub)` is the sole lookup key (`:695`) — not email, not
//! `preferred_username`, not anything else. An existing link authenticates
//! without email (`:695-696`): [`IdentityResolution::ExistingLink`] carries
//! only the resolved `user_id`, nothing from the capability's own email or
//! display hints.
//!
//! **`link_only` returns one generic result for every unlinked upstream
//! identity**, regardless of whether its email happens to collide with a
//! local account (`:699`) — collision denial is specific to
//! `provision_on_first_login` (`:707-708`); under `link_only` it must not
//! leak which case applies, so there is no `link_only`-specific collision
//! state here at all, only [`IdentityResolution::LinkRequired`].
//!
//! **The collision check normalizes with the same function the local user
//! path already uses**: [`sui_id_shared::normalize_email`] — not a second,
//! independently-maintained normalization that could quietly disagree with
//! it and miss a collision.

use sui_id_shared::ids::UserId;
use sui_id_store::models::ProvisionMode;
use sui_id_store::{Database, StoreResult};

use crate::identity_capability::IdentityCapability;

/// The five states RFC 096 `:693-711` names. Stage 6 matches on this rather
/// than re-deriving which case applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityResolution {
    /// `(provider_id, sub)` already has a durable link (`:695`) — may
    /// authenticate without email (`:695-696`).
    ExistingLink { user_id: UserId },
    /// No local account is linked, and `provision_mode` is `link_only`.
    /// One generic result for every such identity, collision or not
    /// (`:699`) — see this module's own doc comment for why collision is
    /// not distinguished here.
    LinkRequired,
    /// No local account is linked, `provision_mode` is
    /// `provision_on_first_login`, a verified email is present, and no
    /// local user already holds its normalized form. Eligible for new-
    /// account provisioning — stage 6's transaction, not this function's.
    ProvisionEligible,
    /// `provision_on_first_login`, a verified email is present, but a
    /// *different* local user already holds its normalized form — deny,
    /// never auto-link (`:707-708`).
    DeniedCollision,
    /// `provision_on_first_login`, and the capability's `verified_email`
    /// is absent — covering both "no email claim at all" and "email
    /// present but not `email_verified`" in the one check, because stage 4
    /// already collapsed those into the one `Option` (`:710-711`).
    DeniedUnverified,
}

/// Resolve `capability` to one of [`IdentityResolution`]'s five states.
///
/// Reads only: the federation-link lookup by `(provider_id, sub)`, and,
/// only for `provision_on_first_login` with a verified email, the local
/// user lookup by normalized email. No write, no session, no cookie — a
/// decision value, not a mutation.
pub async fn resolve_verified_identity(
    db: &Database,
    capability: &IdentityCapability,
    provision_mode: ProvisionMode,
) -> StoreResult<IdentityResolution> {
    let provider_id = capability.provider().provider_id();

    if let Some(link) =
        sui_id_store::repos::federation_link::find_by_sub(db, provider_id, capability.sub()).await?
    {
        return Ok(IdentityResolution::ExistingLink {
            user_id: link.user_id,
        });
    }

    match provision_mode {
        ProvisionMode::LinkOnly => Ok(IdentityResolution::LinkRequired),
        ProvisionMode::ProvisionOnFirstLogin => {
            let Some(email) = capability.verified_email() else {
                return Ok(IdentityResolution::DeniedUnverified);
            };
            let normalized = sui_id_shared::normalize_email(email);
            let collision =
                sui_id_store::repos::users::find_by_email_normalized(db, &normalized).await?;
            if collision.is_some() {
                return Ok(IdentityResolution::DeniedCollision);
            }
            Ok(IdentityResolution::ProvisionEligible)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "identity_resolution/tests.rs"]
mod tests;
