//! RFC 095 M3 stages 1 and 2 — the request envelope, the redirect profile,
//! and the field rules over everything else `POST /oauth2/register` accepts.
//!
//! Stage 1's two independent pieces, per that stage's own split:
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
//! Stage 2 adds one validator per *Supported fields* row:
//! [`validate_client_name`], [`validate_auth_method`],
//! [`validate_grant_types`], [`validate_response_types`],
//! [`validate_scope`] (plus [`check_grant_scope_coupling`], the
//! `refresh_token`/`offline_access` two-way rule), [`validate_application_uri`]
//! (the four `logo_uri`/`client_uri`/`policy_uri`/`tos_uri` fields), and
//! [`validate_post_logout_redirect_uris`] (replacing stage 1's narrower
//! `reject_http_post_logout_uris` entirely — see its own doc comment for
//! why nothing from that function survives unchanged).
//!
//! ## What this does not validate
//!
//! The matrix's "Authentication/grant matrix", "Browser presentation" and
//! "Response/error assertions" sections are separate, not-yet-dispatched
//! stages (4, 5, and part of 3). Stage 2 computes canonical/defaulted
//! values for every field it validates, but does not own the response's
//! echo-with-canonical-ordering policy (stage 3) — the handler echoes
//! what stage 2 returns without further ceremony.

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
    "response_types",
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
    /// Carries the offending URI and a human-readable reason, for internal
    /// logging. **RFC 095 M3 stage 3 decided the public response may not
    /// echo either** — `handlers/dynamic_register.rs`'s own sanitizing
    /// layer reports the entry's index in `redirect_uris` instead, never
    /// this struct's `uri`. This doc comment previously said the opposite
    /// ("both safe to surface"); that was stage 2's own open question,
    /// closed here.
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
///
/// Stage 2 factored the checks above this point in the list — everything
/// through "no userinfo" — into [`well_formed_uri`], shared with
/// [`validate_application_uri`] and [`validate_post_logout_redirect_uris`]:
/// those fields need exactly the same raw-string canonicality floor, scoped
/// to HTTPS only rather than this function's HTTPS-or-numeric-loopback
/// scheme branch below, which stays here because it is specific to the
/// redirect profile.
fn well_formed_shape(uri: &str) -> Result<Shape, RedirectRejection> {
    let reject = |reason: &'static str| {
        Err(RedirectRejection::Malformed {
            uri: uri.to_owned(),
            reason,
        })
    };
    let parsed = match well_formed_uri(uri, MAX_REDIRECT_URI_BYTES) {
        Ok(p) => p,
        Err(reason) => return reject(reason),
    };

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

/// The well-formedness floor every URI this module validates shares,
/// regardless of field: at most `max_bytes`, ASCII-only, no backslash, no
/// wildcard character, no malformed percent escape, no explicit default
/// port, parses as a URL, no fragment, no userinfo. See
/// [`well_formed_shape`]'s own doc comment for why each raw-string check
/// runs before `url::Url::parse` rather than trusting the parsed result —
/// that reasoning is identical here, not re-derived.
///
/// Returns the parsed URL so a caller can still inspect scheme and host —
/// this function takes no position on which schemes are acceptable, since
/// that differs per field (HTTPS-or-numeric-loopback for a redirect,
/// HTTPS-only for an application URI or a post-logout redirect).
fn well_formed_uri(uri: &str, max_bytes: usize) -> Result<url::Url, &'static str> {
    if uri.len() > max_bytes {
        return Err("exceeds the byte limit");
    }
    if !uri.is_ascii() {
        return Err("non-ASCII; submit canonical ASCII punycode, not a Unicode hostname");
    }
    if uri.contains('\\') {
        return Err("contains a backslash");
    }
    if uri.contains('*') {
        return Err("contains a wildcard/pattern character");
    }
    if has_malformed_percent_escape(uri) {
        return Err("malformed percent escape");
    }
    if has_explicit_default_port(uri) {
        return Err("explicit default port (443 for https, 80 for http) is redundant");
    }
    let parsed = url::Url::parse(uri).map_err(|_| "not a valid URL")?;
    if parsed.fragment().is_some() {
        return Err("must not contain a fragment");
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("must not contain userinfo");
    }
    Ok(parsed)
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

