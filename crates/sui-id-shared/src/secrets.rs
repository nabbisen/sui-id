//! Security-critical secret and identifier types (RFC 078).
//!
//! This module introduces typed wrappers that prevent the classical
//! "raw string passed where hash expected" and "plaintext logged in
//! debug output" bug classes.
//!
//! # Types
//!
//! | Type | Purpose |
//! |---|---|
//! | [`RawRefreshToken`] | Plaintext refresh-token value, redacted from `Debug`/`Display`, zeroed on drop |
//! | [`RefreshTokenHash`] | SHA-256 hash of a refresh token; the indexed DB lookup key |
//! | [`RefreshTokenId`] | Opaque row identifier for a `refresh_tokens` DB row |
//! | [`FamilyId`] | Rotation-family identifier carried through token rotations |
//! | [`CodeHash`] | SHA-256 hex digest of an authorization code plaintext |

use base64ct::{Base64UrlUnpadded, Encoding};
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::Zeroizing;

// ---------- RawRefreshToken -----------------------------------------------

/// A plaintext refresh-token value held in process memory.
///
/// `Debug` and `Display` implementations print `[REDACTED]` so the value
/// cannot leak into structured logs or error messages. The inner
/// `Zeroizing<String>` zeroes the allocation on drop.
///
/// # Constructors
///
/// - [`RawRefreshToken::generate`] — generate a new 32-byte CSPRNG token
///   (used at issuance).
/// - [`RawRefreshToken::from_untrusted`] — wrap an externally-supplied
///   string for verification (used at the token endpoint).
pub struct RawRefreshToken(Zeroizing<String>);

impl RawRefreshToken {
    /// Generate a cryptographically random 32-byte refresh token,
    /// URL-safe base64-encoded.
    pub fn generate() -> Self {
        Self(Zeroizing::new(random_base64url(32)))
    }

    /// Wrap an externally-supplied candidate value for verification.
    /// The string is zeroized on drop.
    pub fn from_untrusted(s: String) -> Self {
        Self(Zeroizing::new(s))
    }

    /// Expose the inner plaintext value. Every call site is intentional.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Clone for RawRefreshToken {
    fn clone(&self) -> Self {
        Self(Zeroizing::new((*self.0).clone()))
    }
}

impl fmt::Debug for RawRefreshToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawRefreshToken([REDACTED])")
    }
}

impl fmt::Display for RawRefreshToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

// ---------- RefreshTokenHash -----------------------------------------------

/// SHA-256 hash of a refresh token's plaintext value.
///
/// Stored as the indexed `token_hash` column so DB lookups are
/// O(log n) without ever persisting the plaintext.
///
/// Only constructible via [`RefreshTokenHash::of`].
pub struct RefreshTokenHash(Vec<u8>);

impl RefreshTokenHash {
    /// Compute the SHA-256 hash of the given token.
    pub fn of(token: &RawRefreshToken) -> Self {
        let mut h = Sha256::new();
        h.update(token.expose().as_bytes());
        Self(h.finalize().to_vec())
    }

    /// Raw bytes for use in SQL params (`params![hash.as_bytes(), ...]`).
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for RefreshTokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // length only; never the actual hash bytes
        write!(f, "RefreshTokenHash({} bytes)", self.0.len())
    }
}

// ---------- RefreshTokenId ------------------------------------------------

/// Opaque row-level identifier for a `refresh_tokens` DB row.
///
/// Value is a 16-byte CSPRNG output, URL-safe base64-encoded (~22 chars).
/// The private inner field prevents arbitrary construction outside this
/// module; use [`RefreshTokenId::generate`] or [`RefreshTokenId::from_stored`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTokenId(String);

impl RefreshTokenId {
    /// Generate a new random row identifier.
    pub fn generate() -> Self {
        Self(random_base64url(16))
    }

    /// Reconstruct an identifier previously read from the database.
    /// **Must not** be called with arbitrary untrusted strings — only
    /// with values that were written by this system.
    pub fn from_stored(s: String) -> Self {
        Self(s)
    }

