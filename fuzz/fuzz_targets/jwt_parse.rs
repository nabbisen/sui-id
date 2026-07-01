//! Fuzz target: JWT compact-form parsing (RFC 084).
//!
//! Feeds arbitrary byte sequences into `jwt::verify` with a fixed test key.
//! Because `verify` is sync and pure (the key resolver is a simple closure),
//! this target is cheap and finds panic-inducing parser edge cases.
//!
//! Invariants asserted:
//! - P1 (no panic): any byte sequence must not cause a panic. The function
//!   may return `Err(CoreError::Jwt)` freely.
//! - P2 (accept ⇒ structural): if `verify` returns `Ok`, the decoded
//!   claims JSON must have been valid (covered by the generic return type).

#![no_main]
#![cfg(feature = "core-targets")]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(token_str) = std::str::from_utf8(data) else {
        return;
    };

    // Fixed test key pair — the resolver always returns this verifying key.
    // Using a deterministic key lets us produce valid tokens in the corpus
    // while still testing the full parse + verify path.
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use std::sync::OnceLock;

    static TEST_VK: OnceLock<VerifyingKey> = OnceLock::new();
    let vk = TEST_VK.get_or_init(|| {
        // Deterministic seed for reproducible corpus entries.
        let sk = SigningKey::from_bytes(&[42u8; 32]);
        sk.verifying_key()
    });

    // P1: must not panic regardless of input.
    // The claims type is `serde_json::Value` — accepts any JSON.
    let _ = sui_id_core::jwt::verify::<serde_json::Value, _>(token_str, |_kid| Some(*vk));
});
