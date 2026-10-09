//! RFC 134 D5 Tier 1 — response bounds derived from a measured provider
//! corpus, enforced on every federation response body before it reaches
//! the caller's typed struct.
//!
//! ## The corpus
//!
//! Fetched 2026-10-04, each provider's discovery document and the JWKS
//! it names, over real HTTPS:
//!
//! | Provider | Reached | Discovery bytes | JWKS bytes |
//! |---|---|---|---|
//! | Google | yes | 1,399 | 1,033 |
//! | Microsoft Entra (`common`) | yes | 1,728 | 11,250 |
//! | GitLab (`gitlab.com`) | yes | 1,631 | 1,707 |
//! | Auth0 (`auth0.auth0.com`) | yes | 2,548 | 3,084 |
//! | Okta (`dev-84941762.okta.com`, a tenant named in Okta's own public
//!   JetBrains-integration documentation) | yes | 2,682 | 462 |
//! | Keycloak | **no** — self-hosted; no stable, official public instance found |
//! | Authentik | **no** — self-hosted; no stable, official public instance found (its discovery path is also per-application, not a single fixed root) |
//!
//! **Userinfo could not be measured live for any provider** — every
//! userinfo endpoint requires a real access token from a completed
//! sign-in, which this measurement had no legitimate way to obtain. Used
//! instead: the OIDC Core 1.0 §5.1 Standard Claims set, every optional
//! claim populated with a representative value (833 bytes, 20 members,
//! longest string 59 bytes) — well inside the discovery/JWKS-derived
//! bounds below, so it does not change the chosen numbers, but recorded
//! here rather than silently omitted.
//!
//! Measured maxima across discovery + JWKS + the userinfo stand-in:
//!
//! - **Body size: 11,250 bytes** (Microsoft Entra's JWKS — it carries 7
//!   active signing keys at once, each with a certificate chain).
//! - **Object members: 29** (Auth0's and Okta's discovery documents).
//! - **Array length: 31** (Okta's discovery document).
//! - **String length: 1,044 bytes** — an `x5c` certificate-chain entry in
//!   Microsoft's JWKS, *not* the RSA modulus (`n`), which is a
//!   consistent 342 bytes across every provider in the corpus; checked
//!   per-field, not assumed from the raw maximum.
//!
//! ## The bounds, each at observed maximum times a stated headroom
//!
//! - [`MAX_RESPONSE_BYTES`]: 64 KiB, ~5.8x the observed 11,250 bytes.
//! - [`MAX_MEMBERS`] / [`MAX_ARRAY_LEN`]: 128, ~4.4x / ~4.1x the observed
//!   29 / 31 — the same number for both, deliberately: one policy to
//!   state and justify, not two, and it doubles as a visible echo of
//!   Tier 2's own 128 recursion-depth limit (`serde_json`'s default,
//!   pinned and evidenced rather than re-implemented — see below).
//! - [`MAX_STRING_LEN`]: 8 KiB, ~7.8x the observed 1,044 bytes — enough
//!   headroom for a 4096-bit RSA key's certificate chain entry without
//!   re-tuning the first time a provider rotates to a larger key.
//!
//! A bound nobody can justify is the one that gets raised the first time
//! something legitimate trips it — these numbers are justified against
//! real responses, not chosen round.
//!
//! ## What this does not re-implement
//!
//! Depth and duplicate-declared-key rejection already hold in this
//! workspace's pinned `serde_json` 1.0.150 / `serde` 1.0.228 (checked
//! directly against the locked versions, not the handoff's own citation
//! of `serde` 1.0.196 — a different version than what `Cargo.lock`
//! actually pins today; the *behavior* was re-verified against the real
//! locked versions rather than trusted from an older citation):
//! `serde_json::from_str` on a 200-deep array fails with "recursion limit
//! exceeded" (its `remaining_depth` default is 128), and
//! `serde_json::from_str` into a derived `Deserialize` struct with a
//! duplicate declared field fails with `"duplicate field '...'"`. Both
//! checked against a throwaway struct before writing the pinning tests
//! below, not assumed from reading the dispatch. **Duplicate *unknown*
//! keys are not caught** — `serde` skips unknown fields without
//! duplicate detection, and `deny_unknown_fields` is deliberately not
//! the fix (RFC 134 D5 Tier 1): a discovery document legitimately
//! carries members this crate does not model, and rejecting the whole
//! document over one would break real providers.
//!
//! ## Status and media type
//!
//! RFC 096's matrix allows "200, or 304 under the cache rules" — reduced
//! here to 200 only, stated rather than silently narrowed: nothing on
//! this path sends a conditional request (`If-None-Match`/`If-Modified-
//! Since`), so a well-behaved upstream has no legitimate reason to answer
//! 304, and accepting one anyway would mean serving content from
//! nowhere. Conditional caching is RFC 096's broader "Cache and
//! telemetry" matrix section, not part of this dispatch.

