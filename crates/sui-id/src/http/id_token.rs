//! RFC 096-A: moved verbatim out of `handlers/federation.rs` (the preparatory
//! split), then built on, stage by stage, beside what was there.
//!
//! **`decode_id_token_claims` is still what production calls, and it still
//! verifies nothing** (`federation.rs` trusts the upstream's `token_endpoint`
//! over TLS instead — see its own doc comment). [`verify_id_token`] and
//! [`verify_id_token_against_jwks`] (stage 4a) join stage 2's structural
//! parse and stage 3b's key selection into real signature verification, and
//! are reached only by tests until 096-B1 routes live traffic through them.
//! The JWKS cache and rotation (stage 4b) are not here yet.

use serde::Deserialize;
use serde::de::{IgnoredAny, MapAccess, Visitor};

use crate::jwks;

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

/// RFC 096-A stage 2: the largest decoded header plus payload accepted, in bytes.
pub(crate) const MAX_DECODED_HEADER_AND_PAYLOAD: usize = 16 * 1024;

/// The largest encoded compact JWS accepted, in bytes. Sized to admit the RFC's
/// 16 KiB decoded header plus payload at their largest encoding (21,848 base64url
/// characters), an RSA-4096 signature (684 characters), and the two dots, with
/// room to spare. It is far larger than `response_bounds::MAX_STRING_LEN` (8 KiB),
/// the bound the live transport puts on this same string today. That bound binds
/// first on the production path; this one exists so the RFC's own decoded limit
/// is reachable by the function itself, and so the function bounds its own input.
pub(crate) const MAX_ENCODED_JWS_LEN: usize = 24 * 1024;

/// The largest base64url text that can decode to `MAX_DECODED_HEADER_AND_PAYLOAD`
/// bytes. A segment longer than this is refused before any decoding allocates.
const MAX_ENCODED_SEGMENT: usize = MAX_DECODED_HEADER_AND_PAYLOAD.div_ceil(3) * 4;

/// Which of the three compact-JWS segments a rule refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    Header,
    Payload,
    Signature,
}

/// Why a compact JWS was refused. One variant per rule, so an operator can tell
/// which rule fired. `VerificationError` carries stage 4a's signature and
/// claim failures; this enum is wrapped inside it, not merged into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactJwsError {
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

impl std::fmt::Display for CompactJwsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonSerialization => {
                write!(f, "input is a JSON serialization, not a compact token")
            }
            Self::WrongSegmentCount { found } => write!(f, "{found} segments, not three"),
            Self::EmptySegment(s) => write!(f, "{s:?} segment is empty"),
            Self::Oversized => write!(f, "token exceeds the size limit"),
            Self::NotBase64Url(s) => write!(f, "{s:?} segment is not base64url"),
            Self::HeaderNotJson => write!(f, "header is not JSON"),
            Self::HeaderNotObject => write!(f, "header is JSON but not an object"),
            Self::DuplicateMember(name) => write!(f, "header repeats member {name:?}"),
            Self::AlgMissing => write!(f, "header has no alg"),
            Self::AlgNotString => write!(f, "header's alg is not a string"),
            Self::ForbiddenMember(name) => write!(f, "header carries forbidden member {name:?}"),
            Self::CtyPresent => write!(f, "header carries cty"),
            Self::TypNotJwt => write!(f, "header's typ is present and is not exactly JWT"),
            Self::KidMissing => write!(f, "header has no kid"),
            Self::KidInvalid => write!(f, "header's kid is not 1-128 visible ASCII bytes"),
        }
    }
}

impl std::error::Error for CompactJwsError {}

/// A compact JWS that passed every structural rule. Nothing here is verified:
/// no signature, no algorithm allowlist, no claims. `verify_id_token_against_jwks`
/// does those.
#[derive(Debug, PartialEq, Eq)]
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
pub(crate) struct ValidatedHeader {
    pub(crate) alg: String,
    pub(crate) kid: String,
    pub(crate) typ: Option<String>,
}

