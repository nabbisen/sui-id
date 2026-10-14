// RFC 096-B1 stage 10a fix round, RFC 096 :757-758: the third half of
// the same absence-of-derives proof as the Debug and Clone fixtures
// beside this one. `ValidTotp` carries no `Serialize` impl, so
// `serde_json::to_string` fails to resolve regardless of whether
// `attempt` is ever called.

use sui_id_core::federated_mfa::ValidTotp;

fn attempt(v: ValidTotp) -> String {
    serde_json::to_string(&v).unwrap()
}

fn main() {}