// ── 2: client_name ───────────────────────────────────────────────────────────

/// *Supported fields*: "trimmed 1–128 scalars, ≤512 bytes". Scalars for the
/// count (`char`, i.e. `str::chars().count()`), bytes for the size
/// (`str::len()`) — the matrix states these in two different units and this
/// module matches it rather than picking one definition for both. `"é"` is
/// one scalar and two bytes; the two bounds are independent for most
/// strings, and are *checked* in byte-then-scalar order below for a reason
/// that only matters at the extreme: a UTF-8 scalar is at most 4 bytes, so
/// 513 bytes is unreachable below 129 scalars — checking bytes first is
/// what lets the byte boundary's own rejecting-side test name
/// [`ClientNameRejection::TooManyBytes`] specifically, rather than always
/// surfacing [`ClientNameRejection::TooManyScalars`] for any input large
/// enough to reach either limit.
pub const MAX_CLIENT_NAME_SCALARS: usize = 128;
pub const MAX_CLIENT_NAME_BYTES: usize = 512;

#[derive(Debug, PartialEq, Eq)]
pub enum ClientNameRejection {
    /// Absent, or empty/whitespace-only after trimming.
    Blank,
    /// Contains a C0/C1 control character or a bidi explicit
    /// embedding/override/isolate character.
    ForbiddenCharacter,
    TooManyBytes,
    TooManyScalars,
}

impl fmt::Display for ClientNameRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => write!(f, "client_name is required"),
            Self::ForbiddenCharacter => write!(
                f,
                "client_name: must not contain a control or bidi override/isolate character"
            ),
            Self::TooManyBytes => {
                write!(f, "client_name: exceeds {MAX_CLIENT_NAME_BYTES} bytes")
            }
            Self::TooManyScalars => {
                write!(f, "client_name: exceeds {MAX_CLIENT_NAME_SCALARS} scalars")
            }
        }
    }
}

/// C0 (including DEL) or C1 control, or a bidi explicit
/// embedding/override (U+202A–U+202E) or isolate (U+2066–U+2069) control.
/// Everything else — combining marks, emoji, HTML angle brackets, quotes,
/// any other printable character — is accepted: the matrix's own corpus
/// note says why ("valid values are escaped by the UI and
/// bounded/sanitized before audit"), so this function's job is the
/// control-character floor, not content sanitisation that belongs at
/// render/log time instead.
fn is_forbidden_name_character(c: char) -> bool {
    let cp = u32::from(c);
    let c0_or_del = cp <= 0x1F || cp == 0x7F;
    let c1 = (0x80..=0x9F).contains(&cp);
    let bidi_control = (0x202A..=0x202E).contains(&cp) || (0x2066..=0x2069).contains(&cp);
    c0_or_del || c1 || bidi_control
}

/// *Supported fields* row 2: `client_name`. Trimmed first — the corpus's
/// leading/trailing-whitespace case is an accept case, not a reject one —
/// then checked blank, forbidden-character, byte, and scalar, in that
/// order (see [`MAX_CLIENT_NAME_BYTES`]'s own doc comment for why bytes
/// precedes scalars).
pub fn validate_client_name(raw: Option<&str>) -> Result<String, ClientNameRejection> {
    let trimmed = raw.unwrap_or("").trim();
    if trimmed.is_empty() {
        return Err(ClientNameRejection::Blank);
    }
    if trimmed.chars().any(is_forbidden_name_character) {
        return Err(ClientNameRejection::ForbiddenCharacter);
    }
    if trimmed.len() > MAX_CLIENT_NAME_BYTES {
        return Err(ClientNameRejection::TooManyBytes);
    }
    if trimmed.chars().count() > MAX_CLIENT_NAME_SCALARS {
        return Err(ClientNameRejection::TooManyScalars);
    }
    Ok(trimmed.to_owned())
}

// ── 2: token_endpoint_auth_method ────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidAuthMethod(String);

