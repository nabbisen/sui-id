//! RFC 096-A stage 6c, half one: the eight bounded optional claims, RFC 096
//! `:663-672` verbatim -- `email`, `email_verified`, `preferred_username`,
//! `name`, `amr`, `acr`, `auth_time`, `at_hash`. A new sibling, not more of
//! `identity_claims.rs`: the four required claims take an expected issuer
//! and client ID; none of these eight take any caller-supplied expectation
//! at all, so their parameter list is just the claims, and keeping that
//! distinction visible is worth its own module rather than growing a third
//! parameter shape onto the second.
//!
//! RFC 096 `:679-681`: every one of these is deterministic -- present with
//! the wrong type, a duplicate member, an invalid character, or an exceeded
//! bound rejects the whole ID token rather than being silently ignored.
//! Duplicates are already caught one layer earlier, by stage 6a's
//! `first_duplicate_member` scan inside `verify_id_token_against_jwks`,
//! before a `VerifiedIdTokenClaims` exists for this module to read -- the
//! same note `time_claims.rs` makes for `iat`.
//!
//! Four of the eight (`amr`, `acr`, `auth_time`, `at_hash`) are validated
//! and then discarded: RFC 096 `:668-671` says each is "ignored for
//! authority and not persisted". This module still rejects a malformed one
//! -- that is the deterministic-rejection rule above -- it just has nothing
//! to hand the caller afterwards.
//!
//! Reachable only by tests, same as the rest of 096-A until 096-B1 routes
//! live traffic through it.

use crate::id_token::{NumericDateError, VerifiedIdTokenClaims, numeric_date};

/// RFC 096 `:665-666`'s "no control or bidi override/isolate", for
/// `preferred_username` and `name`. Confirmed empirically, not cited: none
/// of these nine code points are `char::is_control` in Rust, so they need
/// their own check. The five explicit directional formatting characters
/// (LRE/RLE/PDF/LRO/RLO, `U+202A`-`U+202E`) and the four isolates
/// (LRI/RLI/FSI/PDI, `U+2066`-`U+2069`).
fn is_bidi_override_or_isolate(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// RFC 096 `:635`'s "visible ASCII", reused verbatim for `amr`/`acr`/
/// `at_hash` (`:668-671`) rather than re-deriving the same bound a second
/// time: bytes `0x21`-`0x7E`, the same range `id_token.rs`'s `kid` check
/// already uses.
fn is_visible_ascii_byte(b: u8) -> bool {
    (0x21..=0x7E).contains(&b)
}

/// RFC 096 `:664`'s "valid mailbox-shaped" names no grammar, and no shape
/// check exists anywhere else in this codebase to reuse
/// (`sui_id_shared::normalize_email` is trim-plus-lowercase only, no shape
/// check at all). This was flagged as an unanchored interpretation call in
/// stage 6c's own package; the architect ruled on the one point that was
/// genuinely open (stage 6c-fix, 2026-10-09):
///
/// **A domain with no `.` is accepted, deliberately.** The first version of
/// this function required at least one `.` in the domain; ruled out
/// because `email` is metadata only (`:664`), never a lookup key, and an
/// existing link authenticates without it at all (`:693-695`) -- yet
/// `:679-681` makes a present-but-invalid optional claim reject the whole
/// token. Requiring a dot would fail the login of a real, plausible upstream
/// (`user@intranet`, `user@localhost` from an enterprise or internal IdP)
/// over a claim that grants no authority in either direction. **Do not
/// re-add a `domain.contains('.')` check** -- this was a deliberate ruling
/// against lockout risk, not an oversight.
///
/// Everything else stands as the original reading: exactly one `@`; a
/// non-empty local part with no ASCII whitespace or control character; a
/// non-empty domain part with no leading/trailing `.`, no `..`, and no
/// ASCII whitespace or control character -- malformed on any reading, and
/// unlike the dot requirement, costing nothing to keep. Deliberately
/// narrower than RFC 5322's full grammar -- the bound exists to reject
/// garbage, not to accept every byte RFC 5322 technically allows.
fn is_mailbox_shaped(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if domain.contains('@') {
        return false;
    }
    let plain = |c: char| !c.is_whitespace() && !c.is_control();
    let local_ok = !local.is_empty() && local.chars().all(plain);
    let domain_ok = !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && domain.chars().all(plain);
    local_ok && domain_ok
}

/// RFC 096 `:663-672`'s eight bounded-optional-claim rules, each its own
/// variant so a test can assert the specific one rather than `is_err()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionalClaimsError {
    EmailWrongType,
    EmailTooLong,
    EmailNotMailboxShaped,
    EmailVerifiedWrongType,
    PreferredUsernameWrongType,
    /// Checked before the scalar bound -- see
    /// [`validate_bounded_display_string`]'s doc comment for why the order
    /// is load-bearing rather than cosmetic.
    PreferredUsernameTooManyBytes,
    PreferredUsernameTooManyScalars,
    PreferredUsernameHasControlOrBidi,
    NameWrongType,
    NameTooManyBytes,
    NameTooManyScalars,
    NameHasControlOrBidi,
    AmrWrongType,
    AmrTooMany,
    AmrElementWrongType,
    AmrElementLength,
    AmrElementNotVisibleAscii,
    AmrDuplicate,
    AcrWrongType,
    AcrLength,
    AcrNotVisibleAscii,
    AuthTimeWrongType,
    AuthTimeNotAnInteger,
    AuthTimeNegative,
    AuthTimeOutOfRange,
    AtHashWrongType,
    AtHashLength,
    AtHashNotVisibleAscii,
}

