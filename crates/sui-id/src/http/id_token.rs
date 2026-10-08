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
    /// A top-level payload member (`iss`, `sub`, `aud`, `azp`, or any other)
    /// appears more than once. The name is the repeated one. Checked here,
    /// over `compact.payload`, before `jsonwebtoken::decode` ever runs: that
    /// function deserializes the same bytes a second time, internally, into
    /// a struct that already rejects a duplicate `iss`/`sub`/`aud` on its
    /// own -- but collapsed into `SignatureInvalid`, which is false on a
    /// token whose signature is in fact valid. Scanning first, and naming
    /// the real fault, is stage 6a's fix for that.
    DuplicatePayloadMember(String),
    /// `exp` or `nbf` is present but is not a canonical non-negative JSON
    /// integer within the range the application clock can represent --
    /// `jsonwebtoken`'s own `numeric_type` deserializer *rounds* a float
    /// into an accepted integer instead of refusing it (confirmed
    /// empirically: a literal `exp` of `1700000000.5` decodes and verifies
    /// successfully against the real library), so this is checked before
    /// `decode` runs, over the raw payload value, the same reason and the
    /// same placement as `DuplicatePayloadMember`. The name is which claim.
    MalformedTimeClaim(&'static str),
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
    /// `exp` has passed (`jsonwebtoken`'s own `ErrorKind::ExpiredSignature`),
    /// named for the same reason stage 6a named `DuplicatePayloadMember`
    /// instead of leaving it in `SignatureInvalid`: the signature on an
    /// expired token did verify, so reporting it as a signature failure is
    /// false, not merely imprecise.
    Expired,
    /// `nbf` is in the future (`ErrorKind::ImmatureSignature`), named for
    /// the identical reason as `Expired`, one claim over.
    NotYetValid,
    /// The signature itself did not verify, or `jsonwebtoken` rejected the
    /// token for a reason none of the above already names. Still one
    /// bucket, deliberately, for whatever is left: `ErrorKind`'s wording is
    /// written for its own callers and changes between versions, so this
    /// taxonomy does not branch on all of it -- only on the two kinds above,
    /// which are false statements under their old name, not merely vague
    /// ones. The specific `ErrorKind` is for the operator log; the caller
    /// gets only that verification failed.
    SignatureInvalid,
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Structure(e) => write!(f, "{e}"),
            Self::DuplicatePayloadMember(name) => write!(f, "payload repeats member {name:?}"),
            Self::MalformedTimeClaim(name) => {
                write!(f, "{name} is not a canonical in-range integer")
            }
            Self::AlgNotPermitted => {
                write!(f, "alg is not permitted by this provider's configuration")
            }
            Self::NoJwksUri => write!(f, "provider has no jwks_uri to verify against"),
            Self::Jwks(e) => write!(f, "{e}"),
            Self::KeySelection(e) => write!(f, "{e}"),
            Self::Expired => write!(f, "exp has passed"),
            Self::NotYetValid => write!(f, "nbf is in the future"),
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

    /// Any top-level payload member by name, for a claim this module itself
    /// has no accessor for. `pub(crate)`: a sibling claims-validation module
    /// reads through this; nothing outside the crate gets a `Value` back.
    pub(crate) fn claim(&self, name: &str) -> Option<&serde_json::Value> {
        self.payload.get(name)
    }
}

/// Built in this one function so the reason is stated once: `decoding.rs:335-348`
/// guards the empty-algorithm error, the key-family binding and the `alg`
/// allowlist all behind the single `validation.validate_signature` flag, so
/// nothing here ever sets it to anything but its default `true`.
///
/// `validate_aud` stays off, and stays a deliberate choice rather than an
/// omission now that stage 6a's `identity_claims::validate_identity_claims`
/// is what checks `aud` -- not a gap stage 6a still has to fill. Two reasons
/// it is ours rather than the library's: `jsonwebtoken::Validation::aud` can
/// only express "the token's `aud` overlaps this set" (`validation.rs`'s
/// `is_subset`, read directly, is actually an intersection check, not a true
/// subset test), with no concept of a count bound, no uniqueness check, and
/// no `azp` at all -- RFC 096's `aud` rule needs all three, so splitting it
/// across the library's config and our own code would mean one rule reasoned
/// about in two places. Separately, left at its default `true` with no
/// expected audience configured, `jsonwebtoken`'s own validation rejects
/// *every* token that carries an `aud` claim at all -- which every real OIDC
/// ID token does. Verified by reading `validation.rs`'s own match on
/// `(Audience::Parsed(_), None)`, not assumed; that landmine is what stage 4a
/// found and is why this was already off before stage 6a existed.
///
/// `exp` is left at the library's own window check (`validate_exp = true`,
/// `leeway = 60`, `required_spec_claims = {"exp"}`, none of them touched):
/// `exp - 0 < now - 60` rejects, which holds while `now <= exp + 60` --
/// confirmed against the real library, not the RFC's own paraphrase, which
/// states the boundary as the strict `now < exp + 60`. The library accepts
/// one second the RFC's literal wording would not (`now == exp + 60`); both
/// reject at `now == exp + 61`. Stated as a finding in stage 6b's package,
/// not patched here: the dispatch was explicit that `exp`'s window logic is
/// not this function's to change.
///
/// `validate_nbf` is turned **on** (stage 6b): the library's own check,
/// `nbf > now + leeway` rejects, is RFC 096's `nbf <= now + 60s` rule
/// verbatim. `nbf` stays out of `required_spec_claims` -- the RFC marks it
/// optional, and adding it there would make it required instead.
///
/// `iat` has no library support at all and is validated entirely by
/// `time_claims::validate_time_claims`, against `VerifiedIdTokenClaims`,
/// after this function's checks have already passed.
fn validation_for(algorithm: jsonwebtoken::Algorithm) -> jsonwebtoken::Validation {
    let mut validation = jsonwebtoken::Validation::new(algorithm);
    validation.validate_aud = false;
    validation.validate_nbf = true;
    validation
}

