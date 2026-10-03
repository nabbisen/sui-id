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
/// this walks the already-depth-bounded `Value` for the two caps
/// `serde_json` does not itself impose.
fn check_caps(value: &serde_json::Value) -> Result<(), BoundsError> {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() > MAX_MEMBERS {
                return Err(BoundsError::TooManyMembers { limit: MAX_MEMBERS });
            }
            for v in map.values() {
                check_caps(v)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            if items.len() > MAX_ARRAY_LEN {
                return Err(BoundsError::ArrayTooLong {
                    limit: MAX_ARRAY_LEN,
                });
            }
            for v in items {
                check_caps(v)?;
            }
            Ok(())
        }
        serde_json::Value::String(s) => {
            if s.len() > MAX_STRING_LEN {
                return Err(BoundsError::StringTooLong {
                    limit: MAX_STRING_LEN,
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
    if resp.status() != reqwest::StatusCode::OK {
        return Err(BoundsError::WrongStatus(resp.status()));
    }
    content_type_is_json(&resp)?;
    let bytes = read_bounded_bytes(resp, MAX_RESPONSE_BYTES).await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(BoundsError::Json)?;
    check_caps(&value)?;
    serde_json::from_value(value).map_err(BoundsError::Json)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Doc {
        a: String,
        b: Option<String>,
    }

    fn deep_array_json(depth: usize) -> String {
        let mut s = String::new();
        for _ in 0..depth {
            s.push('[');
        }
        for _ in 0..depth {
            s.push(']');
        }
        s
    }

    async fn serve_once(status_line: &str, extra_headers: &str, body: &str) -> reqwest::Response {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let status_line = status_line.to_owned();
        let extra_headers = extra_headers.to_owned();
        let body = body.to_owned();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut discard = [0u8; 1024];
            let _ = stream.read(&mut discard).await;
            let framed = format!(
                "{status_line}\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            let _ = stream.write_all(framed.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
        reqwest::Client::new()
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("connect")
    }

    #[tokio::test]
    async fn a_non_json_content_type_is_rejected() {
        let resp = serve_once(
            "HTTP/1.1 200 OK",
            "Content-Type: text/html\r\n",
            "<html></html>",
        )
        .await;
        let result: Result<serde_json::Value, BoundsError> = read_bounded_json(resp).await;
        assert!(matches!(result, Err(BoundsError::WrongContentType(_))));
    }

    #[tokio::test]
    async fn a_json_plus_suffix_content_type_is_accepted() {
        let resp = serve_once(
            "HTTP/1.1 200 OK",
            "Content-Type: application/trust+json\r\n",
            r#"{"a":"x"}"#,
        )
        .await;
        let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn a_missing_content_type_is_rejected() {
        let resp = serve_once("HTTP/1.1 200 OK", "", r#"{"a":"x"}"#).await;
        let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
        assert!(matches!(result, Err(BoundsError::MissingContentType)));
    }

    #[tokio::test]
    async fn a_non_200_status_is_rejected() {
        let resp = serve_once(
            "HTTP/1.1 201 Created",
            "Content-Type: application/json\r\n",
            r#"{"a":"x"}"#,
        )
        .await;
        let result: Result<Doc, BoundsError> = read_bounded_json(resp).await;
        assert!(matches!(result, Err(BoundsError::WrongStatus(_))));
    }

    #[test]
    fn depth_is_bounded_by_serde_json_itself() {
        // Pinning the library's own guarantee (RFC 134 D5 Tier 1's "pin
        // the floor, record what is relied on" for the two caps that
        // already hold): a 200-deep array -- past serde_json's default
        // 128 -- is rejected.
        let ok: Result<serde_json::Value, _> = serde_json::from_str(&deep_array_json(100));
        assert!(ok.is_ok(), "100 is within the default depth limit");
        let err: Result<serde_json::Value, _> = serde_json::from_str(&deep_array_json(200));
        assert!(err.is_err(), "200 exceeds the default depth limit");
    }

    #[test]
    fn duplicate_declared_key_is_rejected_by_serde_itself() {
        let dup = r#"{"a":"x","b":"y","a":"z"}"#;
        let result: Result<Doc, _> = serde_json::from_str(dup);
        assert!(
            result.is_err(),
            "a duplicate declared field must be rejected"
        );
    }

    #[test]
    fn duplicate_unknown_key_is_not_caught_deliberately_not_deny_unknown_fields() {
        // The one limit RFC 134 D5 Tier 1 states rather than discovers
        // later: duplicate *unknown* keys are not caught, because serde
        // skips unknown fields without duplicate detection.
        // `deny_unknown_fields` is deliberately not added -- it would
        // reject real providers' legitimate extra members.
        let dup_unknown = r#"{"a":"x","unknown":"first","unknown":"second"}"#;
        let result: Result<Doc, _> = serde_json::from_str(dup_unknown);
        assert!(
            result.is_ok(),
            "an unknown field appearing twice is not a rejection -- Doc has no \
             deny_unknown_fields, and this must stay true"
        );
    }

    #[test]
    fn too_many_members_is_rejected() {
        let mut obj = serde_json::Map::new();
        for i in 0..(MAX_MEMBERS + 1) {
            obj.insert(format!("k{i}"), serde_json::Value::Bool(true));
        }
        let err = check_caps(&serde_json::Value::Object(obj)).expect_err("must reject");
        assert!(matches!(err, BoundsError::TooManyMembers { .. }));
    }

    #[test]
    fn exactly_the_member_limit_is_accepted() {
        let mut obj = serde_json::Map::new();
        for i in 0..MAX_MEMBERS {
            obj.insert(format!("k{i}"), serde_json::Value::Bool(true));
        }
        assert!(check_caps(&serde_json::Value::Object(obj)).is_ok());
    }

    #[test]
    fn too_long_an_array_is_rejected() {
        let arr = vec![serde_json::Value::Bool(true); MAX_ARRAY_LEN + 1];
        let err = check_caps(&serde_json::Value::Array(arr)).expect_err("must reject");
        assert!(matches!(err, BoundsError::ArrayTooLong { .. }));
    }

    #[test]
    fn exactly_the_array_limit_is_accepted() {
        let arr = vec![serde_json::Value::Bool(true); MAX_ARRAY_LEN];
        assert!(check_caps(&serde_json::Value::Array(arr)).is_ok());
    }

    #[test]
    fn too_long_a_string_is_rejected() {
        let s = "x".repeat(MAX_STRING_LEN + 1);
        let err = check_caps(&serde_json::Value::String(s)).expect_err("must reject");
        assert!(matches!(err, BoundsError::StringTooLong { .. }));
    }

    #[test]
    fn exactly_the_string_limit_is_accepted() {
        let s = "x".repeat(MAX_STRING_LEN);
        assert!(check_caps(&serde_json::Value::String(s)).is_ok());
    }

    #[tokio::test]
    async fn a_body_with_no_content_length_is_still_capped_on_bytes_read() {
        // RFC 134 D5 Tier 1 S4b: "A Content-Length pre-check is necessary
        // but not sufficient -- it is attacker-supplied and may be
        // absent or lie." This is the absent case: chunked
        // transfer-encoding, no Content-Length header at all, streaming
        // past the cap. A pre-check alone would have nothing to check
        // against and would have to trust the server; the cap here
        // holds on bytes actually received instead.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut discard = [0u8; 1024];
            let _ = stream.read(&mut discard).await;
            let total = MAX_RESPONSE_BYTES + 1000;
            let chunk_body = "x".repeat(total);
            let framed = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{chunk_body}\r\n0\r\n\r\n",
                chunk_body.len(),
            );
            let _ = stream.write_all(framed.as_bytes()).await;
            let _ = stream.shutdown().await;
        });

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("connect");
        assert!(
            resp.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .is_none(),
            "the server must not have declared a length -- this is the absent case, not the lying one"
        );
        let result: Result<serde_json::Value, BoundsError> = read_bounded_json(resp).await;
        assert!(
            matches!(result, Err(BoundsError::TooLarge { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn a_long_string_nested_inside_an_object_is_still_caught() {
        // The caps apply at every depth, not only the top level.
        let mut obj = serde_json::Map::new();
        obj.insert(
            "nested".into(),
            serde_json::Value::Array(vec![serde_json::Value::String(
                "x".repeat(MAX_STRING_LEN + 1),
            )]),
        );
        let err = check_caps(&serde_json::Value::Object(obj)).expect_err("must reject");
        assert!(matches!(err, BoundsError::StringTooLong { .. }));
    }
}
