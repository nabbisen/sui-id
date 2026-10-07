//! RFC 096-A prerequisite: moved verbatim out of `handlers/federation.rs`
//! (the preparatory split).
//!
//! **This is the point of the exercise.** Today this module only decodes
//! ID token claims without verifying the signature (`federation.rs` trusts
//! the upstream's `token_endpoint` over TLS instead — see
//! [`decode_id_token_claims`]'s own doc comment). RFC 096-A — not
//! dispatched by this split, which only prepares a module for it to land
//! in — replaces this with JOSE signature verification, a JWKS cache with
//! rotation, full claims validation, and one-time nonce consumption. That
//! code belongs here, not in `handlers/federation.rs`.

use serde::Deserialize;
use serde::de::{IgnoredAny, MapAccess, Visitor};

#[derive(Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub nonce: Option<String>,
}

/// Decode JWT claims without verifying signature (we trust the upstream's
/// token_endpoint over TLS; full JWKS validation is a future hardening step).
pub fn decode_id_token_claims(jwt: &str) -> Option<IdTokenClaims> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    let parts: Vec<&str> = jwt.split('.').collect();
    let payload = parts.get(1)?;
    // JWT compact serialization uses unpadded base64url.
    let decoded = Base64UrlUnpadded::decode_vec(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

// Reached only by tests until RFC 096-A stage 4 calls `parse_compact_jws` from
// the verification path. Each item below carries `allow(dead_code)` for that
// reason; stage 4 removes them.

/// RFC 096-A stage 2: the largest decoded header plus payload accepted, in bytes.
#[allow(dead_code)]
pub(crate) const MAX_DECODED_HEADER_AND_PAYLOAD: usize = 16 * 1024;

/// The largest base64url text that can decode to `MAX_DECODED_HEADER_AND_PAYLOAD`
/// bytes. A segment longer than this is refused before any decoding allocates.
#[allow(dead_code)]
const MAX_ENCODED_SEGMENT: usize = MAX_DECODED_HEADER_AND_PAYLOAD.div_ceil(3) * 4;

/// Which of the three compact-JWS segments a rule refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum Segment {
    Header,
    Payload,
    Signature,
}

/// Why a compact JWS was refused. One variant per rule, so an operator can tell
/// which rule fired. Stage 4 adds the signature and claim failures.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum CompactJwsError {
    /// The input starts with `{`: a JSON serialization, not a compact token.
    JsonSerialization,
    /// Not exactly three segments. A compact JWE has five, so it lands here.
    WrongSegmentCount { found: usize },
    /// A segment is empty. A detached payload shows up as an empty middle segment.
    EmptySegment(Segment),
    /// A segment is too long to be a header or payload within the size limit,
    /// or the decoded header and payload together exceed it.
    Oversized,
    /// A segment contains `=` padding or a character outside the base64url alphabet.
    NotBase64Url(Segment),
    /// The header is not valid JSON.
    HeaderNotJson,
    /// The header is JSON, but not an object.
    HeaderNotObject,
    /// A header member name appears more than once. The name is the repeated one.
    DuplicateMember(String),
    /// `alg` is absent.
    AlgMissing,
    /// `alg` is present but not a string.
    AlgNotString,
    /// A member RFC 096 forbids in a protected header. The name is the member.
    ForbiddenMember(&'static str),
    /// `cty` is present.
    CtyPresent,
    /// `typ` is present and is not exactly the string `JWT`.
    TypNotJwt,
    /// `kid` is absent.
    KidMissing,
    /// `kid` is not a string of 1 to 128 visible ASCII bytes.
    KidInvalid,
}

/// A compact JWS that passed every structural rule. Nothing here is verified:
/// no signature, no algorithm allowlist, no claims. Stage 4 does those.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct CompactJws {
    pub(crate) header: ValidatedHeader,
    /// The bytes the signature covers: the undecoded `header.payload` text.
    pub(crate) signing_input: String,
    /// The decoded payload.
    pub(crate) payload: Vec<u8>,
    /// The decoded signature.
    pub(crate) signature: Vec<u8>,
}

/// The protected-header members this stage checks. `alg` is present and a
/// string, but its value is not checked: the allowlist belongs to config and
/// to stage 4.
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct ValidatedHeader {
    pub(crate) alg: String,
    pub(crate) kid: String,
    pub(crate) typ: Option<String>,
}

/// The member names of a JSON object, in order, repeats included. `serde_json`
/// collapses a repeated key before a caller can see it, so this reads the names
/// directly.
#[allow(dead_code)]
struct MemberNames(Vec<String>);

