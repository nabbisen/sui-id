// RFC 096 :873-896 (stage 4c): `RetainedEntry`'s fields are private and
// reachable only through accessors; the only way to hold one is
// `cache_freshness::store_fresh`'s or `apply_304`'s success path. This
// fixture tries the direct route from outside the crate, the same proof
// shape as stage 4a's
// `verified_id_token_claims_cannot_be_constructed_directly.rs`.

use std::time::Duration;

use chrono::Utc;
use sui_id::cache_freshness::{
    ActivationGeneration, CacheDirectives, CacheKey, ProviderVersion, RetainedEntry,
};
use sui_id_shared::ids::FederationProviderId;

fn attempt() -> RetainedEntry<()> {
    RetainedEntry {
        key: CacheKey::new(
            FederationProviderId::new(),
            ProviderVersion(1),
            ActivationGeneration(1),
        ),
        document: (),
        etag: None,
        directives: CacheDirectives::default(),
        lifetime: None,
        initial_current_age: Duration::ZERO,
        received_at: Utc::now(),
    }
}

fn main() {}