/// Why a raw JSON value is not usable as a NumericDate (RFC 096 `:673-676`):
/// not a number at all, a float (including one with a zero fraction --
/// `serde_json` tracks the literal's own syntax, so `1700000000.0` is
/// `is_f64`, not `is_u64`, confirmed empirically), a negative integer, or an
/// integer too large for the application clock to represent at all (an
/// overflow past `i64`/`u64` falls back to `serde_json`'s float
/// representation too, landing in the same bucket as a genuine float).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericDateError {
    WrongType,
    NotAnInteger,
    Negative,
    OutOfRange,
}

/// Parses a raw JSON value as a canonical NumericDate: a JSON integer,
/// non-negative, within the range `chrono::DateTime::from_timestamp` can
/// represent -- which is also how "representable by the application clock"
/// is given a concrete boundary rather than an arbitrary one. Returns the
/// value as epoch seconds; callers needing a `DateTime<Utc>` can construct
/// one from it without a second fallibility check, since representability
/// was already confirmed here.
pub(crate) fn numeric_date(value: &serde_json::Value) -> Result<i64, NumericDateError> {
    if let Some(n) = value.as_u64() {
        let secs = i64::try_from(n).map_err(|_| NumericDateError::OutOfRange)?;
        if chrono::DateTime::from_timestamp(secs, 0).is_none() {
            return Err(NumericDateError::OutOfRange);
        }
        return Ok(secs);
    }
    if value.as_i64().is_some() {
        // `as_u64` already failed, so a value `as_i64` can represent is
        // necessarily negative.
        return Err(NumericDateError::Negative);
    }
    if value.is_number() {
        // Not representable as `u64` or `i64`: a float (any fraction,
        // including none) or an integer past both ranges.
        return Err(NumericDateError::NotAnInteger);
    }
    Err(NumericDateError::WrongType)
}

/// The first top-level member name that occurs more than once in a JSON
/// object's bytes, scanned the same way `parse_compact_jws` scans the
/// header -- positional, so a non-adjacent repeat (`{"iss":"a","sub":"s",
/// "iss":"b"}`) is still caught, not just a repeated-neighbour case. A
/// payload that is not a JSON object at all yields no names and so no
/// duplicate; `jsonwebtoken::decode`'s own parsing is what reports that.
fn first_duplicate_member(payload: &[u8]) -> Option<String> {
    let names = serde_json::from_slice::<MemberNames>(payload)
        .map(|m| m.0)
        .unwrap_or_default();
    for (i, name) in names.iter().enumerate() {
        if names[..i].contains(name) {
            return Some(name.clone());
        }
    }
    None
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

    // Scanned over the already-decoded, already-bounded payload bytes,
    // before `jsonwebtoken::decode` runs -- it deserializes the same bytes
    // a second time, internally, into a struct that already rejects a
    // duplicate `iss`/`sub`/`aud` on its own, but collapsed into
    // `SignatureInvalid`, a false statement about a token whose signature
    // is in fact valid. Checking here, first, names the real fault instead.
    if let Some(name) = first_duplicate_member(&compact.payload) {
        return Err(VerificationError::DuplicatePayloadMember(name));
    }

    // `jsonwebtoken`'s own `numeric_type` deserializer rounds a float `exp`/
    // `nbf` into an accepted integer rather than refusing it -- see
    // `MalformedTimeClaim`'s doc comment. Checked over the raw payload
    // value, before `decode` runs, the same placement as the scan above,
    // for the same reason: by the time `decode` would reject a malformed
    // shape itself (if it did at all), it is too late to not have already
    // rounded a float into something that looks valid.
    let raw_payload: serde_json::Value =
        serde_json::from_slice(&compact.payload).unwrap_or(serde_json::Value::Null);
    for claim in ["exp", "nbf"] {
        if let Some(value) = raw_payload.get(claim)
            && numeric_date(value).is_err()
        {
            return Err(VerificationError::MalformedTimeClaim(claim));
        }
    }

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
        jsonwebtoken::decode(token, &decoding_key, &validation).map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => VerificationError::Expired,
            jsonwebtoken::errors::ErrorKind::ImmatureSignature => VerificationError::NotYetValid,
            _ => VerificationError::SignatureInvalid,
        })?;

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
