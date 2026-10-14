// RFC 096-B1 stage 10a, RFC 096 :757-758: the "non-cloneable" half of
// the same absence-of-derives proof as the Debug fixture beside this
// one. `ValidTotp` carries no `Clone` impl, so `.clone()` fails to
// resolve regardless of whether `attempt` is ever called.

use sui_id_core::federated_mfa::ValidTotp;

fn attempt(v: &ValidTotp) -> ValidTotp {
    v.clone()
}

fn main() {}