use serde::de::DeserializeOwned;

/// RFC 134 D5 Tier 1: observed maximum (11,250 bytes, Microsoft Entra's
/// JWKS) times ~5.8 headroom, rounded to a KiB boundary.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// RFC 134 D5 Tier 1: observed maximum (29, both Auth0's and Okta's
/// discovery documents) times ~4.4 headroom, matching [`MAX_ARRAY_LEN`]
/// and echoing `serde_json`'s own 128 recursion-depth default.
pub const MAX_MEMBERS: usize = 128;

/// RFC 134 D5 Tier 1: observed maximum (31, Okta's discovery document)
/// times ~4.1 headroom, matching [`MAX_MEMBERS`].
pub const MAX_ARRAY_LEN: usize = 128;

/// RFC 134 D5 Tier 1: observed maximum (1,044 bytes, an `x5c`
/// certificate-chain entry in Microsoft Entra's JWKS) times ~7.8
/// headroom, rounded to a KiB boundary.
pub const MAX_STRING_LEN: usize = 8 * 1024;

/// RFC 096-A stage 8: [`check_caps`]'s member/array/string bounds,
/// parameterized rather than hardcoded. Added when discovery's own RFC
/// 096 `:531-532` bounds (32 array members, 2,048 bytes per string) turned
/// out to be four times tighter than this module's `MAX_ARRAY_LEN`/
/// `MAX_STRING_LEN` — those two constants are JWKS's and the shared
/// transport's, measured from a real provider corpus (see the module doc
/// comment) and explicitly not to be tightened for every caller, since
/// that would silently re-scope stage 3a. Two near-identical bounds
/// walkers would be the duplication this RFC keeps rejecting (6a's `aud`
/// reasoning, 6c's bound-pinning fix), so the walker itself
/// ([`check_caps`]) stays one function; only the numbers it compares
/// against vary per caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseCaps {
    pub max_members: usize,
    pub max_array_len: usize,
    pub max_string_len: usize,
}

impl ResponseCaps {
    /// JWKS's and the shared transport's own bounds -- [`read_bounded_json`]'s
    /// default, unchanged from before this stage, so every existing caller
    /// (JWKS, and federation.rs's three live `read_bounded_json` call
    /// sites, including its own discovery fetch) sees identical behaviour.
    pub const JWKS_AND_TRANSPORT: Self = Self {
        max_members: MAX_MEMBERS,
        max_array_len: MAX_ARRAY_LEN,
        max_string_len: MAX_STRING_LEN,
    };

    /// RFC 096 `:531-532`'s own discovery bounds, verbatim: 128 object
    /// members (same as [`Self::JWKS_AND_TRANSPORT`]), 32 array members,
    /// 2,048 bytes per string.
    pub const DISCOVERY: Self = Self {
        max_members: MAX_MEMBERS,
        max_array_len: 32,
        max_string_len: 2_048,
    };
}

/// Why a response was rejected. `Display` is safe for a server log; the
/// detail must never reach the browser — every call site here maps this
/// into whatever its own existing "the fetch failed" surface already is
/// (RFC 134 D5 Tier 1 §4b: no new user-visible error class).
#[derive(Debug)]
pub enum BoundsError {
    WrongStatus(reqwest::StatusCode),
    MissingContentType,
    WrongContentType(String),
    TooLarge { limit: usize },
    TooManyMembers { limit: usize },
    ArrayTooLong { limit: usize },
    StringTooLong { limit: usize },
    Json(serde_json::Error),
    Transport(reqwest::Error),
}