impl<'de> Deserialize<'de> for MemberNames {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NamesVisitor;

        impl<'de> Visitor<'de> for NamesVisitor {
            type Value = MemberNames;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<MemberNames, A::Error> {
                let mut names = Vec::new();
                while let Some(name) = map.next_key::<String>()? {
                    map.next_value::<IgnoredAny>()?;
                    names.push(name);
                }
                Ok(MemberNames(names))
            }
        }

        deserializer.deserialize_map(NamesVisitor)
    }
}

/// Structural checks on a compact JWS. The order is part of the contract:
///
/// 1. A JSON serialization (leading `{`) is refused before anything else.
/// 2. The segment count and emptiness are checked on the text.
/// 3. Each header and payload segment is checked for length against the size
///    limit before any base64 decoding allocates.
/// 4. Base64url decoding: unpadded, base64url alphabet only.
/// 5. The decoded header and payload are checked against the size limit
///    **before any JSON parsing**.
/// 6. The header must be a JSON object with no member name repeated.
/// 7. Header members, in a fixed order: `alg`, forbidden members, `cty`, `typ`, `kid`.
#[allow(dead_code)]
pub(crate) fn parse_compact_jws(token: &str) -> Result<CompactJws, CompactJwsError> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    if token.starts_with('{') {
        return Err(CompactJwsError::JsonSerialization);
    }

    let segments: Vec<&str> = token.split('.').collect();
    let [header_b64, payload_b64, signature_b64] = segments.as_slice() else {
        return Err(CompactJwsError::WrongSegmentCount {
            found: segments.len(),
        });
    };
    if header_b64.is_empty() {
        return Err(CompactJwsError::EmptySegment(Segment::Header));
    }
    if payload_b64.is_empty() {
        return Err(CompactJwsError::EmptySegment(Segment::Payload));
    }
    if signature_b64.is_empty() {
        return Err(CompactJwsError::EmptySegment(Segment::Signature));
    }

    if header_b64.len() > MAX_ENCODED_SEGMENT || payload_b64.len() > MAX_ENCODED_SEGMENT {
        return Err(CompactJwsError::Oversized);
    }

    let decode = |text: &str, segment: Segment| {
        Base64UrlUnpadded::decode_vec(text).map_err(|_| CompactJwsError::NotBase64Url(segment))
    };
    let header_bytes = decode(header_b64, Segment::Header)?;
    let payload = decode(payload_b64, Segment::Payload)?;
    let signature = decode(signature_b64, Segment::Signature)?;

    if header_bytes.len() + payload.len() > MAX_DECODED_HEADER_AND_PAYLOAD {
        return Err(CompactJwsError::Oversized);
    }

    let value: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|_| CompactJwsError::HeaderNotJson)?;
    if !value.is_object() {
        return Err(CompactJwsError::HeaderNotObject);
    }
    let names = serde_json::from_slice::<MemberNames>(&header_bytes)
        .map_err(|_| CompactJwsError::HeaderNotJson)?
        .0;
    for (i, name) in names.iter().enumerate() {
        if names[..i].contains(name) {
            return Err(CompactJwsError::DuplicateMember(name.clone()));
        }
    }

    let alg = match value.get("alg") {
        None => return Err(CompactJwsError::AlgMissing),
        Some(v) => v.as_str().ok_or(CompactJwsError::AlgNotString)?.to_owned(),
    };

    for forbidden in ["jku", "x5u", "jwk", "x5c", "crit", "b64"] {
        if value.get(forbidden).is_some() {
            return Err(CompactJwsError::ForbiddenMember(forbidden));
        }
    }

    if value.get("cty").is_some() {
        return Err(CompactJwsError::CtyPresent);
    }

    let typ = match value.get("typ") {
        None => None,
        Some(v) if v.as_str() == Some("JWT") => Some("JWT".to_owned()),
        Some(_) => return Err(CompactJwsError::TypNotJwt),
    };

    let kid = match value.get("kid") {
        None => return Err(CompactJwsError::KidMissing),
        Some(v) => v.as_str().ok_or(CompactJwsError::KidInvalid)?.to_owned(),
    };
    let visible_ascii = kid.bytes().all(|b| (0x21..=0x7E).contains(&b));
    if !(1..=128).contains(&kid.len()) || !visible_ascii {
        return Err(CompactJwsError::KidInvalid);
    }

    Ok(CompactJws {
        header: ValidatedHeader { alg, kid, typ },
        signing_input: format!("{header_b64}.{payload_b64}"),
        payload,
        signature,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "id_token/tests.rs"]
mod tests;
