//! RFC 096-A stage 3a: the JWKS fetch, and the bounds on the document it returns.
//! Stage 3b: [`select_key`], the key-selection rules RFC 096 `:641-646` states
//! and `jsonwebtoken` does not enforce.
//!
//! Signature verification (stage 4) is not here. Nothing in this module is
//! reachable from production: the only caller is the tests, and
//! `decode_id_token_claims` is untouched.
//!
//! The fetch goes through the caller's client, which must be the RFC 134 federation
//! client ([`crate::egress::build_federation_client`]). This module builds no client
//! of its own.
//!
//! ## What is checked, in order
//!
//! 1. The body is at most [`crate::response_bounds::MAX_RESPONSE_BYTES`], on bytes
//!    actually read, with status 200 and a JSON media type (`read_bounded_body`).
//! 2. The bytes parse as JSON, and the root is an object.
//! 3. The member, array and string caps of [`crate::response_bounds::check_caps`]
//!    (128 members, 128 elements, 8 KiB strings).
//! 4. Nesting depth is at most [`MAX_JWKS_DEPTH`]. The root object is depth 1.
//! 5. No member name repeats anywhere in the document, declared or unknown.
//!    `serde_json` does not catch a repeated unknown name, so this reads the names
//!    from the bytes. It runs on the bytes because the parsed `Value` has already
//!    lost the repeats.
//! 6. `keys` is an array of at most [`MAX_JWKS_KEYS`] objects.
//! 7. No two keys share a nonempty `kid`. Two empty `kid`s are not a duplicate:
//!    RFC 096 says *nonempty*.

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

use crate::response_bounds::{self, BoundsError, MAX_RESPONSE_BYTES};

/// RFC 096's JWKS rule: at most 16 levels of nesting, the root object counted as 1.
pub const MAX_JWKS_DEPTH: usize = 16;

/// RFC 096's JWKS rule: at most 32 keys.
pub const MAX_JWKS_KEYS: usize = 32;

/// Why a JWKS was refused. One variant per rule.
#[derive(Debug)]
pub enum JwksError {
    /// The transport rules: status, media type, byte cap, the member, array and
    /// string caps, and the request itself.
    Bounds(BoundsError),
    /// The body is not JSON.
    NotJson,
    /// The root is JSON, but not an object.
    NotObject,
    /// `keys` is absent, or not an array.
    NoKeys,
    /// Nesting is deeper than [`MAX_JWKS_DEPTH`].
    TooDeep { limit: usize },
    /// A member name repeats. The name is the repeated one.
    DuplicateMember(String),
    /// More than [`MAX_JWKS_KEYS`] keys.
    TooManyKeys { limit: usize },
    /// The entry at `index` in `keys` is not an object.
    KeyNotObject { index: usize },
    /// Two keys share this nonempty `kid`.
    DuplicateKid(String),
}

impl std::fmt::Display for JwksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bounds(e) => write!(f, "JWKS transport: {e}"),
            Self::NotJson => write!(f, "JWKS is not JSON"),
            Self::NotObject => write!(f, "JWKS root is not an object"),
            Self::NoKeys => write!(f, "JWKS has no `keys` array"),
            Self::TooDeep { limit } => write!(f, "JWKS nests deeper than {limit} levels"),
            Self::DuplicateMember(name) => write!(f, "JWKS repeats member {name:?}"),
            Self::TooManyKeys { limit } => write!(f, "JWKS has more than {limit} keys"),
            Self::KeyNotObject { index } => write!(f, "JWKS key {index} is not an object"),
            Self::DuplicateKid(kid) => write!(f, "two JWKS keys share kid {kid:?}"),
        }
    }
}

impl std::error::Error for JwksError {}

/// A JWKS that passed every structural rule. Each entry of `keys` is a JSON object,
/// not yet interpreted: key selection is stage 3b's.
#[derive(Debug, PartialEq)]
pub struct Jwks {
    pub keys: Vec<serde_json::Value>,
}

/// Fetch a JWKS and check it. `jwks_uri` must be the value
/// [`crate::discovery::ValidatedDiscovery::jwks_uri`] returned: https, on an origin
/// the provider's configuration allows. This function does not re-check that.
pub async fn fetch_jwks(client: &reqwest::Client, jwks_uri: &str) -> Result<Jwks, JwksError> {
    let resp = client
        .get(jwks_uri)
        .send()
        .await
        .map_err(|e| JwksError::Bounds(BoundsError::Transport(e)))?;
    let bytes = response_bounds::read_bounded_body(resp)
        .await
        .map_err(JwksError::Bounds)?;
    parse_jwks(&bytes)
}

