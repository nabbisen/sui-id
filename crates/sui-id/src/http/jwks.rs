//! RFC 096-A stage 3a: the JWKS fetch, and the bounds on the document it returns.
//!
//! Structure only. Key selection (stage 3b) and signature verification (stage 4)
//! are not here. Nothing in this module is reachable from production: the only
//! caller is the tests, and `decode_id_token_claims` is untouched.
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "jwks/tests.rs"]
mod tests;
