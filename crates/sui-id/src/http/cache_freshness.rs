//! RFC 096-A stage 4b: the closed `Cache-Control` directive set, `Age`/
//! `Date`/`ETag` validation, and freshness arithmetic for federation
//! security documents (discovery, JWKS). RFC 096 `:841-872`.
//!
//! Pure functions over parsed header *values* (the caller has already
//! split a response into its field-line strings) and a wall clock reading
//! taken once. No storage, no network, no concurrency -- stage 4c (200/304
//! semantics) and stage 4d (the request/cache state table) are what wire
//! this into an actual cache. Reachable only by tests until then.
//!
//! This is a federation security-document cache profile, not a general
//! RFC 9111 cache: only ETag revalidation and the five directives below are
//! recognised, stale use is always forbidden, and the two per-cache
//! freshness tables are this profile's own, not derived from any header.
//!
//! ## Clock
//!
//! No new clock abstraction. This reuses the existing
//! [`sui_id_core::time::Clock`]/[`SharedClock`], already injected the same
//! way by `authn::step_up` and `authn::session`. [`validate_response`] is
//! the one function that calls `clock.now()`, exactly once, producing the
//! `trusted_now` every other function here takes as a plain
//! `DateTime<Utc>` value -- so every rule below (including the `Date`
//! boundary tests) is testable by handing a literal timestamp to a pure
//! function, never by mocking a clock object.
//!
//! The freshness formula's other time term, "monotonic resident time", is
//! accepted by [`current_age`] as an already-computed [`Duration`] rather
//! than sampled here. There is no stored entry yet for a monotonic reading
//! to be relative *to* -- stage 4d owns the actual timestamps and can
//! sample `std::time::Instant` directly; inventing a monotonic-clock
//! abstraction in this stage would mock a measurement this stage never
//! takes. This is also what makes "clock regression expires the entry"
//! hold structurally rather than by convention: resident time, supplied by
//! the caller's monotonic source, cannot run backwards, and the wall
//! clock's only other contribution -- the `Date` term in
//! [`initial_current_age`] -- is floored at zero, so rolling `trusted_now`
//! backwards can never produce a negative term that would lower
//! `current_age` below what `Age` alone already states.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sui_id_core::time::SharedClock;

/// RFC 096 `:841-872`'s two per-cache freshness tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheKind {
    Discovery,
    Jwks,
}

impl CacheKind {
    pub fn default_freshness(self) -> Duration {
        match self {
            Self::Discovery => Duration::from_secs(60 * 60),
            Self::Jwks => Duration::from_secs(15 * 60),
        }
    }

    pub fn max_freshness(self) -> Duration {
        match self {
            Self::Discovery => Duration::from_secs(24 * 60 * 60),
            Self::Jwks => Duration::from_secs(6 * 60 * 60),
        }
    }
}

/// An attacker cannot be allowed to force unbounded parsing work by naming
/// thousands of unknown directives in one field; "ignore unknown
/// directives" is bounded by rejecting a combined value past this length
/// before any per-directive work happens. Not named by the dispatch's own
/// test list -- added defensively, in the same spirit as stage 2/3's size
/// limits, and flagged in the package rather than left to be found.
pub const MAX_CACHE_CONTROL_LEN: usize = 4096;

