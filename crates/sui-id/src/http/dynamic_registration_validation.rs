//! RFC 095 M3 stage 1 — the request envelope and the redirect profile.
//!
//! Two independent pieces, per the stage's own split:
//!
//! - [`parse_envelope`]: the raw JSON body of `POST /oauth2/register`,
//!   bounded and checked *before* it becomes a typed
//!   [`crate::http::handlers::dynamic_register::RegistrationRequest`].
//! - [`derive_closed_profile`]: `redirect_uris` classified into exactly one
//!   of [`RedirectProfile`]'s three variants, or rejected.
//!
//! Neither of these touches `sui_id_core::admin::clients::validate_redirect_uri`.
//! That function is called from five administrator-client sites and its own
//! tests assert `http://localhost:8080/cb` is accepted — a case this
//! module's [`RedirectProfile::PublicNativeLoopback`] explicitly rejects
//! (`localhost` is a name, not a numeric literal). Tightening the shared
//! function would change administrator-created client behaviour, which
//! RFC 095 does not govern; this module is the validator written beside
//! it, for dynamic registration only.
//!
//! ## What this does not validate
//!
//! The matrix's "Supported fields", "Name and scope corpus",
//! "Authentication/grant matrix", "Browser presentation" and
//! "Response/error assertions" sections are separate, not-yet-dispatched
//! stages. In particular `post_logout_redirect_uris`' own full policy
//! (1-16 canonical unique HTTPS) is not built here — only the one
//! profile-interaction rule the Redirect corpus names explicitly: no
//! `post_logout_redirect_uris` entry may be `http`, including numeric
//! loopback, for any dynamic profile (see [`reject_http_post_logout_uris`]).

use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use std::collections::HashSet;
use std::fmt;

// ── 1b: the envelope ─────────────────────────────────────────────────────────

/// Observed-nowhere-near-this bound, but stated and enforced rather than
/// left to `axum`'s own default: RFC 095's own matrix sets 64 KiB for the
/// registration envelope, distinct from (and not interchangeable with)
/// RFC 134 D5's 64 KiB response bound — same number, different corpus,
/// different justification, so it gets its own named constant rather
/// than reusing `response_bounds::MAX_RESPONSE_BYTES`.
pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

/// `serde_json`'s own `remaining_depth` default is 128 (RFC 134 D5 already
/// evidenced this for the outbound-response path) — eight times looser
/// than this envelope's own 16. Depth beyond the top level only occurs in
/// an unknown extension member (every supported field is a scalar or an
/// array of scalars), so this bound mainly guards against someone using
/// an extension member as a nesting-depth attack surface.
pub const MAX_ENVELOPE_DEPTH: usize = 16;

/// Top-level member count, per the matrix. Checked on the object actually
/// received, not on any single known field's own cardinality (e.g.
/// `redirect_uris`' own 1-16 is a different, "Supported fields" bound,
/// out of this stage's scope).
pub const MAX_ENVELOPE_MEMBERS: usize = 128;

#[derive(Debug)]
pub enum EnvelopeError {
    TooLarge,
    NotAnObject,
    Malformed(serde_json::Error),
    DuplicateMember(String),
    TooDeep,
    TooManyMembers,
    UnapprovedSoftwareStatement,
    UnsupportedMember(String),
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "request body exceeds {MAX_ENVELOPE_BYTES} bytes"),
            Self::NotAnObject => write!(f, "request body must be a JSON object"),
            Self::Malformed(e) => write!(f, "malformed JSON: {e}"),
            Self::DuplicateMember(k) => write!(f, "duplicate member: {k}"),
            Self::TooDeep => write!(f, "request body exceeds depth {MAX_ENVELOPE_DEPTH}"),
            Self::TooManyMembers => write!(
                f,
                "request body exceeds {MAX_ENVELOPE_MEMBERS} top-level members"
            ),
            Self::UnapprovedSoftwareStatement => write!(f, "software_statement is not approved"),
            Self::UnsupportedMember(k) => write!(f, "unsupported client metadata member: {k}"),
        }
    }
}

