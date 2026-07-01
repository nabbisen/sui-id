//! Fuzz target: post-logout redirect URI validation (RFC 084).
//!
//! Post-logout redirect validation follows the same exact-match semantics as
//! `is_redirect_uri_registered` — this target verifies the property holds
//! under arbitrary URI inputs, including injection patterns.
//!
//! Invariants asserted:
//! - P1 (no panic): arbitrary strings must not cause a panic.
//! - P2 (accept ⇒ registered): if a URI is accepted, it must be present in
//!   the registered list — exact membership, no prefix/suffix/wildcard match.

#![no_main]
#![cfg(feature = "core-targets")]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct LogoutInput {
    /// The `post_logout_redirect_uri` parameter from the relying party.
    submitted: String,
    /// The registered post-logout URIs (primary list).
    registered: Vec<String>,
}

fuzz_target!(|input: LogoutInput| {
    use sui_id_core::authorize::is_redirect_uri_registered;

    // P1: must not panic.
    let accepted = is_redirect_uri_registered(&input.registered, &input.submitted);

    // P2: acceptance ⇒ exact membership (same invariant as authorize_params).
    if accepted {
        assert!(
            input.registered.iter().any(|r| r == &input.submitted),
            "P2: post-logout URI accepted but not in registered list\n\
             submitted: {:?}\n\
             registered: {:?}",
            input.submitted,
            input.registered
        );
    }
});
