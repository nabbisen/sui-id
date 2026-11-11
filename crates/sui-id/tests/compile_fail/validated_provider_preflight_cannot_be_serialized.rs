// RFC 094 :508 (stage 3): "non-serializable" is the absence of a
// `Serialize` impl, not a runtime check. `serde_json::to_string` fails to
// resolve regardless of whether `attempt` is ever called.

use sui_id::federation_preflight::ValidatedProviderPreflight;

fn attempt(v: ValidatedProviderPreflight) -> String {
    serde_json::to_string(&v).unwrap()
}

fn main() {}