/// Members this deployment recognises as real RFC 7591 / OIDC Dynamic
/// Client Registration 1.0 metadata but does not support. Distinct from
/// an unknown extension member (ignored, not persisted or echoed) and from
/// `software_statement` (its own error code, checked first). Sourced from
/// RFC 7591 §2 and OpenID Connect Dynamic Client Registration 1.0 §2 — this
/// is the contract this deployment currently honours; a member legitimately
/// added to a future stage moves out of this list, not just gets accepted
/// silently.
const KNOWN_UNSUPPORTED_MEMBERS: &[&str] = &[
    // RFC 7591 §2.
    "response_types",
    "client_id",
    "client_secret",
    "jwks_uri",
    "jwks",
    "software_id",
    "software_version",
    "contacts",
    // OIDC Dynamic Client Registration 1.0 §2, beyond RFC 7591.
    "application_type",
    "sector_identifier_uri",
    "subject_type",
    "id_token_signed_response_alg",
    "id_token_encrypted_response_alg",
    "id_token_encrypted_response_enc",
    "userinfo_signed_response_alg",
    "userinfo_encrypted_response_alg",
    "userinfo_encrypted_response_enc",
    "request_object_signing_alg",
    "request_object_encryption_alg",
    "request_object_encryption_enc",
    "token_endpoint_auth_signing_alg",
    "default_max_age",
    "require_auth_time",
    "default_acr_values",
    "initiate_login_uri",
    "request_uris",
];

/// Members this deployment's `RegistrationRequest` actually deserialises.
/// Kept beside [`KNOWN_UNSUPPORTED_MEMBERS`] so the two lists are read
/// together, not maintained in two different files.
const SUPPORTED_MEMBERS: &[&str] = &[
    "redirect_uris",
    "client_name",
    "scope",
    "grant_types",
    "token_endpoint_auth_method",
    "logo_uri",
    "client_uri",
    "policy_uri",
    "tos_uri",
    "post_logout_redirect_uris",
];

/// A `serde_json::Map` whose top-level keys are guaranteed duplicate-free.
///
/// Plain `serde_json::Value` deserialisation does not raise an error on a
/// duplicate object key — later entries silently overwrite earlier ones via
/// `Map::insert`, the same last-key-wins behaviour that affects any
/// `HashMap`/`BTreeMap`-backed `Deserialize`. Only a derived struct's
/// `Deserialize` catches a *declared* duplicate field (RFC 134 D5 already
/// evidenced this). Catching an *unknown* duplicate key as well — which
/// the matrix's unqualified "duplicate member" requires — needs this
/// manual `Visitor`, which inspects each key as it streams past rather
/// than after the fact.
struct DuplicateCheckedObject(serde_json::Map<String, serde_json::Value>);

impl<'de> Deserialize<'de> for DuplicateCheckedObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = DuplicateCheckedObject;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut out = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!("duplicate member: {key}")));
                    }
                    let value: serde_json::Value = map.next_value()?;
                    out.insert(key, value);
                }
                Ok(DuplicateCheckedObject(out))
            }
        }
        deserializer.deserialize_map(V)
    }
}

/// Recursive depth, counting the object/array containing a scalar as
/// depth 1 (an empty top-level object is depth 1, not 0) — matches
/// `response_bounds::check_caps`'s own convention for the same reason:
/// a bound of 1 should mean "scalars only", not "nothing at all".
fn depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => 1 + map.values().map(depth).max().unwrap_or(0),
        serde_json::Value::Array(items) => 1 + items.iter().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// 1b: parse and bound the raw registration request body. Returns the
/// validated, duplicate-free top-level object for the caller to convert
/// into its typed `RegistrationRequest` (via `serde_json::from_value`,
/// which naturally produces "wrong JSON scalar/array type ->
/// invalid_client_metadata" through its own type errors, and naturally
/// ignores unknown extension members since `RegistrationRequest` has no
/// `deny_unknown_fields`).
pub fn parse_envelope(
    bytes: &[u8],
) -> Result<serde_json::Map<String, serde_json::Value>, EnvelopeError> {
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(EnvelopeError::TooLarge);
    }
    let checked: DuplicateCheckedObject = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            // `DuplicateCheckedObject`'s own `visit_map` raises its
            // duplicate-key rejection via `serde::de::Error::custom`,
            // which `serde_json` classifies the same as any other "data"
            // error (a type mismatch, say) -- `is_data()` alone cannot
            // tell them apart, so the message (which this module itself
            // produced, not attacker-controlled parser output) is
            // checked first.
            let msg = e.to_string();
            if let Some(rest) = msg.strip_prefix("duplicate member: ") {
                // `serde_json` appends " at line L column C" to any custom
                // message raised from inside a `Deserialize` impl; strip
                // it back off to recover exactly the key this module put
                // in, not a string that merely starts with it.
                let key = rest.split(" at line ").next().unwrap_or(rest);
                return Err(EnvelopeError::DuplicateMember(key.to_owned()));
            }
            // A non-object top-level value (array, string, number, bool,
            // null) fails `deserialize_map` with a type-mismatch message;
            // genuinely malformed JSON fails earlier, during lexing. Both
            // are real rejections either way; distinguishing them in the
            // public error is not required by the matrix (both take
            // `invalid_client_metadata`), but the distinction is kept
            // internally since "not an object" and "not JSON" are
            // different facts a server log may care about.
            return if e.is_data() {
                Err(EnvelopeError::NotAnObject)
            } else {
                Err(EnvelopeError::Malformed(e))
            };
        }
    };
    let map = checked.0;

    if map.len() > MAX_ENVELOPE_MEMBERS {
        return Err(EnvelopeError::TooManyMembers);
    }
    let max_depth = map.values().map(depth).max().unwrap_or(0) + 1;
    if max_depth > MAX_ENVELOPE_DEPTH {
        return Err(EnvelopeError::TooDeep);
    }

    if map.contains_key("software_statement") {
        return Err(EnvelopeError::UnapprovedSoftwareStatement);
    }
    for key in map.keys() {
        if KNOWN_UNSUPPORTED_MEMBERS.contains(&key.as_str()) {
            return Err(EnvelopeError::UnsupportedMember(key.clone()));
        }
    }
    debug_assert!(
        SUPPORTED_MEMBERS
            .iter()
            .all(|m| !KNOWN_UNSUPPORTED_MEMBERS.contains(m)),
        "a member cannot be both supported and known-unsupported"
    );

    Ok(map)
}

