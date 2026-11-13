// RFC 094 Stage 1 item 3: "Prove secret types cannot be formatted/coerced
// into payload attributes." `AuditAttributesBuilder::attribute` takes
// `impl AttributeValue`, a sealed trait implemented only for `String` and
// `&str`. `secrecy::SecretBox` is not one, so passing a secret is a
// compile error, not a runtime rejection.
//
// Asked through a bound on this file's own function, not the builder
// method: rustc 1.100 prints the note for a foreign method's bound
// differently from the MSRV (1.95), and this snapshot must match on both
// (0.90.0 preparation review). The bound is the same `AttributeValue`
// the builder demands.

use secrecy::SecretBox;
use sui_id_store::registry::AttributeValue;

fn must_be_attribute<T: AttributeValue>(_: T) {}

fn attempt(secret: SecretBox<String>) {
    must_be_attribute(secret);
}

fn main() {}
