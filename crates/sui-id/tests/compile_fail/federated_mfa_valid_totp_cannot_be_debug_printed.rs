// RFC 096-B1 stage 10a, RFC 096 :757-758: "non-cloneable,
// non-debuggable, non-serializable" is the absence of those derives,
// not a runtime check. This fixture never constructs a `ValidTotp` (it
// cannot -- see the sibling fixture) -- it only needs the compiler to
// check this function body's `{:?}` against whatever bound `ValidTotp`
// actually satisfies, which is none, regardless of whether `attempt` is
// ever called with a real value.

use sui_id_core::federated_mfa::ValidTotp;

fn attempt(v: ValidTotp) {
    println!("{v:?}");
}

fn main() {}
