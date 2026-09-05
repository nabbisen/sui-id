// RFC 094 §"Class-A transaction seam" test plan: "provide an arbitrary
// event kind" is listed among the compile-negative fixtures.
//
// `AuditEventKind` is a closed enum — every variant is declared in
// `registry.rs`, one per real event name. This is ordinary closed-enum
// semantics, not bespoke sealing (unlike `CommandSpec`/`SealedCommandEvent`,
// `AuditEventKind` and `EventDescriptor` aren't sealed traits, and
// `EventDescriptor`'s fields are all `pub`; nothing stops constructing one
// from outside this crate). What makes an "arbitrary" kind impossible is
// simply that you cannot name a variant that was never declared.

use sui_id_store::registry::AuditEventKind;

fn attempt() -> AuditEventKind {
    AuditEventKind::ThisVariantWasNeverDeclared
}

fn main() {}
