// RFC 096 :687-689 (stage 6c): `IdentityCapability`'s fields are all
// private and there is no public constructor; the only way to hold one is
// `identity_capability::construct_identity_capability`'s return value.
// This fixture tries the direct route from outside the crate, the same
// proof shape as stage 6a's
// `required_identity_claims_cannot_be_constructed_directly.rs`.

use chrono::Utc;
use sui_id::cache_freshness::{ActivationGeneration, CacheKey, ProviderVersion};
use sui_id::identity_capability::IdentityCapability;
use sui_id_shared::ids::FederationProviderId;

fn attempt() -> IdentityCapability {
    IdentityCapability {
        provider: CacheKey::new(
            FederationProviderId::new(),
            ProviderVersion(1),
            ActivationGeneration(1),
        ),
        sub: "not-validated".to_string(),
        verified_email: None,
        preferred_username: None,
        name: None,
        validated_at: Utc::now(),
    }
}

fn main() {}
