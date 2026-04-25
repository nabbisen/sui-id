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
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
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
        rand::rngs::OsRng.fill_bytes(&mut k);
        let out = Self::from_bytes(k);
        k.zeroize();
        out
    }

    /// Encode this key as standard base64 with padding. Used only at setup
    /// time when persisting to a key file the operator has chosen.
    pub fn to_base64(&self) -> String {
        let bytes = self.0.expose_secret();
        let mut out = vec![0u8; 64];
        let n = Base64::encode(bytes, &mut out)
            .expect("output buffer sized for 32-byte input")
            .len();
        out.truncate(n);
        // out is plain text base64, fine to surface to operator
        String::from_utf8(out).expect("base64 is ascii")
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
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let xnonce = XNonce::from_slice(&nonce);
    let ct = cipher
        .encrypt(xnonce, Payload { msg: plaintext, aad })
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
    let xnonce = XNonce::from_slice(nonce_bytes);
    cipher
        .decrypt(xnonce, Payload { msg: ct, aad })
        .map_err(|_| StoreError::Crypto)
}

#[cfg(test)]
mod tests;