/// The bearer token presented to `POST /oauth2/register`, once confirmed to
/// be exactly one syntactically valid token. Still needs to be checked
/// against `client_registration_token` — this only rules out the
/// presentation-shape rejections the matrix lists (missing, oversized,
/// non-hex, uppercase, multiple, wrong scheme), so that whatever check
/// comes next can distinguish "no plausible token was even presented" from
/// "a real-looking token was presented and turned out invalid" without the
/// caller having to re-derive this logic.
pub fn extract_registration_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let mut values = headers.get_all(axum::http::header::AUTHORIZATION).iter();
    let only = values.next()?;
    if values.next().is_some() {
        // More than one Authorization header: the matrix's "multiple"
        // rejection. Take the same path as "none presented" rather than
        // picking one arbitrarily.
        return None;
    }
    let raw = only.to_str().ok()?;
    let token = raw.strip_prefix("Bearer ")?;
    if token.len() != 64 {
        return None;
    }
    if !token
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    Some(token.to_owned())
}

// ── 1a: the derived closed profile ───────────────────────────────────────────

/// The three dynamic-registration redirect profiles RFC 095 defines. A
/// registration request derives exactly one from its
/// `token_endpoint_auth_method` and its `redirect_uris`; every redirect in
/// the list must belong to the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectProfile {
    /// `none` + every redirect HTTPS. PKCE required.
    PublicHttps,
    /// `none` + every redirect a numeric (not named) HTTP loopback address,
    /// with an explicit nonzero port. PKCE required. Request-time matching
    /// is port-flexible for this profile only (RFC 8252's native-app
    /// pattern — the OS assigns the loopback port at launch, not at
    /// registration).
    PublicNativeLoopback,
    /// `client_secret_basic` or `client_secret_post` + every redirect
    /// HTTPS.
    ConfidentialHttps,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RedirectRejection {
    /// A specific URI failed a profile-independent well-formedness check.
    /// Carries the offending URI and a human-readable reason, both safe to
    /// surface (the public error path never echoes request content beyond
    /// what the matrix allows for `invalid_client_metadata`-class errors —
    /// callers choose how much of this to expose).
    Malformed {
        uri: String,
        reason: &'static str,
    },
    /// The list mixed HTTPS and numeric-loopback-HTTP entries. The profile
    /// is closed; this cannot be split or guessed at.
    MixedProfile,
    /// A confidential auth method with any HTTP loopback redirect. Only
    /// `none` may ever use loopback.
    ConfidentialWithLoopback,
    TooManyRedirects,
    DuplicateRedirect(String),
}

impl fmt::Display for RedirectRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { uri, reason } => write!(f, "redirect_uris: {uri}: {reason}"),
            Self::MixedProfile => write!(
                f,
                "redirect_uris: all entries must share one profile (HTTPS or numeric loopback), \
                 not a mix"
            ),
            Self::ConfidentialWithLoopback => write!(
                f,
                "redirect_uris: an HTTP loopback redirect is only permitted with \
                 token_endpoint_auth_method=none"
            ),
            Self::TooManyRedirects => write!(f, "redirect_uris: at most 16 entries"),
            Self::DuplicateRedirect(u) => write!(f, "redirect_uris: duplicate entry: {u}"),
        }
    }
}