impl fmt::Display for InvalidAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "token_endpoint_auth_method: must be none, client_secret_basic, or \
             client_secret_post, not {:?}",
            self.0
        )
    }
}

/// *Supported fields* row 3. The matrix's own table cells read
/// "none/basic/post", but every other caller in this codebase — this
/// module's own [`RedirectProfile::ConfidentialHttps`] doc comment,
/// `discovery.rs`'s `token_endpoint_auth_methods_supported`,
/// `federation.rs`'s upstream auth method selection, and this very
/// handler's pre-stage-2 default — uses RFC 7591's actual wire values,
/// `"none"`, `"client_secret_basic"`, `"client_secret_post"`. "basic"/
/// "post" are nowhere else in this deployment's vocabulary and no real
/// client library sends them; implementing the table's shorthand literally
/// would reject every interoperable caller. Reported rather than silently
/// reinterpreted — see the stage's return package.
///
/// **Default is `client_secret_basic`, not `client_secret_post`.** RFC
/// 7591 §2 states this default explicitly, and the matrix agrees
/// ("default basic"). The handler's pre-stage-2 code defaulted to
/// `client_secret_post` instead — traced to no test asserting the
/// specific default string (only confidentiality, i.e. "a secret is
/// present", which either value satisfies) — so fixing it here changes no
/// currently-asserted behaviour.
pub fn validate_auth_method(raw: Option<&str>) -> Result<&'static str, InvalidAuthMethod> {
    match raw {
        None => Ok("client_secret_basic"),
        Some("none") => Ok("none"),
        Some("client_secret_basic") => Ok("client_secret_basic"),
        Some("client_secret_post") => Ok("client_secret_post"),
        Some(other) => Err(InvalidAuthMethod(other.to_owned())),
    }
}

// ── 2: grant_types ───────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum GrantTypesRejection {
    Empty,
    Duplicate(String),
    RefreshWithoutCode,
    OtherGrant(String),
}

impl fmt::Display for GrantTypesRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "grant_types: must not be an empty list"),
            Self::Duplicate(g) => write!(f, "grant_types: duplicate entry: {g}"),
            Self::RefreshWithoutCode => write!(
                f,
                "grant_types: refresh_token requires authorization_code in the same list"
            ),
            Self::OtherGrant(g) => write!(f, "grant_types: unsupported grant type: {g}"),
        }
    }
}

/// *Supported fields* row 4: `authorization_code` alone, or
/// `authorization_code` plus `refresh_token`; default `authorization_code`
/// alone. The handler's pre-stage-2 response default was
/// `["authorization_code", "refresh_token"]` — traced to no test asserting
/// `grant_types`' specific contents, so corrected to match the matrix's
/// "default code" (one grant, not two) rather than carried forward.
pub fn validate_grant_types(raw: Option<&[String]>) -> Result<Vec<String>, GrantTypesRejection> {
    let Some(grants) = raw else {
        return Ok(vec!["authorization_code".to_owned()]);
    };
    if grants.is_empty() {
        return Err(GrantTypesRejection::Empty);
    }
    let mut seen = HashSet::with_capacity(grants.len());
    for g in grants {
        if !seen.insert(g.as_str()) {
            return Err(GrantTypesRejection::Duplicate(g.clone()));
        }
        if g != "authorization_code" && g != "refresh_token" {
            return Err(GrantTypesRejection::OtherGrant(g.clone()));
        }
    }
    if !seen.contains("authorization_code") {
        return Err(GrantTypesRejection::RefreshWithoutCode);
    }
    Ok(grants.to_vec())
}

// ── 2: response_types ────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseTypesRejection {
    Empty,
    Duplicate(String),
    /// Any entry other than `code`, or more than one distinct entry —
    /// covers the matrix's "token", "hybrid", and "other" buckets
    /// identically: every one of them is "the set is not exactly `{code}`".
    NotCodeOnly(String),
}

