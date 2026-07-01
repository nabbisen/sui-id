//! Fuzz target: PKCE code-challenge verification (RFC 084).
//!
//! Invariants asserted:
//! - P1 (no panic): any (method, verifier, challenge) triple must not panic.
//! - P2 (accept ⇒ correct): if `verify_pkce` returns `Ok`, then
//!   `BASE64URL_NOPAD(SHA-256(verifier)) == challenge` must hold — proven
//!   by recomputing inside the harness and asserting equality.
//! - P2 (reject unknown method): any method that is not `"S256"` must
//!   return `Err`, not `Ok`.

#![no_main]
#![cfg(feature = "core-targets")]

use libfuzzer_sys::fuzz_target;
use sui_id_core::tokens::verify_pkce;

fuzz_target!(|data: &[u8]| {
    // Layout: we split the input into three NUL-separated UTF-8 strings.
    // If fewer than two NULs appear, we use what we have (empty strings
    // for missing segments). This is simpler than structured input for
    // this target.
    let parts: Vec<&[u8]> = data.splitn(3, |&b| b == 0).collect();
    let to_str = |p: &[u8]| std::str::from_utf8(p).unwrap_or("");
    let method    = to_str(parts.first().copied().unwrap_or(b""));
    let verifier  = to_str(parts.get(1).copied().unwrap_or(b""));
    let challenge = to_str(parts.get(2).copied().unwrap_or(b""));

    // P1: no panic.
    let result = verify_pkce(method, verifier, challenge);

    match result {
        Ok(()) => {
            // P2a: only "S256" may succeed.
            assert_eq!(
                method, "S256",
                "P2: verify_pkce returned Ok for non-S256 method {:?}",
                method
            );

            // P2b: recompute expected challenge and compare.
            use base64ct::{Base64UrlUnpadded, Encoding};
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(verifier.as_bytes());
            let expected = Base64UrlUnpadded::encode_string(&digest);
            assert_eq!(
                expected, challenge,
                "P2: verify_pkce returned Ok but SHA-256(verifier) != challenge"
            );
        }
        Err(_) => {
            // Rejection is always safe. Nothing to assert on the Err path
            // (the function returns various protocol errors; all are valid).
        }
    }
});