/// The member names of a JSON object, in order, repeats included. `serde_json`
/// collapses a repeated key before a caller can see it, so this reads the names
/// directly.
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
/// 0. The whole input is refused if longer than `MAX_ENCODED_JWS_LEN`.
/// 1. A JSON serialization (leading `{`) is refused before anything else.
/// 2. The segment count and emptiness are checked on the text.
/// 3. Each header and payload segment is checked for length against the size
///    limit before any base64 decoding allocates.
/// 4. Base64url decoding: unpadded, base64url alphabet only.
/// 5. The decoded header and payload are checked against the size limit
///    **before any JSON parsing**.
/// 6. The header must be a JSON object with no member name repeated.
/// 7. Header members, in a fixed order: `alg`, forbidden members, `cty`, `typ`, `kid`.
pub(crate) fn parse_compact_jws(token: &str) -> Result<CompactJws, CompactJwsError> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    // Before anything else: the whole input, on its encoded length, so that no
    // later step runs on an unbounded string.
    if token.len() > MAX_ENCODED_JWS_LEN {
        return Err(CompactJwsError::Oversized);
    }

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

// ---- RFC 096-A stage 4a: signature verification -----------------------------
//
// Joins `parse_compact_jws` (stage 2) and `select_key` (stage 3b), and enforces
// the two refusals those stages deferred. Reachable only by tests:
// `decode_id_token_claims` is unchanged and `handlers/federation.rs` is
// untouched; 096-B1 is what routes live traffic through this.

/// Why a token did not verify. One variant per rule, as in stages 2, 3a and
/// 3b. Where a failure is one of theirs, it is wrapped, not re-described; the
/// new variants are the ones stage 4a adds.
#[derive(Debug)]
pub enum VerificationError {
    /// Stage 2's structural rules, run on the raw token before anything else.
    Structure(CompactJwsError),
    /// The header's `alg` is not in the provider's configured `id_token_algs`.
    /// This is the outer gate: it runs before key selection, not after, so a
    /// disallowed algorithm is refused for that reason, never for a family
    /// mismatch it happens to also have.
    AlgNotPermitted,
    /// The provider's discovery document has no `jwks_uri`. Stage 3a made the
    /// field optional so a document without one would still deserialize on
    /// the live path; this is the stage that needs one to verify against, so
    /// this is where its absence becomes a refusal.
    NoJwksUri,
    /// Fetching or structurally checking the JWKS (stage 3a).
    Jwks(jwks::JwksError),
    /// Selecting a key from the JWKS (stage 3b), including the key material
    /// being rejected at the point it is converted.
    KeySelection(jwks::KeySelectionError),
    /// The signature itself did not verify, the token's claims failed a
    /// temporal check (`exp`), or `jsonwebtoken` rejected the token for a
    /// reason none of the above already names. One bucket, deliberately:
    /// `jsonwebtoken::errors::ErrorKind`'s wording is written for its own
    /// callers and changes between versions, so this taxonomy does not
    /// branch on it. The specific `ErrorKind` is for the operator log; the
    /// caller gets only that verification failed.
    SignatureInvalid,
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structure(e) => write!(f, "{e}"),
            Self::AlgNotPermitted => {
                write!(f, "alg is not permitted by this provider's configuration")
            }
            Self::NoJwksUri => write!(f, "provider has no jwks_uri to verify against"),
            Self::Jwks(e) => write!(f, "{e}"),
            Self::KeySelection(e) => write!(f, "{e}"),
            Self::SignatureInvalid => write!(f, "signature verification failed"),
        }
    }
}

impl std::error::Error for VerificationError {}

/// RFC 096 `:648`: claims are exposed only once signature verification has
/// completed. There is no public constructor and no public field: the only
/// way to have one of these is this module's own verifying functions, which
/// is why they live in this module rather than behind an accessor that could
/// be called on an unverified payload — see `ValidatedDiscovery` for the same
/// shape used the same way.
#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedIdTokenClaims {
    payload: serde_json::Value,
}

impl VerifiedIdTokenClaims {
    pub fn sub(&self) -> Option<&str> {
        self.payload.get("sub").and_then(serde_json::Value::as_str)
    }