impl fmt::Display for ResponseTypesRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "response_types: must not be an empty list"),
            Self::Duplicate(r) => write!(f, "response_types: duplicate entry: {r}"),
            Self::NotCodeOnly(r) => write!(
                f,
                "response_types: only the code response type is supported, not {r}"
            ),
        }
    }
}

/// *Supported fields* row 5: `code` only, default `code`. **Zero
/// implementation existed before this stage** — `response_types` was a
/// [`KNOWN_UNSUPPORTED_MEMBERS`] entry, rejected outright. Deliberately
/// not wired to `discovery.rs`'s `response_types_supported`: that field
/// describes the *provider's* overall capability for every client, while
/// this one is a *per-registration* request that this deployment currently
/// only ever accepts as `code` — conflating them would make a future,
/// wider discovery capability silently loosen what registration accepts
/// (or vice versa) with no code change announcing it.
pub fn validate_response_types(
    raw: Option<&[String]>,
) -> Result<Vec<String>, ResponseTypesRejection> {
    let Some(types) = raw else {
        return Ok(vec!["code".to_owned()]);
    };
    if types.is_empty() {
        return Err(ResponseTypesRejection::Empty);
    }
    let mut seen = HashSet::with_capacity(types.len());
    for t in types {
        if !seen.insert(t.as_str()) {
            return Err(ResponseTypesRejection::Duplicate(t.clone()));
        }
    }
    if seen.len() != 1 || !seen.contains("code") {
        let offending = types
            .iter()
            .find(|t| t.as_str() != "code")
            .unwrap_or(&types[0]);
        return Err(ResponseTypesRejection::NotCodeOnly(offending.clone()));
    }
    Ok(types.to_vec())
}

// ── 2: scope ─────────────────────────────────────────────────────────────────

/// RFC 6749 §3.3's own bound on a single `scope-token`'s character set:
/// `%x21 / %x23-5B / %x5D-7E` — every printable ASCII byte except space
/// (the token separator), `"` (`%x22`), and `\` (`%x5C`).
fn is_scope_token_byte(b: u8) -> bool {
    b == 0x21 || (0x23..=0x5B).contains(&b) || (0x5D..=0x7E).contains(&b)
}

pub const MAX_SCOPE_TOKENS: usize = 32;
pub const MAX_SCOPE_TOKEN_BYTES: usize = 64;
pub const MAX_SCOPE_TOTAL_BYTES: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ScopeRejection {
    Empty,
    /// A token used a byte outside RFC 6749 §3.3's scope-token character
    /// set, or the string had a leading/trailing/doubled separator space
    /// (an empty token between two spaces is the same fault: a token that
    /// is not 1 or more allowed characters).
    Malformed,
    TooManyBytes,
    TooManyTokens,
    /// RFC 095 M3 stage 3: carries the token's zero-based index, not its
    /// value — a public error may name which entry failed, not what it
    /// was. Internal logging still has the raw request to hand without
    /// this type carrying it.
    TokenTooLong(usize),
    /// The index of the *repeated* occurrence — deterministic (it is
    /// always the second, later one, never ambiguous between two equal
    /// entries) and, like the other variants here, an index rather than
    /// the value.
    Duplicate(usize),
    Unknown(usize),
    MissingOpenid,
}

impl fmt::Display for ScopeRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "scope: must not be empty"),
            Self::Malformed => write!(
                f,
                "scope: must be one or more space-separated RFC 6749 scope tokens"
            ),
            Self::TooManyBytes => {
                write!(f, "scope: exceeds {MAX_SCOPE_TOTAL_BYTES} bytes")
            }
            Self::TooManyTokens => {
                write!(f, "scope: exceeds {MAX_SCOPE_TOKENS} tokens")
            }
            Self::TokenTooLong(i) => {
                write!(
                    f,
                    "scope: token at index {i} exceeds {MAX_SCOPE_TOKEN_BYTES} bytes"
                )
            }
            Self::Duplicate(i) => write!(f, "scope: token at index {i} is a duplicate"),
            Self::Unknown(i) => {
                write!(
                    f,
                    "scope: token at index {i} is not in this deployment's catalog"
                )
            }
            Self::MissingOpenid => write!(f, "scope: must include openid"),
        }
    }
}

