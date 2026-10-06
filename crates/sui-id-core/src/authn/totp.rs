//! RFC 6238 TOTP, with the RFC 4648 Base32 encoding the otpauth URI
//! requires for the secret.
//!
//! We implement TOTP rather than pulling in an external crate because the
//! algorithm is small (a few dozen lines on top of HMAC-SHA1) and we
//! prefer to keep the audit surface tight. Behaviour follows RFC 6238
//! literally:
//!
//! - 30-second time step (`T_X = 30`).
//! - HMAC-SHA1 (the default algorithm assumed by every authenticator
//!   app; SHA-256 is permitted by the RFC but not universally
//!   supported in the wild).
//! - 6-digit code (`Digit = 6`).
//! - The dynamic-truncation step (RFC 4226 §5.3).
//!
//! The generator's window is `now ± 1 step`, allowing at most a 30-second
//! drift either way. Replays within a step are blocked at the storage
//! layer via `last_used_step`.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

const STEP_SECS: i64 = 30;
const DIGITS: u32 = 6;

/// Compute the 6-digit TOTP for the given secret and time step.
///
/// `step = floor(unix_time / 30)`.
#[allow(clippy::expect_used)]
pub async fn code_for_step(secret: &[u8], step: i64) -> u32 {
    let counter = (step as u64).to_be_bytes();
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&counter);
    let hash = mac.finalize().into_bytes();
    // Dynamic truncation (RFC 4226 §5.3).
    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let bin = ((hash[offset] & 0x7f) as u32) << 24
        | (hash[offset + 1] as u32) << 16
        | (hash[offset + 2] as u32) << 8
        | (hash[offset + 3] as u32);
    bin % 10u32.pow(DIGITS)
}

/// Verify a user-supplied 6-digit code against `secret` at `unix_time`,
/// allowing a ±1 step drift. Comparison is constant-time per step.
///
/// On success returns the matching `step` so the caller can persist it
/// as `last_used_step` (replay defence). On failure returns `None`.
pub async fn verify(
    secret: &[u8],
    unix_time: i64,
    supplied: u32,
    last_used_step: i64,
) -> Option<i64> {
    if supplied >= 10u32.pow(DIGITS) {
        return None;
    }
    let now_step = unix_time.div_euclid(STEP_SECS);
    for delta in [-1i64, 0, 1] {
        let step = now_step + delta;
        if step <= last_used_step {
            // Replay: this code (or an earlier one) has already been used.
            continue;
        }
        let expected = code_for_step(secret, step).await;
        // Constant-time compare. `u32` → 4 bytes BE.
        if expected.to_be_bytes().ct_eq(&supplied.to_be_bytes()).into() {
            return Some(step);
        }
    }
    None
}

/// Encode bytes as RFC 4648 Base32 (upper-case, no padding). The
/// otpauth:// URI format requires this exact encoding.
pub async fn base32_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buf >> bits) & 0x1f) as usize;
            out.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buf << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[idx] as char);
    }
    out
}

/// Build the `otpauth://totp/...` URI an authenticator app expects.
///
/// `issuer` and `account` should be percent-encoded already; the caller
/// (the HTTP layer) usually has access to a percent-encoder. This helper
/// keeps no opinions about encoding so that callers can reuse whichever
/// implementation they already have.
pub async fn otpauth_uri(issuer: &str, account: &str, secret_bytes: &[u8]) -> String {
    let secret_b32 = base32_encode(secret_bytes).await;
    format!(
        "otpauth://totp/{issuer}:{account}?secret={secret_b32}&issuer={issuer}&algorithm=SHA1&digits=6&period=30"
    )
}

#[cfg(test)]
#[path = "totp/tests.rs"]
mod tests;
