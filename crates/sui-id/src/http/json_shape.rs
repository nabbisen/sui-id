//! Generic JSON-structure utilities, with nothing JWKS- or discovery-specific
//! in them. Moved out of `jwks.rs` -- the three tidies, 2026-10-20 -- where
//! both functions had been *promoted to `pub(crate)` in place* since RFC
//! 096-A stage 8 rather than relocated, on the grounds that relocating them
//! was a larger footprint on an already-accepted, tested module than a
//! visibility change alone. That tradeoff no longer applies on its own: both
//! callers ([`crate::jwks::parse_jwks`] and
//! [`crate::discovery::validate_discovery_bytes`]) already exist and are
//! unaffected by where the walkers live, so this is a pure move, not a
//! behavior change.
//!
//! **[`first_repeated_member`] no longer returns [`crate::jwks::JwksError`].**
//! That return type was the one JWKS-specific thing left in either function --
//! a module with "nothing JWKS-specific" in it should not import JWKS's own
//! error enum just to report its one failure mode. It now returns the local
//! [`NotJson`] marker, and each caller translates it into whatever its own
//! "not JSON" variant already is. `jwks.rs`'s caller used to get this for
//! free through `?`; it now does the same explicit `map_err` discovery.rs's
//! caller already did, which is the only line this move changes evidence
//! for.

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

/// The nesting depth of a parsed document. A scalar is 0; a container is one
/// more than its deepest child. The root object of `{"keys":[{}]}` is 3.
pub(crate) fn nesting_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => 1 + map.values().map(nesting_depth).max().unwrap_or(0),
        serde_json::Value::Array(items) => 1 + items.iter().map(nesting_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// The one way [`first_repeated_member`] can fail: the bytes did not parse
/// as JSON. A marker, not an enum -- this module has exactly one failure
/// mode, and no caller distinguishes a second.
#[derive(Debug)]
pub(crate) struct NotJson;

/// The first member name that repeats, anywhere in the document, or `None`.
///
/// Reads the bytes, not a `Value`, because `serde_json::Value` keeps only the last
/// of two equal names. The bytes already passed `serde_json`'s own parse at the
/// caller, so the `Err` branch below is unreachable in practice; it is reported
/// as [`NotJson`].
pub(crate) fn first_repeated_member(bytes: &[u8]) -> Result<Option<String>, NotJson> {
    let mut found: Option<String> = None;
    let mut de = serde_json::Deserializer::from_slice(bytes);
    match (NoRepeats { found: &mut found }).deserialize(&mut de) {
        Ok(()) => Ok(None),
        Err(_) if found.is_some() => Ok(found),
        Err(_) => Err(NotJson),
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
#[path = "json_shape/tests.rs"]
mod tests;
