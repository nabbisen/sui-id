// RFC 094 :508 (stage 3): "non-debuggable" is the absence of a `Debug`
// derive, not a runtime check. `{v:?}` fails to resolve regardless of
// whether `attempt` is ever called.

use sui_id::federation_preflight::ValidatedProviderPreflight;

fn attempt(v: ValidatedProviderPreflight) {
    println!("{v:?}");
}

fn main() {}
