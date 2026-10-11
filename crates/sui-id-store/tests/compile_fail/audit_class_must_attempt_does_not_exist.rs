// RFC 096-B1 stage 6b: the replacement for `commands::tests::
// every_descriptor_is_atomic_class` (removed in the same change this
// fixture landed in).
//
// `AuditClass` used to have a second variant, `MustAttempt`, for a
// Class-B mechanism that did not exist yet -- so a sealed
// `EventDescriptor`'s `class` field could, in principle, have been
// written as `AuditClass::MustAttempt` by mistake, and the removed test
// was a runtime check that nobody had. Now that the Class-B mechanism
// (`repos::audit::emit_must_attempt`) exists with its own descriptor
// type (`MustAttemptDescriptor`, no `class` field at all), `AuditClass`
// was narrowed to its one real variant. "Every `EventDescriptor` is
// Atomic" is therefore not a fact that happens to hold today -- there is
// no other value to write. This is ordinary closed-enum semantics, the
// same kind `arbitrary_event_kind_does_not_exist.rs` proves for
// `AuditEventKind`, not bespoke sealing.

use sui_id_store::registry::AuditClass;

fn attempt() -> AuditClass {
    AuditClass::MustAttempt
}

fn main() {}
