// RFC 094 Stage 1 item 3: "Prove secret types cannot be formatted/coerced
// into payload attributes." `AuditAttributesBuilder::attribute` takes
// `impl Into<String>` -- `secrecy::SecretBox` deliberately implements
// neither `Into<String>` nor `Display`, so passing one is a compile error,
// not a runtime rejection.

use secrecy::SecretBox;
use sui_id_store::registry::AuditAttributes;

fn attempt(secret: SecretBox<String>) {
    let _ = AuditAttributes::builder().attribute("token", secret);
}

fn main() {}