/// Check the bytes of a JWKS against every structural rule in the module doc.
pub fn parse_jwks(bytes: &[u8]) -> Result<Jwks, JwksError> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(JwksError::Bounds(BoundsError::TooLarge {
            limit: MAX_RESPONSE_BYTES,
        }));
    }

    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| JwksError::NotJson)?;
    if !value.is_object() {
        return Err(JwksError::NotObject);
    }
    response_bounds::check_caps(&value).map_err(JwksError::Bounds)?;

    let depth = nesting_depth(&value);
    if depth > MAX_JWKS_DEPTH {
        return Err(JwksError::TooDeep {
            limit: MAX_JWKS_DEPTH,
        });
    }

    if let Some(name) = first_repeated_member(bytes)? {
        return Err(JwksError::DuplicateMember(name));
    }

    let mut document = match value {
        serde_json::Value::Object(map) => map,
        _ => return Err(JwksError::NotObject),
    };
    let keys = match document.remove("keys") {
        Some(serde_json::Value::Array(keys)) => keys,
        _ => return Err(JwksError::NoKeys),
    };
    if keys.len() > MAX_JWKS_KEYS {
        return Err(JwksError::TooManyKeys {
            limit: MAX_JWKS_KEYS,
        });
    }

    let mut seen_kids: Vec<&str> = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let object = key.as_object().ok_or(JwksError::KeyNotObject { index })?;
        let kid = object.get("kid").and_then(serde_json::Value::as_str);
        if let Some(kid) = kid.filter(|kid| !kid.is_empty()) {
            if seen_kids.contains(&kid) {
                return Err(JwksError::DuplicateKid(kid.to_owned()));
            }
            seen_kids.push(kid);
        }
    }

    Ok(Jwks { keys })
}

/// The nesting depth of a parsed document. A scalar is 0; a container is one more
/// than its deepest child. The root object of `{"keys":[{}]}` is 3.
fn nesting_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => 1 + map.values().map(nesting_depth).max().unwrap_or(0),
        serde_json::Value::Array(items) => 1 + items.iter().map(nesting_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// The first member name that repeats, anywhere in the document, or `None`.
///
/// Reads the bytes, not a `Value`, because `serde_json::Value` keeps only the last
/// of two equal names. The bytes already passed `serde_json`'s own parse, so the
/// `Err` branch below is unreachable in practice; it is reported as `NotJson`.
fn first_repeated_member(bytes: &[u8]) -> Result<Option<String>, JwksError> {
    let mut found: Option<String> = None;
    let mut de = serde_json::Deserializer::from_slice(bytes);
    match (NoRepeats { found: &mut found }).deserialize(&mut de) {
        Ok(()) => Ok(None),
        Err(_) if found.is_some() => Ok(found),
        Err(_) => Err(JwksError::NotJson),
    }
}

/// Walks every value of a JSON document and stops at the first repeated member
/// name. The name is written to `found`; the walk then fails with a custom error,
/// which the caller reads from `found`, not from the error text.
struct NoRepeats<'a> {
    found: &'a mut Option<String>,
}

impl<'de> DeserializeSeed<'de> for NoRepeats<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(NoRepeatsVisitor { found: self.found })
    }
}

struct NoRepeatsVisitor<'a> {
    found: &'a mut Option<String>,
}

impl<'de> Visitor<'de> for NoRepeatsVisitor<'_> {
    type Value = ();

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut names: Vec<String> = Vec::new();
        while let Some(name) = map.next_key::<String>()? {
            if names.contains(&name) {
                *self.found = Some(name);
                return Err(serde::de::Error::custom("repeated member"));
            }
            names.push(name);
            map.next_value_seed(NoRepeats {
                found: &mut *self.found,
            })?;
        }
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(NoRepeats {
                found: &mut *self.found,
            })?
            .is_some()
        {}
        Ok(())
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
}

// ---- RFC 096-A stage 3b: JWKS key selection (RFC 096 :641-646) -------------
//
// `jsonwebtoken`'s `jwk` module converts a JWK into verification material; it
// enforces none of the selection rules below -- not `kid`, not `use`/`key_ops`,
// not a private-key member, not RSA's minimum size or exponent shape, and not
// "exactly one". Every rule here runs on the key's raw JSON. The typed
// `jsonwebtoken::jwk::Jwk` is used only once, to convert the single surviving
// candidate into a `DecodingKey` -- never to decide whether it survives.

/// RFC 7518 §6.3.2's RSA private members, plus the EC and OKP private members
/// §6.2.2 and §6.3.2 name with the same two letters do not collide with these;
/// RFC 096 names exactly this set. None may appear on a key this stage treats
/// as public.
const PRIVATE_KEY_MEMBERS: [&str; 7] = ["d", "p", "q", "dp", "dq", "qi", "oth"];

