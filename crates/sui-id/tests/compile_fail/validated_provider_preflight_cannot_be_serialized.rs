// RFC 094 :508 (stage 3): "non-serializable" is the absence of a
// `Serialize` impl, not a runtime check. The bound below fails to
// resolve regardless of whether `attempt` is ever called.

use sui_id::federation_preflight::ValidatedProviderPreflight;

// Asked through a bound on this file's own function, not `serde_json`'s:
// rustc 1.100 prints a foreign function's bound note differently from the
// MSRV (1.95), and this snapshot must match on both (0.90.0 preparation
// review).
fn must_serialize<T: serde::Serialize>(_: &T) {}

fn attempt(v: ValidatedProviderPreflight) {
    must_serialize(&v);
}

fn main() {}
