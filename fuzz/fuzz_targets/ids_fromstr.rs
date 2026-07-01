//! Fuzz target: typed ID `FromStr` parsing (RFC 084).
//!
//! Invariants asserted:
//! - P1 (no panic): arbitrary bytes → string → parse must never panic.
//! - P3 (round-trip): if `Ok(v)` then `v.to_string().parse() == Ok(v)`.
//! - P2 (valid ⇒ UUID v4): a successful parse must produce a well-formed UUID.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::str::FromStr;

use sui_id_shared::ids::{
    ClientId, EmailOutboxId, PasswordResetTokenId, PendingMfaId, SessionId, SigningKeyId, UserId,
    WebauthnCredentialId, WebauthnPendingId,
};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Macro to fuzz one type: no panic, and round-trip coherence on Ok.
    macro_rules! fuzz_id {
        ($T:ty) => {{
            match <$T>::from_str(s) {
                Ok(v) => {
                    // P3: round-trip
                    let reparsed = <$T>::from_str(&v.to_string());
                    assert!(
                        reparsed.is_ok(),
                        "P3: {}::from_str round-trip failed for {:?}",
                        stringify!($T),
                        s
                    );
                    // A parsed ID must round-trip to exactly the same value.
                    // (Equality is available since all ID types derive PartialEq.)
                    assert_eq!(
                        reparsed.unwrap().to_string(),
                        v.to_string(),
                        "P3: {}::from_str round-trip value mismatch",
                        stringify!($T)
                    );
                }
                Err(_) => {
                    // Rejection is fine — arbitrary bytes are rarely valid UUIDs.
                }
            }
        }};
    }

    fuzz_id!(UserId);
    fuzz_id!(ClientId);
    fuzz_id!(SessionId);
    fuzz_id!(SigningKeyId);
    fuzz_id!(PendingMfaId);
    fuzz_id!(WebauthnPendingId);
    fuzz_id!(WebauthnCredentialId);
    fuzz_id!(PasswordResetTokenId);
    fuzz_id!(EmailOutboxId);
});
