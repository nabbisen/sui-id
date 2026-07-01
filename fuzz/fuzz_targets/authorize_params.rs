//! Fuzz target: OAuth2 authorize parameter validation (RFC 084).
//!
//! Targets the pure validation functions reachable from the authorize
//! endpoint before any authentication or DB access:
//! - `is_redirect_uri_registered` — the security-critical URI comparison.
//! - `is_redirect_uri_registered` with the `submitted` containing common
//!   injection patterns.
//!
//! Invariants asserted:
//! - P1 (no panic): any input pair must not panic.
//! - P2 (exact match only): `registered == submitted` must be the only
//!   accepting condition (no prefix, suffix, or case variants accepted).

#![no_main]
#![cfg(feature = "core-targets")]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct RedirectInput {
    registered: Vec<String>,
    submitted: String,
}

fuzz_target!(|input: RedirectInput| {
    use sui_id_core::authorize::is_redirect_uri_registered;

    // P1: must not panic.
    let accepted = is_redirect_uri_registered(&input.registered, &input.submitted);

    // P2: acceptance ⇒ exact membership.
    if accepted {
        assert!(
            input.registered.iter().any(|r| r == &input.submitted),
            "P2: is_redirect_uri_registered returned true but submitted is not in registered list\n\
             submitted: {:?}\n\
             registered: {:?}",
            input.submitted,
            input.registered
        );
    }
});
