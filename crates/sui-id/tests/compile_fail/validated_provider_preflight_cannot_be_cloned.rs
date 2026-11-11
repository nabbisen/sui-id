// RFC 094 :508 (stage 3): the "non-cloneable" half of the same
// absence-of-derives proof as the Debug and construct fixtures beside
// this one. `ValidatedProviderPreflight` carries no `Clone` impl, so
// `.clone()` fails to resolve regardless of whether `attempt` is ever
// called.

use sui_id::federation_preflight::ValidatedProviderPreflight;

fn attempt(v: &ValidatedProviderPreflight) -> ValidatedProviderPreflight {
    v.clone()
}

fn main() {}
