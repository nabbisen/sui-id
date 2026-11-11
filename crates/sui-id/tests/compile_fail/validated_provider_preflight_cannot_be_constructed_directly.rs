// RFC 096 :409-413 / RFC 094 :506-512 (stage 3): `ValidatedProviderPreflight`'s
// fields are all private and there is no public constructor; the only way
// to hold one is `federation_preflight::preflight_provider`'s return
// value. This fixture tries the direct route from outside the crate, the
// same proof shape as `identity_capability_cannot_be_constructed_directly.
// rs`.

use chrono::Utc;
use sui_id::federation_preflight::ValidatedProviderPreflight;
use sui_id_shared::ids::FederationProviderId;

fn attempt() -> ValidatedProviderPreflight {
    ValidatedProviderPreflight {
        provider_id: FederationProviderId::new(),
        config_version: 0,
        activation_generation: 0,
        observed_at: Utc::now(),
        metadata_fingerprint: [0u8; 32],
        jwks_fingerprint: [0u8; 32],
        policy_digest: todo!(),
    }
}

fn main() {}
