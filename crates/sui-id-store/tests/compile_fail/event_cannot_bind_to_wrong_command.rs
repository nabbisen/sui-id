// RFC 094 §"Class-A transaction seam": "A payload for one command type
// cannot satisfy another command's bound." Listed among the RFC's own
// compile-negative fixtures: "use an event enum for the wrong `C`".
//
// `K01Event` implements `SealedCommandEvent<K01>` (via
// `declare_write_command!`). This asks for it as `U22`'s event, through a
// bound, and must be refused.
//
// Until the 0.90.0 preparation this fixture tried to *add* that impl from
// outside the crate, which Rust's orphan rule (E0117) refuses whatever
// sui-id-store contains. That shape could not fail: with a blanket
// `impl<C> SealedCommandEvent<C> for K01Event` planted in the store, it
// still printed the same E0117. This shape compiles under that mutation,
// so trybuild reports it. (Its output also matches on 1.95 and 1.100;
// the orphan diagnostic's labels do not.)

use sui_id_store::commands::{K01Event, U22};
use sui_id_store::registry::SealedCommandEvent;

fn must_bind_to_u22<E: SealedCommandEvent<U22>>(_: &E) {}

fn attempt(event: K01Event) {
    must_bind_to_u22(&event);
}

fn main() {}
