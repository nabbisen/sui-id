//! RFC 096-A prerequisite: moved verbatim out of `handlers/federation.rs`
//! (the preparatory split) — username derivation and conflict resolution
//! for a newly provisioned federated user (P7).
//!
//! RFC 096-B1 stage 12a: [`derive_username`] now reads an
//! [`IdentityCapability`] rather than the legacy, unverified claims struct
//! this module used before — the capability is only ever constructed from
//! a signature-verified
//! token whose optional claims already passed `:663-672`'s bounded-length,
//! no-control/bidi-character rule (`optional_claims::validate_preferred_
//! username`), so the ASCII/length filtering below is now a second,
//! narrower restriction on an already-validated string, not the only check
//! standing between this function and a malformed upstream claim. The one
//! caller reaches this function only from `IdentityResolution::
//! ProvisionEligible`, which itself only exists when `capability.
//! verified_email()` is `Some` — so the `email` fallback below reads the
//! *verified* email, exactly as the legacy code's own call site reached it
//! only after its own, separate unverified-email refusal already ran.

use crate::identity_capability::IdentityCapability;

/// Derive a candidate username from the validated upstream identity (P7).
pub fn derive_username(capability: &IdentityCapability) -> String {
    // Priority: preferred_username → verified email local-part → sub (truncated)
    if let Some(pu) = capability.preferred_username() {
        let clean: String = pu
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    if let Some(email) = capability.verified_email()
        && let Some(local) = email.split('@').next()
    {
        let clean: String = local
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(32)
            .collect();
        if !clean.is_empty() {
            return clean;
        }
    }
    // Final fallback: first 16 chars of sub
    capability.sub().chars().take(16).collect()
}

/// Conflict-resolve a proposed username (P7): append numeric suffix until free.
pub async fn resolve_shadow_username(db: &sui_id_store::Database, proposed: &str) -> String {
    if sui_id_store::repos::users::find_by_username(db, proposed)
        .await
        .is_err()
    {
        return proposed.to_owned();
    }
    for n in 2u32..=1000 {
        let candidate = format!("{proposed}{n}");
        if sui_id_store::repos::users::find_by_username(db, &candidate)
            .await
            .is_err()
        {
            return candidate;
        }
    }
    format!("{proposed}-{}", sui_id_shared::ids::UserId::new())
}