    /// Borrow the underlying string for use in SQL params.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefreshTokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------- FamilyId -------------------------------------------------------

/// Rotation-family identifier.
///
/// The first token issued for an authorization-code exchange has
/// `family_id == id`; every rotation inherits the same family id. When
/// theft detection fires, the entire family is revoked atomically.
///
/// Constructible via [`FamilyId::root_of`] (initial issuance) or
/// [`FamilyId::from_stored`] (reading from DB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyId(String);

impl FamilyId {
    /// Create the root family id for a new token family. The family id
    /// equals the initial token's id — one UUID-like value roots the chain.
    pub fn root_of(id: &RefreshTokenId) -> Self {
        Self(id.as_str().to_owned())
    }

    /// Reconstruct a family id previously read from the database.
    pub fn from_stored(s: String) -> Self {
        Self(s)
    }

    /// Borrow the underlying string for use in SQL params.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FamilyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------- CodeHash -------------------------------------------------------

/// SHA-256 hex digest of an authorization code's plaintext value.
///
/// Stored in `auth_codes.code_hash` so the plaintext code is never written
/// to the database; a DB leak cannot expose outstanding codes for replay.
///
/// Constructible via [`CodeHash::of`] (at issuance and exchange time) or
/// [`CodeHash::from_stored`] (reading from DB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeHash(String);

impl CodeHash {
    /// Compute the SHA-256 hex digest of the given authorization code.
    pub fn of(code: &str) -> Self {
        let digest = Sha256::digest(code.as_bytes());
        Self(format!("{digest:x}"))
    }

    /// Reconstruct a code hash previously read from the database.
    pub fn from_stored(s: String) -> Self {
        Self(s)
    }

    /// Borrow the underlying hex string for use in SQL params.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------- private helpers -----------------------------------------------

/// Generate `byte_len` random bytes and return them as a URL-safe
/// base64url (no-pad) string. Used by the generate() constructors above.
fn random_base64url(byte_len: usize) -> String {
    let mut buf = vec![0u8; byte_len];
    getrandom::fill(&mut buf).expect("system RNG unavailable");
    let mut out = vec![0u8; byte_len * 2 + 4];
    let n = Base64UrlUnpadded::encode(&buf, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).expect("base64url is ascii")
}

// ---------- tests ----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_refresh_token_debug_does_not_contain_value() {
        let t = RawRefreshToken::from_untrusted("super-secret-12345".to_owned());
        let dbg = format!("{t:?}");
        assert!(!dbg.contains("super-secret-12345"), "debug must not expose value");
        assert!(dbg.contains("REDACTED"));
    }

    #[test]
    fn raw_refresh_token_display_does_not_contain_value() {
        let t = RawRefreshToken::from_untrusted("super-secret-12345".to_owned());
        let display = format!("{t}");
        assert!(!display.contains("super-secret-12345"));
    }

    #[test]
    fn raw_refresh_token_expose_returns_value() {
        let t = RawRefreshToken::from_untrusted("my-token".to_owned());
        assert_eq!(t.expose(), "my-token");
    }

    #[test]
    fn refresh_token_hash_of_is_deterministic() {
        let t = RawRefreshToken::from_untrusted("hello".to_owned());
        let h1 = RefreshTokenHash::of(&t);
        let h2 = RefreshTokenHash::of(&t);
        assert_eq!(h1.as_bytes(), h2.as_bytes());
    }

    #[test]
    fn code_hash_of_is_hex_string() {
        let h = CodeHash::of("testcode");
        // SHA-256 produces 32 bytes → 64 hex chars
        assert_eq!(h.as_str().len(), 64);
        assert!(h.as_str().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn family_id_root_of_equals_token_id_string() {
        let id = RefreshTokenId::generate();
        let fam = FamilyId::root_of(&id);
        assert_eq!(id.as_str(), fam.as_str());
    }

    #[test]
    fn refresh_token_id_generate_looks_reasonable() {
        let id = RefreshTokenId::generate();
        // 16 bytes base64url → ~22 chars (no padding)
        assert!(id.as_str().len() >= 20);
        // Only base64url chars
        assert!(id.as_str().chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }
}
