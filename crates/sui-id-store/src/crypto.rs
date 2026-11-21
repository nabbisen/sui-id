//! Application-layer column encryption.
//!
//! Sensitive values are sealed with XChaCha20-Poly1305. Each ciphertext carries
//! its own random 24-byte nonce prepended to the bytes; this avoids any need
//! for nonce-state tracking and makes nonce reuse statistically impossible
//! within reasonable use.
//!
//! The master key is provided externally and must be exactly 32 bytes. It
//! never touches the database.

use crate::errors::{StoreError, StoreResult};
use base64ct::{Base64, Encoding};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use getrandom;
use secrecy::{ExposeSecret, SecretBox};
use zeroize::Zeroize;

/// 32-byte symmetric key used for column encryption.
///
/// Wrapped in `SecretBox` so debug-printing or accidental cloning into logs
/// does not reveal the key material.
pub struct MasterKey(SecretBox<[u8; 32]>);

impl MasterKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(SecretBox::new(Box::new(bytes)))
    }

    /// Decode a base64 (standard alphabet, padded) encoded 32-byte key.
    pub fn from_base64(s: &str) -> StoreResult<Self> {
        let mut buf = [0u8; 64];
        let decoded = Base64::decode(s.trim(), &mut buf).map_err(|_| StoreError::Crypto)?;
        if decoded.len() != 32 {
            return Err(StoreError::InvalidMasterKeyLength(decoded.len()));
        }
        let mut k = [0u8; 32];
        k.copy_from_slice(decoded);
        let out = Self::from_bytes(k);
        k.zeroize();
        Ok(out)
    }

    /// Generate a new random 32-byte key from the OS RNG.
    pub fn generate() -> Self {
        let mut k = [0u8; 32];
        // System RNG failure is unrecoverable; panic is the correct fail-safe.
        #[allow(clippy::expect_used)]
        getrandom::fill(&mut k).expect("system RNG unavailable");
        let out = Self::from_bytes(k);
        k.zeroize();
        out
    }

    /// Encode this key as standard base64 with padding. Used only at setup
    /// time when persisting to a key file the operator has chosen.
    pub fn to_base64(&self) -> String {
        let bytes = self.0.expose_secret();
        let mut out = vec![0u8; 64];
        // Buffer is sized for the input; encode cannot fail here.
        #[allow(clippy::expect_used)]
        let n = Base64::encode(bytes, &mut out)
            .expect("output buffer sized for 32-byte input")
            .len();
        out.truncate(n);
        // out is plain text base64, fine to surface to operator
        // base64 output is ASCII; from_utf8 is infallible here.
        #[allow(clippy::expect_used)]
        String::from_utf8(out).expect("base64 is ascii")
    }

    /// Expose the raw key bytes for HMAC derivation (RFC 004).
    /// Callers must not log or persist the returned bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.expose_secret()
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(self.0.expose_secret().into())
    }
}

const NONCE_LEN: usize = 24;

/// Encrypt a plaintext byte slice. Output layout: `nonce(24) || ciphertext || tag(16)`.
pub fn seal(key: &MasterKey, plaintext: &[u8], aad: &[u8]) -> StoreResult<Vec<u8>> {
    let cipher = key.cipher();
    let mut nonce = [0u8; NONCE_LEN];
    // System RNG failure is unrecoverable; panic is the correct fail-safe.
    #[allow(clippy::expect_used)]
    getrandom::fill(&mut nonce).expect("system RNG unavailable");
    let xnonce = <&XNonce>::try_from(nonce.as_slice()).map_err(|_| StoreError::Crypto)?;
    let ct = cipher
        .encrypt(
            xnonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| StoreError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Inverse of [`seal`]. Verifies authentication tag and returns the plaintext.
pub fn open(key: &MasterKey, sealed: &[u8], aad: &[u8]) -> StoreResult<Vec<u8>> {
    if sealed.len() < NONCE_LEN + 16 {
        return Err(StoreError::Crypto);
    }
    let cipher = key.cipher();
    let (nonce_bytes, ct) = sealed.split_at(NONCE_LEN);
    let xnonce = <&XNonce>::try_from(nonce_bytes).map_err(|_| StoreError::Crypto)?;
    cipher
        .decrypt(xnonce, Payload { msg: ct, aad })
        .map_err(|_| StoreError::Crypto)
}

/// RFC 100 §2: a non-secret identifier for a master key, safe to log, to
/// print in a refusal, or to store in the clear — `HMAC-SHA256(key,
/// "sui-id/master-key-fingerprint/v1")`, its first 8 bytes as lowercase
/// hex (16 characters). Keyed and domain-separated: it identifies a key
/// without revealing anything usable about it, and two different
/// purposes keyed on the same bytes (were there ever another) could
/// never collide into the same tag family.
const FINGERPRINT_DOMAIN: &[u8] = b"sui-id/master-key-fingerprint/v1";

pub fn fingerprint(key: &MasterKey) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    // A key of any length is accepted by HMAC's own construction; ours is
    // always exactly 32 bytes, so this never fails.
    #[allow(clippy::expect_used)]
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(FINGERPRINT_DOMAIN);
    let tag = mac.finalize().into_bytes();
    hex_encode(&tag[..8])
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        // A two-digit hex write into a String never fails.
        #[allow(clippy::expect_used)]
        write!(out, "{byte:02x}").expect("fmt::Write to String cannot fail");
    }
    out
}