/// *Supported fields* row 6: 1–32 unique catalog tokens including `openid`.
///
/// `catalog` is one read of the whole `scope_definition` table, taken
/// once by the caller before this function runs — not re-queried per
/// token. That is this stage's answer to the corpus's "concurrent catalog
/// deletion/default change" case: validating all of a request's up-to-32
/// tokens against a single consistent snapshot closes the narrower race
/// (two tokens checked against two different states of the catalog mid
/// validation) that a per-token query would leave open. It does not, and
/// is not meant to, make this request's validation linearize with a
/// concurrent catalog edit — see the stage's return package for why that
/// wider race is accepted rather than closed.
///
/// Checks run cheapest-and-structural first, catalog membership last: a
/// request with a structural fault (empty, oversize, malformed character,
/// duplicate) never reaches the database at all.
pub fn validate_scope(
    raw: Option<&str>,
    catalog: &HashSet<String>,
) -> Result<Vec<String>, ScopeRejection> {
    let s = raw.unwrap_or("");
    if s.is_empty() {
        return Err(ScopeRejection::Empty);
    }
    if s.len() > MAX_SCOPE_TOTAL_BYTES {
        return Err(ScopeRejection::TooManyBytes);
    }
    let tokens: Vec<&str> = s.split(' ').collect();
    if tokens.len() > MAX_SCOPE_TOKENS {
        return Err(ScopeRejection::TooManyTokens);
    }
    let mut seen: HashSet<&str> = HashSet::with_capacity(tokens.len());
    for (i, token) in tokens.iter().enumerate() {
        if token.is_empty() || !token.bytes().all(is_scope_token_byte) {
            return Err(ScopeRejection::Malformed);
        }
        if token.len() > MAX_SCOPE_TOKEN_BYTES {
            return Err(ScopeRejection::TokenTooLong(i));
        }
        if !seen.insert(token) {
            return Err(ScopeRejection::Duplicate(i));
        }
    }
    for (i, token) in tokens.iter().enumerate() {
        if !catalog.contains(*token) {
            return Err(ScopeRejection::Unknown(i));
        }
    }
    if !seen.contains("openid") {
        return Err(ScopeRejection::MissingOpenid);
    }
    Ok(tokens.into_iter().map(str::to_owned).collect())
}

// ── 2: the refresh_token / offline_access coupling ───────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum GrantScopeCouplingRejection {
    RefreshWithoutOfflineAccess,
    OfflineAccessWithoutRefresh,
}

impl fmt::Display for GrantScopeCouplingRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefreshWithoutOfflineAccess => write!(
                f,
                "grant_types: refresh_token requires the offline_access scope"
            ),
            Self::OfflineAccessWithoutRefresh => write!(
                f,
                "scope: offline_access requires the refresh_token grant type"
            ),
        }
    }
}

/// *Supported fields*' closing sentence: `refresh_token` requires
/// `offline_access`; `offline_access` requires `refresh_token`. Both
/// directions, checked only after each side has already passed its own
/// field validation — this function takes the validated `Vec<String>`s,
/// not the raw request, so it never has to re-derive what counts as a
/// valid grant or scope token.
pub fn check_grant_scope_coupling(
    grant_types: &[String],
    scope_tokens: &[String],
) -> Result<(), GrantScopeCouplingRejection> {
    let has_refresh = grant_types.iter().any(|g| g == "refresh_token");
    let has_offline_access = scope_tokens.iter().any(|s| s == "offline_access");
    match (has_refresh, has_offline_access) {
        (true, false) => Err(GrantScopeCouplingRejection::RefreshWithoutOfflineAccess),
        (false, true) => Err(GrantScopeCouplingRejection::OfflineAccessWithoutRefresh),
        _ => Ok(()),
    }
}

// ── 2: application-identity URI fields ───────────────────────────────────────

