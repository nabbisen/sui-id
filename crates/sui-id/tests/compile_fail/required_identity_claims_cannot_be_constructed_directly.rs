// RFC 096 :656-659 (stage 6a): `RequiredIdentityClaims`'s only field is
// private and there is no public constructor; the only way to have one is
// `identity_claims::validate_identity_claims`'s success path. This fixture
// tries the direct route from outside the crate, the same proof shape as
// stage 4a's `verified_id_token_claims_cannot_be_constructed_directly.rs`
// and stage 4c's `retained_cache_entry_cannot_be_constructed_directly.rs`.

use sui_id::identity_claims::RequiredIdentityClaims;

fn attempt() -> RequiredIdentityClaims {
    RequiredIdentityClaims {
        sub: "not-validated".to_string(),
    }
}

fn main() {}
