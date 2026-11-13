// RFC 096-B1 stage 10a fix round, RFC 096 :757-758: the third half of
// the same absence-of-derives proof as the Debug and Clone fixtures
// beside this one. `ValidTotp` carries no `Serialize` impl, so
// the bound below fails to resolve regardless of whether
// `attempt` is ever called.

use sui_id_core::federated_mfa::ValidTotp;

// Asked through a bound on this file's own function, not `serde_json`'s:
// rustc 1.100 prints a foreign function's bound note differently from the
// MSRV (1.95), and this snapshot must match on both (0.90.0 preparation
// review).
fn must_serialize<T: serde::Serialize>(_: &T) {}

fn attempt(v: ValidTotp) {
    must_serialize(&v);
}

fn main() {}