impl std::fmt::Display for OptionalClaimsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmailWrongType => write!(f, "email is not a string"),
            Self::EmailTooLong => write!(f, "email is more than 254 bytes"),
            Self::EmailNotMailboxShaped => write!(f, "email is not mailbox-shaped"),
            Self::EmailVerifiedWrongType => write!(f, "email_verified is not a boolean"),
            Self::PreferredUsernameWrongType => write!(f, "preferred_username is not a string"),
            Self::PreferredUsernameTooManyBytes => {
                write!(f, "preferred_username is more than 512 UTF-8 bytes")
            }
            Self::PreferredUsernameTooManyScalars => {
                write!(f, "preferred_username is more than 128 scalars")
            }
            Self::PreferredUsernameHasControlOrBidi => {
                write!(
                    f,
                    "preferred_username contains a control, bidi override, or isolate character"
                )
            }
            Self::NameWrongType => write!(f, "name is not a string"),
            Self::NameTooManyBytes => write!(f, "name is more than 1024 UTF-8 bytes"),
            Self::NameTooManyScalars => write!(f, "name is more than 256 scalars"),
            Self::NameHasControlOrBidi => {
                write!(
                    f,
                    "name contains a control, bidi override, or isolate character"
                )
            }
            Self::AmrWrongType => write!(f, "amr is not an array"),
            Self::AmrTooMany => write!(f, "amr has more than 16 entries"),
            Self::AmrElementWrongType => write!(f, "an amr entry is not a string"),
            Self::AmrElementLength => write!(f, "an amr entry is not 1-64 bytes"),
            Self::AmrElementNotVisibleAscii => write!(f, "an amr entry is not visible ASCII"),
            Self::AmrDuplicate => write!(f, "amr repeats the same value"),
            Self::AcrWrongType => write!(f, "acr is not a string"),
            Self::AcrLength => write!(f, "acr is not 1-256 bytes"),
            Self::AcrNotVisibleAscii => write!(f, "acr is not visible ASCII"),
            Self::AuthTimeWrongType => write!(f, "auth_time is not a number"),
            Self::AuthTimeNotAnInteger => write!(f, "auth_time is not a canonical integer"),
            Self::AuthTimeNegative => write!(f, "auth_time is negative"),
            Self::AuthTimeOutOfRange => {
                write!(f, "auth_time is outside the representable clock range")
            }
            Self::AtHashWrongType => write!(f, "at_hash is not a string"),
            Self::AtHashLength => write!(f, "at_hash is not 1-256 bytes"),
            Self::AtHashNotVisibleAscii => write!(f, "at_hash is not visible ASCII"),
        }
    }
}

impl std::error::Error for OptionalClaimsError {}

/// `auth_time` reuses `id_token::numeric_date` rather than a second
/// NumericDate parser (the dispatch's own instruction) -- this maps its
/// four rejections onto this module's own variant names, the same
/// technique `time_claims::TimeClaimsError`'s `From<NumericDateError>` uses.
impl From<NumericDateError> for OptionalClaimsError {
    fn from(e: NumericDateError) -> Self {
        match e {
            NumericDateError::WrongType => Self::AuthTimeWrongType,
            NumericDateError::NotAnInteger => Self::AuthTimeNotAnInteger,
            NumericDateError::Negative => Self::AuthTimeNegative,
            NumericDateError::OutOfRange => Self::AuthTimeOutOfRange,
        }
    }
}