    pub fn email(&self) -> Option<&str> {
        self.payload
            .get("email")
            .and_then(serde_json::Value::as_str)
    }

    pub fn email_verified(&self) -> bool {
        self.payload
            .get("email_verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    pub fn nonce(&self) -> Option<&str> {
        self.payload
            .get("nonce")
            .and_then(serde_json::Value::as_str)
    }
}

/// Built in this one function so the reason is stated once: `decoding.rs:335-348`
/// guards the empty-algorithm error, the key-family binding and the `alg`
/// allowlist all behind the single `validation.validate_signature` flag, so
/// nothing here ever sets it to anything but its default `true`.
///
/// `validate_aud` is turned off, deliberately rather than by omission. Left at
/// its default `true` with no expected audience configured, `jsonwebtoken`'s
/// own validation rejects *every* token that carries an `aud` claim at all —
/// which every real OIDC ID token does, since `aud` is required by the spec.
/// Verified by reading `validation.rs`'s own match on `(Audience::Parsed(_),
/// None)`, not assumed. Checking that this provider's `client_id` is in `aud`
/// is claims matching, not signature verification, and is out of this
/// stage's scope; disabling the check here is the only way that does not
/// reject every valid token by accident. `exp` is left at the library's
/// default (required, checked): a stale signature is a freshness property of
/// the token itself, not a per-provider value to match.
fn validation_for(algorithm: jsonwebtoken::Algorithm) -> jsonwebtoken::Validation {
    let mut validation = jsonwebtoken::Validation::new(algorithm);
    validation.validate_aud = false;
    validation
}

/// The pure core: verify `token` against an already-fetched `jwks`, given the
/// provider's configured `id_token_algs`. No network, no decision about
/// whether to fetch — that is [`verify_id_token`]'s job, because the
/// "no `jwks_uri`" refusal can only be made by whatever decides there is
/// nothing to fetch from.
pub fn verify_id_token_against_jwks(
    token: &str,
    id_token_algs: &[String],
    jwks: &jwks::Jwks,
) -> Result<VerifiedIdTokenClaims, VerificationError> {
    let compact = parse_compact_jws(token).map_err(VerificationError::Structure)?;

    // The outer gate: before key selection, not after, so a disallowed `alg`
    // is refused for that reason even when a key for it exists in the set.
    if !id_token_algs
        .iter()
        .any(|allowed| allowed == &compact.header.alg)
    {
        return Err(VerificationError::AlgNotPermitted);
    }

    let decoding_key = jwks::select_key(jwks, &compact.header.kid, &compact.header.alg)
        .map_err(VerificationError::KeySelection)?;

    // `compact.header.alg` is already one of the four strings stage 1
    // validated into the provider's configuration and the membership check
    // above just confirmed it is in that set, so this always parses.
    let algorithm = compact
        .header
        .alg
        .parse::<jsonwebtoken::Algorithm>()
        .map_err(|_| VerificationError::AlgNotPermitted)?;
    let validation = validation_for(algorithm);

    let token_data: jsonwebtoken::TokenData<serde_json::Value> =
        jsonwebtoken::decode(token, &decoding_key, &validation)
            .map_err(|_| VerificationError::SignatureInvalid)?;

    Ok(VerifiedIdTokenClaims {
        payload: token_data.claims,
    })
}

/// Verify `token` for a provider whose discovery named `jwks_uri` and whose
/// configuration allows `id_token_algs`. Fetches the JWKS itself, on `client`,
/// which must be the RFC 134 federation client. Refuses immediately, before
/// any network access, if `jwks_uri` is `None`.
pub async fn verify_id_token(
    client: &reqwest::Client,
    token: &str,
    id_token_algs: &[String],
    jwks_uri: Option<&str>,
) -> Result<VerifiedIdTokenClaims, VerificationError> {
    let Some(jwks_uri) = jwks_uri else {
        return Err(VerificationError::NoJwksUri);
    };
    let jwks = jwks::fetch_jwks(client, jwks_uri)
        .await
        .map_err(VerificationError::Jwks)?;
    verify_id_token_against_jwks(token, id_token_algs, &jwks)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "id_token/tests.rs"]
mod tests;