impl std::fmt::Display for BoundsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongStatus(s) => write!(f, "unexpected status {s}"),
            Self::MissingContentType => write!(f, "response has no Content-Type"),
            Self::WrongContentType(ct) => write!(f, "unexpected Content-Type {ct:?}"),
            Self::TooLarge { limit } => write!(f, "response body exceeds {limit} bytes"),
            Self::TooManyMembers { limit } => write!(f, "a JSON object exceeds {limit} members"),
            Self::ArrayTooLong { limit } => write!(f, "a JSON array exceeds {limit} elements"),
            Self::StringTooLong { limit } => write!(f, "a JSON string exceeds {limit} bytes"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Transport(e) => write!(f, "transport error: {e}"),
        }
    }
}

impl std::error::Error for BoundsError {}

fn content_type_is_json(resp: &reqwest::Response) -> Result<(), BoundsError> {
    let raw = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .ok_or(BoundsError::MissingContentType)?;
    // Drop parameters (`; charset=utf-8`), lowercase, compare the bare
    // type/subtype only.
    let essence = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if essence == "application/json" || essence.ends_with("+json") {
        Ok(())
    } else {
        Err(BoundsError::WrongContentType(raw.to_owned()))
    }
}

/// Read the body with the cap enforced on bytes actually received, not
/// `Content-Length` — which is attacker-supplied, may be absent, and may
/// lie (RFC 134 D5 Tier 1 §4b). Stops as soon as the running total
/// exceeds `max_bytes`, never buffering past it.
async fn read_bounded_bytes(
    mut resp: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, BoundsError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(BoundsError::Transport)? {
        buf.extend_from_slice(&chunk);
        if buf.len() > max_bytes {
            return Err(BoundsError::TooLarge { limit: max_bytes });
        }
    }
    Ok(buf)
}

/// Depth is `serde_json`'s own job (pinned, see the module doc comment);
/// this walks the already-depth-bounded `Value` for the three caps
/// `serde_json` does not itself impose. `caps` lets JWKS and discovery
/// share this one walker while comparing against their own numbers --
/// see [`ResponseCaps`].
pub fn check_caps(value: &serde_json::Value, caps: &ResponseCaps) -> Result<(), BoundsError> {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() > caps.max_members {
                return Err(BoundsError::TooManyMembers {
                    limit: caps.max_members,
                });
            }
            for v in map.values() {
                check_caps(v, caps)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            if items.len() > caps.max_array_len {
                return Err(BoundsError::ArrayTooLong {
                    limit: caps.max_array_len,
                });
            }
            for v in items {
                check_caps(v, caps)?;
            }
            Ok(())
        }
        serde_json::Value::String(s) => {
            if s.len() > caps.max_string_len {
                return Err(BoundsError::StringTooLong {
                    limit: caps.max_string_len,
                });
            }
            Ok(())
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            Ok(())
        }
    }
}

/// The one bounded-response pipeline for every federation response body:
/// status, media type, a byte cap honored on bytes actually read, member/
/// array/string caps on the parsed structure, then (depth and duplicate-
/// declared-key already guaranteed by `serde_json`/`serde` themselves)
/// deserialize into `T`. Used at all three of `federation.rs`'s `.json()`
/// call sites — discovery, token response, userinfo.
pub async fn read_bounded_json<T: DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, BoundsError> {
    let bytes = read_bounded_body(resp).await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(BoundsError::Json)?;
    check_caps(&value, &ResponseCaps::JWKS_AND_TRANSPORT)?;
    serde_json::from_value(value).map_err(BoundsError::Json)
}

/// The status, media type and byte cap of [`read_bounded_json`], without the
/// parse. For a caller that must see the bytes themselves: the JWKS duplicate-
/// member rule cannot run on a `Value`, which has already lost the duplicates.
pub async fn read_bounded_body(resp: reqwest::Response) -> Result<Vec<u8>, BoundsError> {
    if resp.status() != reqwest::StatusCode::OK {
        return Err(BoundsError::WrongStatus(resp.status()));
    }
    content_type_is_json(&resp)?;
    read_bounded_bytes(resp, MAX_RESPONSE_BYTES).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "response_bounds/tests.rs"]
mod tests;