fn validate_email(claims: &VerifiedIdTokenClaims) -> Result<Option<String>, OptionalClaimsError> {
    let Some(value) = claims.claim("email") else {
        return Ok(None);
    };
    let email = value.as_str().ok_or(OptionalClaimsError::EmailWrongType)?;
    if email.len() > 254 {
        return Err(OptionalClaimsError::EmailTooLong);
    }
    if !is_mailbox_shaped(email) {
        return Err(OptionalClaimsError::EmailNotMailboxShaped);
    }
    Ok(Some(email.to_owned()))
}

/// RFC 096 `:664`: absent is not authoritative, but not an error either --
/// only a present, non-boolean value is a rejection. The returned `bool` is
/// the authority flag itself; only [`validate_optional_claims`] decides
/// what to do with it.
fn validate_email_verified(claims: &VerifiedIdTokenClaims) -> Result<bool, OptionalClaimsError> {
    let Some(value) = claims.claim("email_verified") else {
        return Ok(false);
    };
    value
        .as_bool()
        .ok_or(OptionalClaimsError::EmailVerifiedWrongType)
}

/// Shared by `preferred_username` and `name`: same shape (string, a byte
/// bound, a scalar bound, the same forbidden-character set), different
/// numbers and error variants.
///
/// **The byte check runs before the scalar check, and that order is
/// load-bearing.** Both claims' limits satisfy `max_bytes == 4 *
/// max_scalars` (128/512, 256/1024), and UTF-8 encodes at most 4 bytes per
/// scalar (confirmed empirically, not assumed) -- so for `N` scalars, the
/// maximum possible byte count is `4 * N`. At `N <= max_scalars`, that
/// maximum is `<= max_bytes`: **exceeding the byte bound without already
/// exceeding the scalar bound is mathematically impossible for these
/// numbers.** If the scalar check ran first, the byte check could never be
/// the first check to fire -- every input that would trip it has already
/// tripped the scalar check -- making it dead code. Checking bytes first
/// makes both checks reachable: an over-length multi-byte string trips the
/// byte check before the scalar check runs, and a long all-ASCII string
/// (which can exceed the scalar bound while staying well under the byte
/// bound) still reaches and trips the scalar check.
fn validate_bounded_display_string(
    value: &str,
    max_bytes: usize,
    max_scalars: usize,
    too_many_bytes: OptionalClaimsError,
    too_many_scalars: OptionalClaimsError,
    has_control_or_bidi: OptionalClaimsError,
) -> Result<(), OptionalClaimsError> {
    if value.len() > max_bytes {
        return Err(too_many_bytes);
    }
    if value.chars().count() > max_scalars {
        return Err(too_many_scalars);
    }
    if value
        .chars()
        .any(|c| c.is_control() || is_bidi_override_or_isolate(c))
    {
        return Err(has_control_or_bidi);
    }
    Ok(())
}

fn validate_preferred_username(
    claims: &VerifiedIdTokenClaims,
) -> Result<Option<String>, OptionalClaimsError> {
    let Some(value) = claims.claim("preferred_username") else {
        return Ok(None);
    };
    let s = value
        .as_str()
        .ok_or(OptionalClaimsError::PreferredUsernameWrongType)?;
    validate_bounded_display_string(
        s,
        512,
        128,
        OptionalClaimsError::PreferredUsernameTooManyBytes,
        OptionalClaimsError::PreferredUsernameTooManyScalars,
        OptionalClaimsError::PreferredUsernameHasControlOrBidi,
    )?;
    Ok(Some(s.to_owned()))
}

fn validate_name(claims: &VerifiedIdTokenClaims) -> Result<Option<String>, OptionalClaimsError> {
    let Some(value) = claims.claim("name") else {
        return Ok(None);
    };
    let s = value.as_str().ok_or(OptionalClaimsError::NameWrongType)?;
    validate_bounded_display_string(
        s,
        1024,
        256,
        OptionalClaimsError::NameTooManyBytes,
        OptionalClaimsError::NameTooManyScalars,
        OptionalClaimsError::NameHasControlOrBidi,
    )?;
    Ok(Some(s.to_owned()))
}

/// Validated and then discarded -- RFC 096 `:668`: "ignored for authority
/// and not persisted". Uniqueness is per-array, the same rule 6a's `aud`
/// uses.
fn validate_amr(claims: &VerifiedIdTokenClaims) -> Result<(), OptionalClaimsError> {
    let Some(value) = claims.claim("amr") else {
        return Ok(());
    };
    let items = value.as_array().ok_or(OptionalClaimsError::AmrWrongType)?;
    if items.len() > 16 {
        return Err(OptionalClaimsError::AmrTooMany);
    }
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    for item in items {
        let s = item
            .as_str()
            .ok_or(OptionalClaimsError::AmrElementWrongType)?;
        if !(1..=64).contains(&s.len()) {
            return Err(OptionalClaimsError::AmrElementLength);
        }
        if !s.bytes().all(is_visible_ascii_byte) {
            return Err(OptionalClaimsError::AmrElementNotVisibleAscii);
        }
        if !seen.insert(s) {
            return Err(OptionalClaimsError::AmrDuplicate);
        }
    }
    Ok(())
}