/// Why no key, or more than one key, was selected from a JWKS for a header.
/// One variant per rule, so an operator can tell which one fired. Every check
/// before `KeyMaterialInvalid` runs on the raw JSON.
#[derive(Debug)]
pub enum KeySelectionError {
    /// No key in the set has the header's `kid`.
    KidNotFound,
    /// More than one key in the set has the header's `kid`. `parse_jwks` already
    /// refuses a duplicate nonempty `kid` within one document; this exists for a
    /// `Jwks` built directly rather than through `parse_jwks`, so selection stays
    /// safe even then.
    AmbiguousKid { count: usize },
    /// The raw JWK object carries a private-key member. Checked on the JSON,
    /// because the typed library struct has no field for any of them and
    /// silently drops them.
    PrivateKeyMember(&'static str),
    /// `use` is present and is not `"sig"`.
    UseNotSig,
    /// `key_ops` is present and does not contain `"verify"`.
    KeyOpsMissingVerify,
    /// `key_ops` names `"sign"` alongside `"verify"`, or names an operation
    /// other than those two while `use` is `"sig"`.
    MultiUse,
    /// The key's own `alg` is present and is not the header's `alg`.
    AlgMismatch,
    /// The header's algorithm family does not match the key's `kty`, or the
    /// header's `alg` is not one this stage recognizes at all.
    AlgorithmFamilyMismatch,
    /// `ES256` with a curve other than `P-256`, or `EdDSA` with a curve other
    /// than `Ed25519`.
    CurveMismatch,
    /// An RSA key's `n`, `e`, or both, are absent or not valid base64url.
    RsaParameterInvalid,
    /// An RSA key's modulus decodes to fewer than 256 bytes (2,048 bits).
    RsaTooSmall,
    /// An RSA key's exponent decodes to an even value, or to a value less
    /// than 3.
    RsaInvalidExponent,
    /// The chosen key passed every rule above but the library could not
    /// convert it. Not expected; recorded rather than panicking.
    KeyMaterialInvalid,
}

impl std::fmt::Display for KeySelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KidNotFound => write!(f, "no JWKS key matches the token's kid"),
            Self::AmbiguousKid { count } => {
                write!(f, "{count} JWKS keys share the token's kid")
            }
            Self::PrivateKeyMember(name) => {
                write!(f, "the selected JWK carries the private member {name:?}")
            }
            Self::UseNotSig => write!(f, "the selected JWK's `use` is not `sig`"),
            Self::KeyOpsMissingVerify => {
                write!(f, "the selected JWK's `key_ops` does not include `verify`")
            }
            Self::MultiUse => write!(f, "the selected JWK is usable for more than verification"),
            Self::AlgMismatch => write!(f, "the selected JWK's `alg` does not match the header"),
            Self::AlgorithmFamilyMismatch => {
                write!(
                    f,
                    "the selected JWK's `kty` does not match the header's algorithm"
                )
            }
            Self::CurveMismatch => {
                write!(f, "the selected JWK's curve does not match the algorithm")
            }
            Self::RsaParameterInvalid => write!(f, "the selected JWK's RSA parameters are invalid"),
            Self::RsaTooSmall => write!(
                f,
                "the selected JWK's RSA modulus is smaller than 2,048 bits"
            ),
            Self::RsaInvalidExponent => write!(f, "the selected JWK's RSA exponent is invalid"),
            Self::KeyMaterialInvalid => write!(f, "the selected JWK could not be converted"),
        }
    }
}

impl std::error::Error for KeySelectionError {}

