//! RFC 096-A stage 7: the nonce rule, RFC 096 `:663` verbatim --
//! *"Required string; digest compared in constant time to the attempt's
//! nonce digest."* A new sibling, not folded into `identity_claims.rs` or
//! `optional_claims.rs`: this rule's parameter is the expected digest, a
//! shape none of the other claim-validation modules take, and 096-A's own
//! split (one module per parameter shape -- see `time_claims.rs`'s and
//! `identity_capability.rs`'s doc comments) applies here too.
//!
//! **The expected digest is a parameter, not a lookup.** RFC 096 `:65-66`:
//! 096-A delivers the *nonce validation rule*; the durable attempt state
//! that makes a nonce genuinely one-time, including loading, storing, and
//! marking the expected digest consumed, is 096-B1's, because it is a
//! mutation. This module only compares.
//!
//! **Three things already in the tree, reused rather than rewritten:**
//! - The digest algorithm is SHA-256 -- RFC 096 `:579`'s
//!   `federation_login_attempt` schema names the column `nonce_sha256`
//!   alongside `state_sha256` and `browser_binding_sha256`; no other
//!   digest is in play.
//! - [`sui_id_core::tokens::sha256_hex`] already exists, is public, and
//!   has a known-vector test (`tokens/tests.rs:22`). It is imported here as
//!   `sui_id_core::tokens::sha256_hex`, *not* `sui_id_core::oidc::tokens::
//!   sha256_hex` as the dispatch's own text named it -- `oidc` is a
//!   directory the file physically lives under (`#[path =
//!   "oidc/tokens.rs"]`), not a module segment; `tokens` is declared
//!   directly off the crate root. Confirmed by reading `sui-id-core/src/
//!   lib.rs` and by every existing caller in this crate using the shorter
//!   path, before trusting the dispatch's citation.
//! - The constant-time idiom, `use subtle::ConstantTimeEq;
//!   a.as_bytes().ct_eq(b.as_bytes()).into()`, already has five call
//!   sites, one of them (`handlers/federation.rs`'s `state` comparison)
//!   doing exactly this for a different digest-shaped value. Matched here
//!   verbatim rather than re-derived.
//!
//! A private duplicate of `sha256_hex` already exists at
//! `handlers/dynamic_register.rs:327`. Not fixed here -- the dispatch asks
//! for it to be noted, not touched, so a security-path stage does not grow
//! to also tidy an unrelated handler.
//!
//! Reachable only by tests, same as the rest of 096-A until 096-B1 routes
//! live traffic through it.

use subtle::ConstantTimeEq;
use sui_id_core::tokens::sha256_hex;

use crate::id_token::VerifiedIdTokenClaims;

/// RFC 096 `:663`'s nonce rule, in named rejections. `Missing`/`WrongType`
/// are faults in the token's own `nonce` claim; `ExpectedDigestMalformed`
/// is a fault in the caller's own input, not the token's -- a different
/// fault again, and a reader of the error should be able to tell which one
/// happened, per the dispatch's own instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceError {
    Missing,
    WrongType,
    /// The expected digest is not exactly 64 lowercase-or-any-case hex
    /// characters -- see [`validate_nonce`]'s doc comment for why this is
    /// checked before the comparison runs rather than left to `ct_eq`'s
    /// own length short-circuit.
    ExpectedDigestMalformed,
    Mismatch,
}

impl std::fmt::Display for NonceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "nonce is absent"),
            Self::WrongType => write!(f, "nonce is not a string"),
            Self::ExpectedDigestMalformed => {
                write!(f, "the expected nonce digest is not 64 hex characters")
            }
            Self::Mismatch => write!(f, "nonce digest does not match the attempt's"),
        }
    }
}

impl std::error::Error for NonceError {}

/// A SHA-256 digest, hex-encoded, is always exactly 64 characters -- true
/// of both sides of the comparison when the caller passes a genuine digest,
/// but the token's `nonce` claim is attacker-controlled and the *expected*
/// digest is the caller's own value, so this is checked on the expected
/// side as a precondition rather than assumed.
fn is_64_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Validates the `nonce` claim against `expected_digest` -- the attempt's
/// own stored `nonce_sha256`, supplied by the caller, not loaded here (RFC
/// 096 `:65-66`).
///
/// **Why the expected digest is checked for shape before `ct_eq` ever
/// runs.** `ct_eq` on `&[u8]` is constant-time in *contents* only: it
/// short-circuits on length first, un-hidden (confirmed by reading
/// `subtle` 2.6.1's own source, not assumed from its docs). `sha256_hex`
/// always produces exactly 64 hex characters, so the token side of the
/// comparison can never be any other length -- but `expected_digest` is the
/// caller's value, and nothing stops a caller passing a short or malformed
/// one. Rejecting that up front, before computing the token's own digest
/// at all, means a caller bug fails loudly with its own named error rather
/// than silently taking the length short-circuit's fast "not equal" path
/// -- weaker than it looks, and for the wrong reason. Checked first, before
/// even reading the token's `nonce` claim: a malformed expected digest is a
/// fault in this system's own input, independent of whatever the token
/// claims.
///
/// **Compares digests, not nonces.** `expected_digest` is hashed nowhere
/// here -- it already *is* the digest, by its own name and by `:579`'s
/// schema. Only the token's `nonce` claim is hashed, with `sha256_hex`, and
/// the two digests are compared. Taking a plaintext expected nonce instead
/// would invite a caller to pass the raw value the schema never stores.
pub fn validate_nonce(
    claims: &VerifiedIdTokenClaims,
    expected_digest: &str,
) -> Result<(), NonceError> {
    if !is_64_hex(expected_digest) {
        return Err(NonceError::ExpectedDigestMalformed);
    }
    let value = claims.claim("nonce").ok_or(NonceError::Missing)?;
    let nonce = value.as_str().ok_or(NonceError::WrongType)?;
    let actual_digest = sha256_hex(nonce);
    let matches: bool = actual_digest
        .as_bytes()
        .ct_eq(expected_digest.as_bytes())
        .into();
    if !matches {
        return Err(NonceError::Mismatch);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "nonce_claim/tests.rs"]
mod tests;