const MAX_REDIRECT_URIS: usize = 16;
const MAX_REDIRECT_URI_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Https,
    NumericLoopbackHttp,
}

/// Profile-independent well-formedness, checked on every redirect
/// regardless of which profile it will end up in. Returns the URI's shape
/// on success.
///
/// Checks are ordered to fail on the cheapest, most obviously-wrong signal
/// first (raw-string scans) before the more expensive `url::Url::parse`
/// call, but every check here was individually measured against
/// `url::Url` 2.5's real behaviour rather than assumed from its docs — see
/// the module's test corpus, which pins each one:
///
/// - **ASCII-only.** `url::Url::parse` IDNA-normalises a Unicode hostname
///   to ASCII punycode transparently — `Url::parse("https://例え.jp/cb")`
///   and `Url::parse("https://xn--r8jz45g.jp/cb")` produce an *identical*
///   `host_str()`. Checking the parsed host cannot distinguish "the
///   caller already sent canonical punycode" from "the caller sent
///   Unicode and the library silently fixed it" — only the *raw* input
///   can, so this checks `uri.is_ascii()` before parsing.
/// - **No backslash.** `Url::parse("https://rp.example\\evil.com/cb")`
///   parses successfully with host `rp.example` (the backslash and
///   everything after becomes path, not host) — not an error, and not
///   obviously wrong by host alone. Rejected as malformed on the raw
///   string instead of relying on where the library happens to place it.
/// - **No malformed percent escape.** `Url::parse("https://rp.example/cb%")`
///   parses successfully with path `/cb%` — a lone trailing `%` is not an
///   error. Checked on the raw string: every `%` must be followed by
///   exactly two hex digits.
/// - **No explicit default port.** `Url::parse("https://rp.example:443/cb")`
///   and `Url::parse("https://rp.example/cb")` produce *byte-identical*
///   `as_str()` output — the library silently strips a redundant default
///   port during parsing, which would make rejecting it after parsing
///   impossible. Checked on the raw string's authority section instead.
fn well_formed_shape(uri: &str) -> Result<Shape, RedirectRejection> {
    let reject = |reason: &'static str| {
        Err(RedirectRejection::Malformed {
            uri: uri.to_owned(),
            reason,
        })
    };
    if uri.len() > MAX_REDIRECT_URI_BYTES {
        return reject("exceeds 2048 bytes");
    }
    if !uri.is_ascii() {
        return reject("non-ASCII; submit canonical ASCII punycode, not a Unicode hostname");
    }
    if uri.contains('\\') {
        return reject("contains a backslash");
    }
    if uri.contains('*') {
        return reject("contains a wildcard/pattern character");
    }
    if has_malformed_percent_escape(uri) {
        return reject("malformed percent escape");
    }
    if has_explicit_default_port(uri) {
        return reject("explicit default port (443 for https, 80 for http) is redundant");
    }

    let parsed = url::Url::parse(uri).map_err(|_| RedirectRejection::Malformed {
        uri: uri.to_owned(),
        reason: "not a valid URL",
    })?;
    if parsed.fragment().is_some() {
        return reject("must not contain a fragment");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return reject("must not contain userinfo");
    }

    match parsed.scheme() {
        "https" => Ok(Shape::Https),
        "http" => match parsed.host() {
            Some(url::Host::Ipv4(ip)) if ip.is_loopback() => match parsed.port() {
                None | Some(0) => reject("numeric loopback requires an explicit, nonzero port"),
                Some(_) => Ok(Shape::NumericLoopbackHttp),
            },
            Some(url::Host::Ipv6(ip)) if ip.is_loopback() => match parsed.port() {
                None | Some(0) => reject("numeric loopback requires an explicit, nonzero port"),
                Some(_) => Ok(Shape::NumericLoopbackHttp),
            },
            _ => reject(
                "http is only permitted on a numeric loopback address \
                 (127.0.0.1 or [::1], not a name such as localhost, \
                 and not a remote or private host)",
            ),
        },
        _ => reject("scheme must be https, or http on numeric loopback"),
    }
}