/// Validated and then discarded, same as [`validate_amr`].
fn validate_acr(claims: &VerifiedIdTokenClaims) -> Result<(), OptionalClaimsError> {
    let Some(value) = claims.claim("acr") else {
        return Ok(());
    };
    let s = value.as_str().ok_or(OptionalClaimsError::AcrWrongType)?;
    if !(1..=256).contains(&s.len()) {
        return Err(OptionalClaimsError::AcrLength);
    }
    if !s.bytes().all(is_visible_ascii_byte) {
        return Err(OptionalClaimsError::AcrNotVisibleAscii);
    }
    Ok(())
}

/// Validated and then discarded, same as [`validate_amr`]. Shares
/// `id_token::numeric_date` rather than a second parser; its own bound
/// (representable clock range) is exactly what that function already
/// checks, so there is nothing further to add here.
fn validate_auth_time(claims: &VerifiedIdTokenClaims) -> Result<(), OptionalClaimsError> {
    let Some(value) = claims.claim("auth_time") else {
        return Ok(());
    };
    numeric_date(value)?;
    Ok(())
}

/// Validated and then discarded -- but *why* it is validated at all despite
/// being discarded is RFC 096 `:682-684`'s own point, not this module's:
/// the access token it would hash is never used for identity, userinfo, or
/// an API, and is zeroized immediately, so a well-formed `at_hash` grants
/// nothing. A malformed one still fails the bounded-claims envelope,
/// because `:679-681`'s determinism rule does not carve out an exception
/// for a claim whose value turns out not to matter.
fn validate_at_hash(claims: &VerifiedIdTokenClaims) -> Result<(), OptionalClaimsError> {
    let Some(value) = claims.claim("at_hash") else {
        return Ok(());
    };
    let s = value.as_str().ok_or(OptionalClaimsError::AtHashWrongType)?;
    if !(1..=256).contains(&s.len()) {
        return Err(OptionalClaimsError::AtHashLength);
    }
    if !s.bytes().all(is_visible_ascii_byte) {
        return Err(OptionalClaimsError::AtHashNotVisibleAscii);
    }
    Ok(())
}

/// The three values [`validate_optional_claims`] carries forward: the email
/// RFC 096 `:687-689` calls "verified email state" (present only when
/// `email` is mailbox-shaped, bounded, *and* `email_verified` is exactly
/// `true` -- present-but-unverified collapses to `None`, matching
/// `:694-698`'s "an absent or unverified email denies" treating both the
/// same), and the two bounded display hints. `amr`/`acr`/`auth_time`/
/// `at_hash` are validated but never reach this struct at all -- RFC 096
/// `:668-671`'s "not persisted" is true from the moment they are checked,
/// not a promise kept by some later code path that just happens not to read
/// them.
///
/// No `Debug` derive: holds `verified_email`, and RFC 096 `:707` is explicit
/// that raw email never appears in a log or metric label. Nothing today
/// needs to print this struct; if something later does, it should redact by
/// hand the same way `identity_capability::IdentityCapability` does, not
/// pick up a derive that silently stops redacting.
#[derive(Clone, PartialEq, Eq)]
pub struct ValidatedOptionalClaims {
    verified_email: Option<String>,
    preferred_username: Option<String>,
    name: Option<String>,
}

impl ValidatedOptionalClaims {
    pub fn verified_email(&self) -> Option<&str> {
        self.verified_email.as_deref()
    }

    pub fn preferred_username(&self) -> Option<&str> {
        self.preferred_username.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Validates all eight bounded optional claims. No lookup, no mutation,
/// same as `validate_identity_claims` and `validate_iat`: everything needed
/// is either a parameter or already on `claims`.
pub fn validate_optional_claims(
    claims: &VerifiedIdTokenClaims,
) -> Result<ValidatedOptionalClaims, OptionalClaimsError> {
    let email = validate_email(claims)?;
    let email_verified = validate_email_verified(claims)?;
    let preferred_username = validate_preferred_username(claims)?;
    let name = validate_name(claims)?;
    validate_amr(claims)?;
    validate_acr(claims)?;
    validate_auth_time(claims)?;
    validate_at_hash(claims)?;

    let verified_email = if email_verified { email } else { None };

    Ok(ValidatedOptionalClaims {
        verified_email,
        preferred_username,
        name,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "optional_claims/tests.rs"]
mod tests;
