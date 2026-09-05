// RFC 094 §"Class-A transaction seam": "A payload for one command type
// cannot satisfy another command's bound." Listed among the RFC's own
// compile-negative fixtures: "use an event enum for the wrong `C`".
//
// `registry.rs`'s `SealedCommandEvent<C>` doc comment already claimed this
// is proven by a compile-fail fixture; until this file, that claim was
// false -- no such fixture existed. `K01Event` already implements
// `SealedCommandEvent<K01>` (via `declare_write_command!`, inside
// sui-id-store). This attempts a *second* impl, wiring the same event
// type to `U22` instead. Both `SealedCommandEvent` and `K01Event` are
// defined in `sui-id-store`, foreign to this fixture crate, so Rust's
// orphan rules (E0117) reject the attempt outright -- an ordinary
// language guarantee, not bespoke sealing, but the one that actually
// backs this specific RFC claim from outside the crate.

use sui_id_store::commands::{K01Event, U22};
use sui_id_store::registry::{AuditAttributes, AuditBuildError, AuditResult, AuditTarget, SealedCommandEvent};

impl SealedCommandEvent<U22> for K01Event {
    fn target(&self) -> Option<AuditTarget> {
        None
    }

    fn result(&self) -> AuditResult {
        AuditResult::Ok
    }

    fn attributes(&self) -> Result<AuditAttributes, AuditBuildError> {
        AuditAttributes::builder().build()
    }
}

fn main() {}
