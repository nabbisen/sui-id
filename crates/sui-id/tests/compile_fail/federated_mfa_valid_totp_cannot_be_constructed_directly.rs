// RFC 096-B1 stage 10a, RFC 096 :757-762: `ValidTotp`'s field is private
// and there is no public constructor; the only way to hold one is
// `federated_mfa::verify`'s own return value. This fixture tries the
// direct route from outside the crate -- the same proof shape as
// `identity_capability_cannot_be_constructed_directly.rs`.

use sui_id_core::federated_mfa::ValidTotp;

fn attempt() -> ValidTotp {
    ValidTotp { step: 0 }
}

fn main() {}
