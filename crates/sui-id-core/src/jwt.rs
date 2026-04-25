//! Minimal RFC 7519 JWT support, restricted to the EdDSA (Ed25519) algorithm.
//!
//! We implement the encode/decode path ourselves rather than pulling in a
//! general-purpose JWT crate. The surface we need is small (one algorithm,
//! sign and verify), and writing it here keeps the dependency graph honest:
//! only the upstream `ed25519-dalek` and base64 crates are involved.
//!
//! ## Format
//!
//! `header.payload.signature` where each segment is base64url-no-pad. The
//! header always uses `{"alg":"EdDSA","typ":"JWT","kid":"<kid>"}`; we accept
//! and ignore additional fields on decode but emit only the canonical form.

use crate::errors::{CoreError, CoreResult};
use base64ct::{Base64UrlUnpadded, Encoding};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Standard JWT header for our EdDSA tokens.
#[derive(Debug, Serialize, Deserialize)]
struct Header<'a> {
    alg: &'a str,
    typ: &'a str,
    kid: &'a str,
}

fn b64u(b: &[u8]) -> String {
    let mut out = vec![0u8; (b.len() * 4 / 3) + 4];
    let n = Base64UrlUnpadded::encode(b, &mut out)
        .map(str::len)
        .unwrap_or(0);
    out.truncate(n);
    String::from_utf8(out).expect("base64url is ascii")
}

fn b64u_decode(s: &str) -> Result<Vec<u8>, CoreError> {
    // Output length is at most input length.
    let mut out = vec![0u8; s.len()];
    let n = Base64UrlUnpadded::decode(s, &mut out)
        .map_err(|_| CoreError::Jwt)?
        .len();
    out.truncate(n);
    Ok(out)
}

/// Sign `claims` with `signing_key` and produce a compact JWS.
pub fn sign<C: Serialize>(kid: &str, signing_key: &SigningKey, claims: &C) -> CoreResult<String> {
    let header = Header {
        alg: "EdDSA",
        typ: "JWT",
        kid,
    };
    let header_json = serde_json::to_vec(&header).map_err(|_| CoreError::Jwt)?;
    let payload_json = serde_json::to_vec(claims).map_err(|_| CoreError::Jwt)?;
    let mut signing_input = String::with_capacity(header_json.len() + payload_json.len() + 1);
    signing_input.push_str(&b64u(&header_json));
    signing_input.push('.');
    signing_input.push_str(&b64u(&payload_json));

    let sig = signing_key.sign(signing_input.as_bytes());
    let mut out = signing_input;
    out.push('.');
    out.push_str(&b64u(&sig.to_bytes()));
    Ok(out)
}

/// Result of decoding a token: the verified claims plus the `kid` header.
#[derive(Debug)]
pub struct Decoded<C> {
    pub claims: C,
    pub kid: String,
}

/// Verify a JWS produced by [`sign`] using the supplied resolver to find the
/// public key for the token's `kid`. The resolver lets the caller key off the
/// JWKS in storage without coupling this module to it.
pub fn verify<C: DeserializeOwned, F>(token: &str, mut resolver: F) -> CoreResult<Decoded<C>>
where
    F: FnMut(&str) -> Option<VerifyingKey>,
{
    let mut parts = token.split('.');
    let h = parts.next().ok_or(CoreError::Jwt)?;
    let p = parts.next().ok_or(CoreError::Jwt)?;
    let s = parts.next().ok_or(CoreError::Jwt)?;
    if parts.next().is_some() {
        return Err(CoreError::Jwt);
    }

    let header_bytes = b64u_decode(h)?;
    #[derive(Deserialize)]
    struct ParsedHeader {
        alg: String,
        kid: String,
    }
    let parsed: ParsedHeader = serde_json::from_slice(&header_bytes).map_err(|_| CoreError::Jwt)?;
    if parsed.alg != "EdDSA" {
        return Err(CoreError::Jwt);
    }

    let vk = resolver(&parsed.kid).ok_or(CoreError::Jwt)?;
    let signing_input = format!("{h}.{p}");
    let sig_bytes = b64u_decode(s)?;
    let sig_arr: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| CoreError::Jwt)?;
    let signature = Signature::from_bytes(&sig_arr);
    vk.verify(signing_input.as_bytes(), &signature)
        .map_err(|_| CoreError::Jwt)?;

    let payload = b64u_decode(p)?;
    let claims: C = serde_json::from_slice(&payload).map_err(|_| CoreError::Jwt)?;
    Ok(Decoded {
        claims,
        kid: parsed.kid,
    })
}

#[cfg(test)]
mod tests;