/// The closed, case-insensitive directive set, after validation. Each
/// field is populated independently of the others; [`lifetime`] is what
/// applies this profile's precedence rules to turn this into a single
/// freshness duration (or "not retained at all", for `no_store`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheDirectives {
    pub no_store: bool,
    pub no_cache: bool,
    /// Recorded for fidelity; changes no behaviour here, because stale use
    /// is already forbidden unconditionally in this profile.
    pub must_revalidate: bool,
    pub max_age: Option<Duration>,
    /// Parsed and validated like `max_age`, but never read by
    /// [`lifetime`]: this is a private cache, and shared-cache `s-maxage`
    /// does not apply to it. A malformed `s-maxage` still rejects the
    /// response -- parsing happens regardless of whether the value will be
    /// used.
    pub s_maxage: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheValidationError {
    CacheControlOversized,
    ObsoleteLineFolding,
    InvalidListSyntax,
    DuplicateDirective(&'static str),
    /// `no_store` together with `max-age`, `s-maxage`, or
    /// `must-revalidate`: directives whose entire purpose is describing a
    /// retained entry's lifetime, present alongside a directive that
    /// forbids retaining one at all. My reading of "conflicting" --
    /// flagged for the architect in the package, the same way stage 3b's
    /// "disagree" reading was.
    ConflictingDirectives,
    DirectiveMissingValue(&'static str),
    DirectiveHasUnexpectedValue(&'static str),
    QuotedDeltaSeconds(&'static str),
    NonCanonicalDeltaSeconds(&'static str),
    DeltaSecondsOverflow(&'static str),
    DuplicateField(&'static str),
    MalformedAge,
    AgeOverflow,
    MalformedDate,
    DateInFuture,
    MalformedEtag,
    EtagTooLarge,
}

impl std::fmt::Display for CacheValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CacheControlOversized => write!(f, "Cache-Control exceeds the bounded length"),
            Self::ObsoleteLineFolding => write!(f, "header value carries obsolete line folding"),
            Self::InvalidListSyntax => write!(f, "Cache-Control is not a valid HTTP list"),
            Self::DuplicateDirective(name) => write!(f, "directive {name:?} occurs more than once"),
            Self::ConflictingDirectives => {
                write!(f, "no-store conflicts with a retention directive")
            }
            Self::DirectiveMissingValue(name) => write!(f, "directive {name:?} requires a value"),
            Self::DirectiveHasUnexpectedValue(name) => {
                write!(f, "directive {name:?} does not take a value")
            }
            Self::QuotedDeltaSeconds(name) => write!(f, "{name:?}'s delta-seconds is quoted"),
            Self::NonCanonicalDeltaSeconds(name) => {
                write!(f, "{name:?}'s delta-seconds is not canonical decimal")
            }
            Self::DeltaSecondsOverflow(name) => write!(f, "{name:?}'s delta-seconds overflows"),
            Self::DuplicateField(name) => write!(f, "{name} occurs more than once"),
            Self::MalformedAge => write!(f, "Age is not canonical decimal seconds"),
            Self::AgeOverflow => write!(f, "Age exceeds u32::MAX seconds"),
            Self::MalformedDate => write!(f, "Date is not IMF-fixdate"),
            Self::DateInFuture => {
                write!(f, "Date is more than 60 seconds ahead of the trusted clock")
            }
            Self::MalformedEtag => write!(f, "ETag does not match the entity-tag grammar"),
            Self::EtagTooLarge => write!(f, "ETag exceeds 256 bytes"),
        }
    }
}

impl std::error::Error for CacheValidationError {}

fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

enum DeltaSecondsError {
    NonCanonical,
    Overflow,
}

/// `delta-seconds`, RFC 9111 §1.2.2: one or more digits, no sign, no
/// leading zero unless the whole value is exactly `"0"`.
fn parse_canonical_delta_seconds(value: &str) -> Result<u64, DeltaSecondsError> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DeltaSecondsError::NonCanonical);
    }
    if value.len() > 1 && value.as_bytes()[0] == b'0' {
        return Err(DeltaSecondsError::NonCanonical);
    }
    value
        .parse::<u64>()
        .map_err(|_| DeltaSecondsError::Overflow)
}

/// Splits a combined `Cache-Control` value (multiple field lines already
/// joined with `,` by the caller -- RFC 9111 §5.2's HTTP-list combination)
/// into trimmed, non-empty list elements, honouring quoted-string commas.
/// An unterminated quoted-string is the one list-syntax error this stage
/// detects structurally; `,,` (an empty list element) is valid HTTP-list
/// grammar and is dropped rather than rejected.
fn split_http_list(combined: &str) -> Result<Vec<&str>, CacheValidationError> {
    let bytes = combined.as_bytes();
    let mut elements = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_quotes = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_quotes {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_quotes = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => {
                in_quotes = true;
                i += 1;
            }
            b',' => {
                elements.push(combined[start..i].trim());
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if in_quotes {
        return Err(CacheValidationError::InvalidListSyntax);
    }
    elements.push(combined[start..].trim());
    Ok(elements.into_iter().filter(|e| !e.is_empty()).collect())
}

fn parse_directive(elem: &str) -> Result<(&str, Option<&str>), CacheValidationError> {
    let (name, value) = match elem.split_once('=') {
        Some((n, v)) => (n.trim_end(), Some(v.trim_start())),
        None => (elem, None),
    };
    if name.is_empty() || !name.bytes().all(is_tchar) {
        return Err(CacheValidationError::InvalidListSyntax);
    }
    Ok((name, value))
}

fn delta_seconds_directive(
    name: &'static str,
    value: Option<&str>,
) -> Result<Duration, CacheValidationError> {
    let raw = value.ok_or(CacheValidationError::DirectiveMissingValue(name))?;
    if raw.starts_with('"') {
        return Err(CacheValidationError::QuotedDeltaSeconds(name));
    }
    match parse_canonical_delta_seconds(raw) {
        Ok(secs) => Ok(Duration::from_secs(secs)),
        Err(DeltaSecondsError::NonCanonical) => {
            Err(CacheValidationError::NonCanonicalDeltaSeconds(name))
        }
        Err(DeltaSecondsError::Overflow) => Err(CacheValidationError::DeltaSecondsOverflow(name)),
    }
}

fn flag_directive(name: &'static str, value: Option<&str>) -> Result<(), CacheValidationError> {
    if value.is_some() {
        return Err(CacheValidationError::DirectiveHasUnexpectedValue(name));
    }
    Ok(())
}

/// Parses and validates the closed directive set from every `Cache-Control`
/// field-line value, combined per RFC 9111 §5.2 ("Cache-Control is the only
/// field that may" combine this way -- every other field in this stage
/// rejects on more than one occurrence instead). Obsolete line folding
/// (`CR`/`LF` surviving into a field value) and control characters are
/// rejected before list-splitting; conflicting/duplicate recognised
/// directives and malformed delta-seconds are rejected while walking the
/// parsed list.
pub fn parse_cache_control(field_values: &[&str]) -> Result<CacheDirectives, CacheValidationError> {
    let combined = field_values.join(",");
    if combined.len() > MAX_CACHE_CONTROL_LEN {
        return Err(CacheValidationError::CacheControlOversized);
    }
    if combined.bytes().any(|b| b == b'\r' || b == b'\n') {
        return Err(CacheValidationError::ObsoleteLineFolding);
    }
    if combined
        .bytes()
        .any(|b| (b < 0x20 && b != b'\t') || b == 0x7f)
    {
        return Err(CacheValidationError::InvalidListSyntax);
    }

    let mut directives = CacheDirectives::default();
    let mut seen_no_store = false;
    let mut seen_no_cache = false;
    let mut seen_must_revalidate = false;
    let mut seen_max_age = false;
    let mut seen_s_maxage = false;

    for elem in split_http_list(&combined)? {
        let (name, value) = parse_directive(elem)?;
        match name.to_ascii_lowercase().as_str() {
            "no-store" => {
                if seen_no_store {
                    return Err(CacheValidationError::DuplicateDirective("no-store"));
                }
                flag_directive("no-store", value)?;
                seen_no_store = true;
                directives.no_store = true;
            }
            "no-cache" => {
                if seen_no_cache {
                    return Err(CacheValidationError::DuplicateDirective("no-cache"));
                }
                flag_directive("no-cache", value)?;
                seen_no_cache = true;
                directives.no_cache = true;
            }
            "must-revalidate" => {
                if seen_must_revalidate {
                    return Err(CacheValidationError::DuplicateDirective("must-revalidate"));
                }
                flag_directive("must-revalidate", value)?;
                seen_must_revalidate = true;
                directives.must_revalidate = true;
            }
            "max-age" => {
                if seen_max_age {
                    return Err(CacheValidationError::DuplicateDirective("max-age"));
                }
                seen_max_age = true;
                directives.max_age = Some(delta_seconds_directive("max-age", value)?);
            }
            "s-maxage" => {
                if seen_s_maxage {
                    return Err(CacheValidationError::DuplicateDirective("s-maxage"));
                }
                seen_s_maxage = true;
                directives.s_maxage = Some(delta_seconds_directive("s-maxage", value)?);
            }
            // Ignored: unknown, bounded above by MAX_CACHE_CONTROL_LEN.
            _ => {}
        }
    }

    if directives.no_store
        && (directives.max_age.is_some()
            || directives.s_maxage.is_some()
            || directives.must_revalidate)
    {
        return Err(CacheValidationError::ConflictingDirectives);
    }

    Ok(directives)
}

/// This profile's precedence, RFC 096 `:841-872`'s table: `no_store` means
/// there is no freshness duration to speak of at all -- the validated 200
/// may be used for the current operation only, with no entry and no ETag
/// retained, so this returns `None` rather than `Some(Duration::ZERO)`.
/// Otherwise `no_cache` floors freshness at zero regardless of any
/// `max-age` value also present (the latter was still fully validated by
/// [`parse_cache_control`]; it just does not change this answer). A
/// present `max-age` is honoured exactly, clamped only at the per-cache
/// maximum, and never raised to the default when it is smaller.
pub fn lifetime(directives: &CacheDirectives, kind: CacheKind) -> Option<Duration> {
    if directives.no_store {
        return None;
    }
    if directives.no_cache {
        return Some(Duration::ZERO);
    }
    Some(match directives.max_age {
        Some(max_age) => max_age.min(kind.max_freshness()),
        None => kind.default_freshness(),
    })
}

/// `Age`: canonical decimal seconds, bounded by `u32::MAX`. Zero or one
/// occurrence; absent is zero.
pub fn validate_age(values: &[&str]) -> Result<Duration, CacheValidationError> {
    match values {
        [] => Ok(Duration::ZERO),
        [v] => match parse_canonical_delta_seconds(v) {
            Ok(secs) if secs <= u32::MAX as u64 => Ok(Duration::from_secs(secs)),
            Ok(_) => Err(CacheValidationError::AgeOverflow),
            Err(DeltaSecondsError::NonCanonical) => Err(CacheValidationError::MalformedAge),
            Err(DeltaSecondsError::Overflow) => Err(CacheValidationError::AgeOverflow),
        },
        _ => Err(CacheValidationError::DuplicateField("Age")),
    }
}

/// `Date`: strict IMF-fixdate (RFC 9110 §5.6.7), rejected if later than
/// `trusted_now + 60s`. Zero or one occurrence; absent is `None`.
/// `trusted_now` is a plain value, not a clock -- see the module doc
/// comment for why only [`validate_response`] ever reads a clock.
pub fn validate_date(
    values: &[&str],
    trusted_now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, CacheValidationError> {
    match values {
        [] => Ok(None),
        [v] => {
            let naive = chrono::NaiveDateTime::parse_from_str(v, "%a, %d %b %Y %H:%M:%S GMT")
                .map_err(|_| CacheValidationError::MalformedDate)?;
            let parsed = naive.and_utc();
            if parsed > trusted_now + chrono::Duration::seconds(60) {
                return Err(CacheValidationError::DateInFuture);
            }
            Ok(Some(parsed))
        }
        _ => Err(CacheValidationError::DuplicateField("Date")),
    }
}

/// One strong or weak entity-tag (RFC 9110 §8.8.3), at most 256
/// visible/quoted bytes inside the quotes. Zero or one occurrence; absent
/// is `None`.
pub fn validate_etag(values: &[&str]) -> Result<Option<String>, CacheValidationError> {
    match values {
        [] => Ok(None),
        [v] => {
            let rest = v.strip_prefix("W/").unwrap_or(v);
            let inner = rest
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or(CacheValidationError::MalformedEtag)?;
            if inner.len() > 256 {
                return Err(CacheValidationError::EtagTooLarge);
            }
            if !inner
                .bytes()
                .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b))
            {
                return Err(CacheValidationError::MalformedEtag);
            }
            Ok(Some((*v).to_string()))
        }
        _ => Err(CacheValidationError::DuplicateField("ETag")),
    }
}

/// `Content-Type`: this stage's only new rule for it is multiplicity --
/// the value's own media-type semantics are `response_bounds`'s job, not
/// this one's. Zero or one occurrence.
pub fn validate_content_type_multiplicity(values: &[&str]) -> Result<(), CacheValidationError> {
    if values.len() > 1 {
        return Err(CacheValidationError::DuplicateField("Content-Type"));
    }
    Ok(())
}

/// `initial_current_age`, RFC 9111 §4.2.3: the larger of the `Age` header
/// (zero if absent) and the non-negative gap between `trusted_now` and
/// `Date` (zero if `Date` is absent, and floored at zero rather than
/// negative when `Date` is in the past of a clock that later moved
/// forward). This is the one place the wall clock enters the freshness
/// formula; a rolled-back `trusted_now` can only ever push this term
/// toward its zero floor, never below it, so it cannot manufacture extra
/// freshness.
pub fn initial_current_age(
    age: Duration,
    date: Option<DateTime<Utc>>,
    trusted_now: DateTime<Utc>,
) -> Duration {
    let date_term = date
        .map(|d| trusted_now.signed_duration_since(d))
        .and_then(|gap| gap.to_std().ok())
        .unwrap_or(Duration::ZERO);
    age.max(date_term)
}

/// `current_age`: `initial_current_age` plus however long the entry has
/// been resident, per RFC 9111 §4.2.3. `resident` is a monotonic duration
/// the caller already measured -- see the module doc comment for why no
/// clock is read here.
pub fn current_age(initial_current_age: Duration, resident: Duration) -> Duration {
    initial_current_age + resident
}

/// `lifetime > current_age`, RFC 9111 §4.2: equality is stale, and
/// `no_store` (`lifetime` is `None`) is never fresh.
pub fn is_fresh(lifetime: Option<Duration>, current_age: Duration) -> bool {
    lifetime.is_some_and(|l| l > current_age)
}

/// Validated metadata for one response: this profile's freshness duration
/// (`None` for `no_store`), the age at validation time, and the retained
/// validator. Resident time and the fresh/stale verdict are deliberately
/// not part of this -- they depend on how long the caller holds the entry,
/// which stage 4c/4d own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCacheMetadata {
    pub directives: CacheDirectives,
    pub lifetime: Option<Duration>,
    pub initial_current_age: Duration,
    pub etag: Option<String>,
}

/// The one function in this stage that reads a clock, and the only
/// injection point: `clock.now()` runs exactly once, producing the
/// `trusted_now` every pure rule above is given as a plain value. Runs
/// every rule in this module over one response's header values and this
/// profile's per-cache table.
pub fn validate_response(
    cache_control_values: &[&str],
    age_values: &[&str],
    date_values: &[&str],
    etag_values: &[&str],
    content_type_values: &[&str],
    kind: CacheKind,
    clock: &SharedClock,
) -> Result<ValidatedCacheMetadata, CacheValidationError> {
    let trusted_now = clock.now();
    let directives = parse_cache_control(cache_control_values)?;
    let age = validate_age(age_values)?;
    let date = validate_date(date_values, trusted_now)?;
    let etag = validate_etag(etag_values)?;
    validate_content_type_multiplicity(content_type_values)?;

    Ok(ValidatedCacheMetadata {
        lifetime: lifetime(&directives, kind),
        initial_current_age: initial_current_age(age, date, trusted_now),
        directives,
        etag,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "cache_freshness/tests.rs"]
mod tests;