/// Inverse of [`hex_encode`], into a fixed-size array. `None` on anything
/// but exactly `N * 2` **lowercase** hex digits, the one spelling
/// [`hex_encode`] produces — used to decode a stored journal tag, which
/// must be exactly 32 bytes. Not `u8::from_str_radix`: it accepts a
/// leading `+` (`"+f"` is 15), so one tag would have several spellings.
fn hex_decode<const N: usize>(s: &str) -> Option<[u8; N]> {
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    let (pairs, _) = bytes.as_chunks::<2>();
    for (i, [high, low]) in pairs.iter().enumerate() {
        out[i] = (nibble(*high)? << 4) | nibble(*low)?;
    }
    Some(out)
}

/// RFC 100 stage 3 §2: which key a rotation journal's tag authenticates
/// against — the journal is readable by whichever side's key opens the
/// database sentinel, so recovery never has to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    Old,
    New,
}

/// Domain-separated from [`FINGERPRINT_DOMAIN`] and from each other: the
/// journal's two tags can never be confused with a fingerprint, or with
/// each other, even though both are computed over the same body bytes.
const JOURNAL_OLD_DOMAIN: &[u8] = b"sui-id/rotation-journal/v1/old";
const JOURNAL_NEW_DOMAIN: &[u8] = b"sui-id/rotation-journal/v1/new";

fn journal_mac(key: &MasterKey, domain: &[u8], body: &[u8]) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    #[allow(clippy::expect_used)]
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(domain);
    mac.update(&[0u8]);
    mac.update(body);
    mac.finalize().into_bytes().into()
}

/// `HMAC-SHA256(key, "sui-id/rotation-journal/v1/old" || 0x00 || body)`,
/// hex-encoded. Computed under the *old* key when a journal is written or
/// advanced.
pub fn journal_tag_old(key: &MasterKey, body: &[u8]) -> String {
    hex_encode(&journal_mac(key, JOURNAL_OLD_DOMAIN, body))
}

/// Same as [`journal_tag_old`], domain-separated, computed under the
/// *new* key.
pub fn journal_tag_new(key: &MasterKey, body: &[u8]) -> String {
    hex_encode(&journal_mac(key, JOURNAL_NEW_DOMAIN, body))
}

/// RFC 100 stage 3 §2: verify `body` against both stored tags with one
/// candidate key. Reports which role the key holds -- old or new -- if
/// either tag matches, `None` otherwise (a malformed stored tag is also
/// `None`, never an error: an unreadable journal is simply unauthenticated).
/// Constant-time: both comparisons always run, and `subtle::ConstantTimeEq`
/// (already used elsewhere in this crate) does the comparing, never `==`
/// on the raw bytes.
pub fn verify_journal_tags(
    key: &MasterKey,
    body: &[u8],
    tag_old: &str,
    tag_new: &str,
) -> Option<KeyRole> {
    use subtle::ConstantTimeEq;
    let (Some(stored_old), Some(stored_new)) =
        (hex_decode::<32>(tag_old), hex_decode::<32>(tag_new))
    else {
        return None;
    };
    let candidate_old = journal_mac(key, JOURNAL_OLD_DOMAIN, body);
    let candidate_new = journal_mac(key, JOURNAL_NEW_DOMAIN, body);
    let old_matches: bool = candidate_old.ct_eq(&stored_old).into();
    let new_matches: bool = candidate_new.ct_eq(&stored_new).into();
    if old_matches {
        Some(KeyRole::Old)
    } else if new_matches {
        Some(KeyRole::New)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