fn has_malformed_percent_escape(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let rest = &bytes[i + 1..];
            if rest.len() < 2 || !rest[0].is_ascii_hexdigit() || !rest[1].is_ascii_hexdigit() {
                return true;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    false
}

/// Detects `:443` right after the host in an `https` URI, or `:80` after
/// the host in an `http` URI — the one normalisation `url::Url::parse`
/// performs that this module must catch *before* parsing (see
/// [`well_formed_shape`]'s doc comment). A minimal authority-section scan,
/// not a full URL parse: good enough to catch the redundant-port case
/// without duplicating `url::Url`'s own parser.
fn has_explicit_default_port(uri: &str) -> bool {
    let (scheme, default_port, rest) = if let Some(r) = uri.strip_prefix("https://") {
        ("https", ":443", r)
    } else if let Some(r) = uri.strip_prefix("http://") {
        ("http", ":80", r)
    } else {
        return false;
    };
    let _ = scheme;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    // A bracketed IPv6 literal's own colons must not be mistaken for a
    // port separator; only a `:` after the closing `]` (or anywhere, for
    // a non-bracketed host) followed by the default port and then the end
    // of the authority counts.
    let host_end = authority.rfind(']').map(|i| i + 1).unwrap_or(0);
    authority[host_end..].ends_with(default_port)
}

/// 1a: derive the one closed profile a registration request's redirect
/// list belongs to, or reject it. `auth_method` is the raw
/// `token_endpoint_auth_method` string from the request (`"none"` for
/// public; anything else, including absent, is confidential — the same
/// signal `dynamic_register.rs` already uses to decide `confidential`, so
/// this does not introduce a second source of truth for it).
#[allow(clippy::expect_used)]
pub fn derive_closed_profile(
    auth_method: &str,
    redirect_uris: &[String],
) -> Result<RedirectProfile, RedirectRejection> {
    if redirect_uris.len() > MAX_REDIRECT_URIS {
        return Err(RedirectRejection::TooManyRedirects);
    }
    let mut seen = HashSet::with_capacity(redirect_uris.len());
    for uri in redirect_uris {
        if !seen.insert(uri.as_str()) {
            return Err(RedirectRejection::DuplicateRedirect(uri.clone()));
        }
    }

    let mut shapes = redirect_uris.iter().map(|u| well_formed_shape(u));
    let first = shapes.next().expect("redirect_uris is non-empty by the time this runs -- the caller's own empty-list guard (dynamic_register.rs) already rejected that case")?;
    for shape in shapes {
        if shape? != first {
            return Err(RedirectRejection::MixedProfile);
        }
    }

    let is_public = auth_method == "none";
    match (is_public, first) {
        (true, Shape::Https) => Ok(RedirectProfile::PublicHttps),
        (true, Shape::NumericLoopbackHttp) => Ok(RedirectProfile::PublicNativeLoopback),
        (false, Shape::Https) => Ok(RedirectProfile::ConfidentialHttps),
        (false, Shape::NumericLoopbackHttp) => Err(RedirectRejection::ConfidentialWithLoopback),
    }
}

/// Redirect corpus row: "Any HTTP post-logout URI, including numeric
/// loopback, is rejected for every dynamic profile" — the one
/// profile-interaction rule for `post_logout_redirect_uris` this stage
/// builds. The field's own full policy (count, uniqueness, canonical form)
/// is a separate, not-yet-dispatched stage; this only rejects `http`.
pub fn reject_http_post_logout_uris(uris: &[String]) -> Result<(), String> {
    for uri in uris {
        match url::Url::parse(uri) {
            Ok(parsed) if parsed.scheme() == "https" => {}
            _ => return Err(uri.clone()),
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── 1b: the envelope ─────────────────────────────────────────────────────

    fn obj(json: &str) -> &[u8] {
        json.as_bytes()
    }

    #[test]
    fn a_well_formed_object_is_accepted() {
        let map = parse_envelope(obj(r#"{"redirect_uris":["https://rp.example/cb"]}"#)).unwrap();
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn too_large_a_body_is_rejected() {
        let padding = "x".repeat(MAX_ENVELOPE_BYTES + 1);
        let body = format!(r#"{{"client_name":"{padding}"}}"#);
        assert!(matches!(
            parse_envelope(body.as_bytes()),
            Err(EnvelopeError::TooLarge)
        ));
    }

    #[test]
    fn exactly_the_byte_limit_is_not_rejected_for_being_large() {
        // Pad to exactly the limit; must not trip `TooLarge` (it may still
        // fail on member count/depth if it did, but it does not here).
        let overhead = r#"{"client_name":""}"#.len();
        let padding = "x".repeat(MAX_ENVELOPE_BYTES - overhead);
        let body = format!(r#"{{"client_name":"{padding}"}}"#);
        assert_eq!(body.len(), MAX_ENVELOPE_BYTES);
        assert!(!matches!(
            parse_envelope(body.as_bytes()),
            Err(EnvelopeError::TooLarge)
        ));
    }

    #[test]
    fn a_top_level_array_is_rejected_as_not_an_object() {
        assert!(matches!(
            parse_envelope(obj(r#"[1,2,3]"#)),
            Err(EnvelopeError::NotAnObject)
        ));
    }

    #[test]
    fn a_top_level_string_is_rejected_as_not_an_object() {
        assert!(matches!(
            parse_envelope(obj(r#""hello""#)),
            Err(EnvelopeError::NotAnObject)
        ));
    }

    #[test]
    fn malformed_json_is_rejected() {
        assert!(matches!(
            parse_envelope(obj(r#"{"redirect_uris": ["#)),
            Err(EnvelopeError::Malformed(_))
        ));
    }

    #[test]
    fn a_duplicate_declared_member_is_rejected() {
        let err = parse_envelope(obj(
            r#"{"client_name":"a","client_name":"b","redirect_uris":[]}"#,
        ))
        .unwrap_err();
        assert!(matches!(err, EnvelopeError::DuplicateMember(ref k) if k == "client_name"));
    }

    /// The matrix says "duplicate member", unqualified -- not just a
    /// duplicate of a field this deployment happens to model. A plain
    /// `serde_json::Value` parse would silently last-key-win this; this
    /// is exactly what [`DuplicateCheckedObject`] exists to catch instead.
    #[test]
    fn a_duplicate_unknown_member_is_rejected_not_silently_overwritten() {
        let err = parse_envelope(obj(
            r#"{"totally_unknown_field":"a","totally_unknown_field":"b"}"#,
        ))
        .unwrap_err();
        assert!(
            matches!(err, EnvelopeError::DuplicateMember(ref k) if k == "totally_unknown_field")
        );
    }

    #[test]
    fn too_many_top_level_members_is_rejected() {
        let members: String = (0..MAX_ENVELOPE_MEMBERS + 1)
            .map(|i| format!(r#""k{i}":true"#))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!("{{{members}}}");
        assert!(matches!(
            parse_envelope(body.as_bytes()),
            Err(EnvelopeError::TooManyMembers)
        ));
    }

    #[test]
    fn exactly_the_member_limit_is_accepted() {
        let members: String = (0..MAX_ENVELOPE_MEMBERS)
            .map(|i| format!(r#""k{i}":true"#))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!("{{{members}}}");
        assert!(parse_envelope(body.as_bytes()).is_ok());
    }

    fn nested(depth: usize) -> String {
        let mut s = String::from("1");
        for _ in 0..depth {
            s = format!("[{s}]");
        }
        format!(r#"{{"k":{s}}}"#)
    }

    #[test]
    fn too_deep_a_body_is_rejected() {
        // The outer object is depth 1; `k`'s value nested `MAX_ENVELOPE_DEPTH`
        // arrays deep makes the whole document `MAX_ENVELOPE_DEPTH + 1`.
        let body = nested(MAX_ENVELOPE_DEPTH);
        assert!(matches!(
            parse_envelope(body.as_bytes()),
            Err(EnvelopeError::TooDeep)
        ));
    }

    #[test]
    fn exactly_the_depth_limit_is_accepted() {
        let body = nested(MAX_ENVELOPE_DEPTH - 1);
        assert!(parse_envelope(body.as_bytes()).is_ok());
    }

    #[test]
    fn software_statement_is_rejected_with_its_own_error_not_invalid_client_metadata() {
        assert!(matches!(
            parse_envelope(obj(r#"{"software_statement":"ignored.jwt.value"}"#)),
            Err(EnvelopeError::UnapprovedSoftwareStatement)
        ));
    }

    #[test]
    fn a_known_unsupported_member_is_rejected_by_name() {
        let err = parse_envelope(obj(r#"{"jwks_uri":"https://rp.example/jwks"}"#)).unwrap_err();
        assert!(matches!(err, EnvelopeError::UnsupportedMember(ref k) if k == "jwks_uri"));
    }

    #[test]
    fn every_supported_member_is_accepted_by_name() {
        for member in SUPPORTED_MEMBERS {
            let body = format!(r#"{{"{member}":null}}"#);
            let err = parse_envelope(body.as_bytes());
            assert!(
                !matches!(err, Err(EnvelopeError::UnsupportedMember(_))),
                "{member} must not be treated as unsupported"
            );
        }
    }

    #[test]
    fn an_unknown_extension_member_is_ignored_not_rejected() {
        let map = parse_envelope(obj(
            r#"{"redirect_uris":["https://rp.example/cb"],"x_totally_custom":"whatever"}"#,
        ))
        .unwrap();
        // `parse_envelope` itself does not drop it -- that happens at the
        // typed-struct conversion step, which simply never reads it. Prove
        // that conversion step's own behaviour here with a tiny local
        // struct, independent of the real `RegistrationRequest` (which
        // lives in the handler module, deliberately not depended on from
        // here).
        #[derive(serde::Deserialize)]
        struct Minimal {
            redirect_uris: Vec<String>,
        }
        let typed: Minimal = serde_json::from_value(serde_json::Value::Object(map)).unwrap();
        assert_eq!(typed.redirect_uris, vec!["https://rp.example/cb"]);
    }

    #[test]
    fn a_wrong_scalar_type_fails_at_the_typed_conversion_step() {
        #[derive(serde::Deserialize)]
        struct Minimal {
            #[allow(dead_code)]
            redirect_uris: Vec<String>,
        }
        let map = parse_envelope(obj(r#"{"redirect_uris": 42}"#)).unwrap();
        assert!(serde_json::from_value::<Minimal>(serde_json::Value::Object(map)).is_err());
    }

    // ── bearer token format ──────────────────────────────────────────────────

    fn headers_with(values: &[&str]) -> axum::http::HeaderMap {
        let mut h = axum::http::HeaderMap::new();
        for v in values {
            h.append(
                axum::http::header::AUTHORIZATION,
                axum::http::HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    const VALID_TOKEN: &str = "abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab";

    #[test]
    fn a_well_formed_bearer_token_is_extracted() {
        assert_eq!(
            VALID_TOKEN.len(),
            64,
            "test fixture must itself be 64 chars"
        );
        let h = headers_with(&[&format!("Bearer {VALID_TOKEN}")]);
        assert_eq!(
            extract_registration_bearer_token(&h),
            Some(VALID_TOKEN.to_string())
        );
    }

    #[test]
    fn a_missing_authorization_header_yields_none() {
        let h = headers_with(&[]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    #[test]
    fn an_oversized_token_is_rejected() {
        let too_long = format!("{VALID_TOKEN}0");
        let h = headers_with(&[&format!("Bearer {too_long}")]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    #[test]
    fn a_non_hex_token_is_rejected() {
        let not_hex = "g".repeat(64);
        let h = headers_with(&[&format!("Bearer {not_hex}")]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    #[test]
    fn an_uppercase_token_is_rejected() {
        let upper = VALID_TOKEN.to_uppercase();
        let h = headers_with(&[&format!("Bearer {upper}")]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    #[test]
    fn multiple_authorization_headers_are_rejected() {
        let h = headers_with(&[
            &format!("Bearer {VALID_TOKEN}"),
            &format!("Bearer {VALID_TOKEN}"),
        ]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    #[test]
    fn a_wrong_scheme_is_rejected() {
        let h = headers_with(&[&format!("Basic {VALID_TOKEN}")]);
        assert_eq!(extract_registration_bearer_token(&h), None);
    }

    // ── 1a: the derived closed profile ───────────────────────────────────────

    #[test]
    fn confidential_https_is_accepted() {
        let uris = vec!["https://rp.example/cb".to_string()];
        assert_eq!(
            derive_closed_profile("client_secret_post", &uris),
            Ok(RedirectProfile::ConfidentialHttps)
        );
        assert_eq!(
            derive_closed_profile("client_secret_basic", &uris),
            Ok(RedirectProfile::ConfidentialHttps)
        );
    }

    #[test]
    fn public_https_is_accepted() {
        let uris = vec!["https://rp.example/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicHttps)
        );
    }

    #[test]
    fn public_native_loopback_ipv4_is_accepted() {
        let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicNativeLoopback)
        );
    }

    #[test]
    fn public_native_loopback_ipv6_is_accepted() {
        let uris = vec!["http://[::1]:49152/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicNativeLoopback)
        );
    }

    #[test]
    fn confidential_with_any_http_loopback_is_rejected() {
        let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("client_secret_post", &uris),
            Err(RedirectRejection::ConfidentialWithLoopback)
        ));
    }

    #[test]
    fn mixed_https_and_loopback_is_rejected() {
        let uris = vec![
            "https://rp.example/cb".to_string(),
            "http://127.0.0.1:49152/cb".to_string(),
        ];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::MixedProfile)
        ));
    }

    #[test]
    fn named_localhost_is_rejected_not_treated_as_numeric_loopback() {
        let uris = vec!["http://localhost:49152/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn http_on_a_remote_or_private_host_is_rejected() {
        for host in ["http://rp.example/cb", "http://10.0.0.5:8080/cb"] {
            let uris = vec![host.to_string()];
            assert!(
                matches!(
                    derive_closed_profile("none", &uris),
                    Err(RedirectRejection::Malformed { .. })
                ),
                "{host} must be rejected"
            );
        }
    }

    #[test]
    fn loopback_without_an_explicit_port_is_rejected() {
        let uris = vec!["http://127.0.0.1/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn loopback_with_port_zero_is_rejected() {
        let uris = vec!["http://127.0.0.1:0/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn a_fragment_is_rejected() {
        let uris = vec!["https://rp.example/cb#frag".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn userinfo_is_rejected() {
        let uris = vec!["https://user:pass@rp.example/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn a_wildcard_pattern_is_rejected() {
        let uris = vec!["https://rp.example/*".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn a_custom_scheme_is_rejected() {
        for uri in [
            "file:///etc/passwd",
            "data:text/plain,hi",
            "javascript:alert(1)",
            "custom-scheme://foo/cb",
        ] {
            let uris = vec![uri.to_string()];
            assert!(
                matches!(
                    derive_closed_profile("none", &uris),
                    Err(RedirectRejection::Malformed { .. })
                ),
                "{uri} must be rejected"
            );
        }
    }

    #[test]
    fn a_unicode_hostname_is_rejected() {
        let uris = vec!["https://例え.jp/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn canonical_ascii_punycode_is_accepted() {
        let uris = vec!["https://xn--r8jz45g.jp/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicHttps)
        );
    }

    #[test]
    fn a_malformed_percent_escape_is_rejected() {
        let uris = vec!["https://rp.example/cb%".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn a_backslash_is_rejected() {
        let uris = vec!["https://rp.example\\evil.com/cb".to_string()];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn an_explicit_default_port_is_rejected() {
        for uri in ["https://rp.example:443/cb", "http://127.0.0.1:80/cb"] {
            let uris = vec![uri.to_string()];
            assert!(
                matches!(
                    derive_closed_profile("none", &uris),
                    Err(RedirectRejection::Malformed { .. })
                ),
                "{uri} must be rejected"
            );
        }
    }

    #[test]
    fn a_non_default_explicit_port_on_https_is_still_accepted() {
        // The default-port check must not become "reject any explicit port".
        let uris = vec!["https://rp.example:8443/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicHttps)
        );
    }

    #[test]
    fn an_exact_duplicate_in_one_list_is_rejected() {
        let uris = vec![
            "https://rp.example/cb".to_string(),
            "https://rp.example/cb".to_string(),
        ];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::DuplicateRedirect(_))
        ));
    }

    #[test]
    fn a_uri_over_2048_bytes_is_rejected() {
        let long_path = "a".repeat(MAX_REDIRECT_URI_BYTES);
        let uri = format!("https://rp.example/{long_path}");
        assert!(uri.len() > MAX_REDIRECT_URI_BYTES);
        let uris = vec![uri];
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::Malformed { .. })
        ));
    }

    #[test]
    fn a_list_over_16_entries_is_rejected() {
        let uris: Vec<String> = (0..MAX_REDIRECT_URIS + 1)
            .map(|i| format!("https://rp.example/cb{i}"))
            .collect();
        assert!(matches!(
            derive_closed_profile("none", &uris),
            Err(RedirectRejection::TooManyRedirects)
        ));
    }

    #[test]
    fn exactly_16_entries_is_accepted() {
        let uris: Vec<String> = (0..MAX_REDIRECT_URIS)
            .map(|i| format!("https://rp.example/cb{i}"))
            .collect();
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicHttps)
        );
    }

    /// Public-native authorization redirects plus an independent HTTPS
    /// logout URI: the authorization profile is unchanged by what
    /// `post_logout_redirect_uris` contains, since the two are validated
    /// independently.
    #[test]
    fn a_native_loopback_profile_is_unaffected_by_an_independent_https_logout_uri() {
        let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
        assert_eq!(
            derive_closed_profile("none", &uris),
            Ok(RedirectProfile::PublicNativeLoopback)
        );
        assert!(reject_http_post_logout_uris(&["https://rp.example/logout".to_string()]).is_ok());
    }

    #[test]
    fn any_http_post_logout_uri_is_rejected_including_numeric_loopback() {
        assert!(reject_http_post_logout_uris(&["http://rp.example/logout".to_string()]).is_err());
        assert!(
            reject_http_post_logout_uris(&["http://127.0.0.1:49152/logout".to_string()]).is_err()
        );
    }

    #[test]
    fn an_https_post_logout_uri_is_accepted() {
        assert!(reject_http_post_logout_uris(&["https://rp.example/logout".to_string()]).is_ok());
    }

    #[test]
    fn an_empty_post_logout_list_is_accepted() {
        assert!(reject_http_post_logout_uris(&[]).is_ok());
    }
}