/// RFC 096 `:641-646`: select the one key in `jwks` that matches `header_kid`
/// and is compatible with `header_alg`, and convert it to a [`DecodingKey`].
///
/// `header_alg` is not checked against the provider's configured
/// `id_token_algs` here -- that allowlist is stage 1's and stage 4's. This
/// matches a key to whatever `header_alg` already is.
pub fn select_key(
    jwks: &Jwks,
    header_kid: &str,
    header_alg: &str,
) -> Result<jsonwebtoken::DecodingKey, KeySelectionError> {
    let matching_kid: Vec<&serde_json::Value> = jwks
        .keys
        .iter()
        .filter(|k| k.get("kid").and_then(serde_json::Value::as_str) == Some(header_kid))
        .collect();
    let candidate = match matching_kid.as_slice() {
        [] => return Err(KeySelectionError::KidNotFound),
        [one] => *one,
        many => {
            return Err(KeySelectionError::AmbiguousKid { count: many.len() });
        }
    };

    for member in PRIVATE_KEY_MEMBERS {
        if candidate.get(member).is_some() {
            return Err(KeySelectionError::PrivateKeyMember(member));
        }
    }

    // `use` is checked before `key_ops`: a `use` other than `sig` is refused
    // on its own, so by the time `key_ops` is read, `use` is `sig` or absent.
    if let Some(use_) = candidate.get("use").and_then(serde_json::Value::as_str)
        && use_ != "sig"
    {
        return Err(KeySelectionError::UseNotSig);
    }

    if let Some(ops) = candidate
        .get("key_ops")
        .and_then(serde_json::Value::as_array)
    {
        let ops: Vec<&str> = ops.iter().filter_map(serde_json::Value::as_str).collect();
        if !ops.contains(&"verify") {
            return Err(KeySelectionError::KeyOpsMissingVerify);
        }
        // "disagree": `use` is absent or `sig` here (checked above), so a
        // `key_ops` naming anything beyond the signing pair contradicts a
        // single verification purpose, as does naming both halves of the pair.
        if ops.contains(&"sign") || ops.iter().any(|op| *op != "sign" && *op != "verify") {
            return Err(KeySelectionError::MultiUse);
        }
    }

    if let Some(key_alg) = candidate.get("alg").and_then(serde_json::Value::as_str)
        && key_alg != header_alg
    {
        return Err(KeySelectionError::AlgMismatch);
    }

    let required_kty = match header_alg {
        "RS256" | "RS384" | "RS512" | "PS256" | "PS384" | "PS512" => "RSA",
        "ES256" => "EC",
        "EdDSA" => "OKP",
        _ => return Err(KeySelectionError::AlgorithmFamilyMismatch),
    };
    if candidate.get("kty").and_then(serde_json::Value::as_str) != Some(required_kty) {
        return Err(KeySelectionError::AlgorithmFamilyMismatch);
    }

    match required_kty {
        "EC" => check_curve(candidate, "P-256")?,
        "OKP" => check_curve(candidate, "Ed25519")?,
        "RSA" => check_rsa_parameters(candidate)?,
        _ => unreachable!("required_kty is one of the three values matched above"),
    }

    let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(candidate.clone())
        .map_err(|_| KeySelectionError::KeyMaterialInvalid)?;
    jsonwebtoken::DecodingKey::from_jwk(&jwk).map_err(|_| KeySelectionError::KeyMaterialInvalid)
}

fn check_curve(candidate: &serde_json::Value, required: &str) -> Result<(), KeySelectionError> {
    if candidate.get("crv").and_then(serde_json::Value::as_str) == Some(required) {
        Ok(())
    } else {
        Err(KeySelectionError::CurveMismatch)
    }
}

/// RFC 096's RSA rules: the modulus decodes to at least 256 bytes (2,048
/// bits), and the exponent decodes to an odd value of at least 3 -- "valid
/// exponent" is the RFC's phrase; this is this implementation of it.
///
/// The exponent check reads only the decoded byte length and the low bit of
/// the last byte, never a numeric value: after leading zero bytes are
/// stripped, any result of two or more bytes already exceeds 255, so only a
/// zero- or one-byte result can be below 3.
fn check_rsa_parameters(candidate: &serde_json::Value) -> Result<(), KeySelectionError> {
    use base64ct::{Base64UrlUnpadded, Encoding};

    let n = candidate
        .get("n")
        .and_then(serde_json::Value::as_str)
        .ok_or(KeySelectionError::RsaParameterInvalid)?;
    let e = candidate
        .get("e")
        .and_then(serde_json::Value::as_str)
        .ok_or(KeySelectionError::RsaParameterInvalid)?;
    let n_bytes =
        Base64UrlUnpadded::decode_vec(n).map_err(|_| KeySelectionError::RsaParameterInvalid)?;
    let e_bytes =
        Base64UrlUnpadded::decode_vec(e).map_err(|_| KeySelectionError::RsaParameterInvalid)?;

    if n_bytes.len() < 256 {
        return Err(KeySelectionError::RsaTooSmall);
    }

    let even = match e_bytes.last() {
        Some(last) => last & 1 == 0,
        None => true,
    };
    let trimmed = {
        let mut s = e_bytes.as_slice();
        while s.first() == Some(&0) {
            s = &s[1..];
        }
        s
    };
    let too_small = trimmed.is_empty() || (trimmed.len() == 1 && trimmed[0] < 3);
    if even || too_small {
        return Err(KeySelectionError::RsaInvalidExponent);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "jwks/tests.rs"]
mod tests;