/// Reused for every application-identity URI field — `logo_uri`,
/// `client_uri`, `policy_uri`, `tos_uri` — and for each entry of
/// `post_logout_redirect_uris`. The matrix states a byte bound for
/// `redirect_uris` (2048) and for `client_name` (512) and `scope` (1024
/// total / 64 per token) but gives none for this group; reused from
/// [`MAX_REDIRECT_URI_BYTES`] as the nearest already-justified bound for
/// "a URI this deployment stores", stated here as a judgement call rather
/// than a number the matrix actually gives — see the stage's return
/// package.
const MAX_APPLICATION_URI_BYTES: usize = MAX_REDIRECT_URI_BYTES;

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidApplicationUri {
    pub field: &'static str,
    pub uri: String,
    pub reason: &'static str,
}

impl fmt::Display for InvalidApplicationUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.field, self.uri, self.reason)
    }
}

/// *Supported fields* row 8: absent, or exactly one canonical HTTPS URI.
/// Shares [`well_formed_uri`]'s raw-string canonicality floor with
/// [`well_formed_shape`] (redirects) and [`validate_post_logout_redirect_uris`],
/// scoped here to HTTPS only — these are passive display/link targets,
/// never a redirect target, so the numeric-loopback-HTTP allowance a
/// redirect gets has no reason to extend here.
pub fn validate_application_uri(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<String>, InvalidApplicationUri> {
    let Some(uri) = raw.filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    let reject = |reason: &'static str| {
        Err(InvalidApplicationUri {
            field,
            uri: uri.to_owned(),
            reason,
        })
    };
    let parsed = match well_formed_uri(uri, MAX_APPLICATION_URI_BYTES) {
        Ok(p) => p,
        Err(reason) => return reject(reason),
    };
    if parsed.scheme() != "https" {
        return reject("must be https");
    }
    Ok(Some(uri.to_owned()))
}

// ── 2: post_logout_redirect_uris, the full policy ────────────────────────────

pub const MAX_POST_LOGOUT_REDIRECT_URIS: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum PostLogoutRejection {
    Malformed { uri: String, reason: &'static str },
    TooMany,
    Duplicate(String),
}

impl fmt::Display for PostLogoutRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { uri, reason } => {
                write!(f, "post_logout_redirect_uris: {uri}: {reason}")
            }
            Self::TooMany => write!(
                f,
                "post_logout_redirect_uris: at most {MAX_POST_LOGOUT_REDIRECT_URIS} entries"
            ),
            Self::Duplicate(u) => write!(f, "post_logout_redirect_uris: duplicate entry: {u}"),
        }
    }
}

/// *Supported fields* row 7: absent/empty, or 1–16 canonical unique HTTPS
/// redirects, independent of `redirect_uris`' own authorization profile —
/// even a [`RedirectProfile::PublicNativeLoopback`] registration's logout
/// URIs must be HTTPS. Replaces stage 1's `reject_http_post_logout_uris`
/// entirely: that function checked only the one profile-interaction rule
/// (no `http`) the Redirect corpus names explicitly; this adds the count,
/// uniqueness, and full canonical-form checks the *Supported fields* row
/// asks for, over the same field, so one function owns the whole rule
/// rather than two functions each owning half of it.
pub fn validate_post_logout_redirect_uris(
    raw: Option<&[String]>,
) -> Result<Vec<String>, PostLogoutRejection> {
    let Some(uris) = raw else {
        return Ok(Vec::new());
    };
    if uris.len() > MAX_POST_LOGOUT_REDIRECT_URIS {
        return Err(PostLogoutRejection::TooMany);
    }
    let mut seen = HashSet::with_capacity(uris.len());
    for uri in uris {
        if !seen.insert(uri.as_str()) {
            return Err(PostLogoutRejection::Duplicate(uri.clone()));
        }
        let parsed = well_formed_uri(uri, MAX_APPLICATION_URI_BYTES).map_err(|reason| {
            PostLogoutRejection::Malformed {
                uri: uri.clone(),
                reason,
            }
        })?;
        if parsed.scheme() != "https" {
            return Err(PostLogoutRejection::Malformed {
                uri: uri.clone(),
                reason: "must be https",
            });
        }
    }
    Ok(uris.to_vec())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "dynamic_registration_validation/tests.rs"]
mod tests;
