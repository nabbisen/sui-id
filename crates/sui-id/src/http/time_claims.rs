//! RFC 096-A stage 6b: the time claims -- `exp`, `iat`, `nbf`. A new
//! sibling, beside `identity_claims.rs` rather than inside it: `iat`'s
//! validator needs different parameters (`created_at`, `now`) than
//! `identity_claims`'s (`expected_issuer`, `expected_client_id`), and 6c
//! adds a third group with its own parameter shape again -- one module per
//! group keeps each parameter list to exactly what its own rule needs,
//! rather than one function accumulating all three stages' parameters.
//!
//! **Only `iat` is validated here.** `exp` and `nbf` are already fully
//! handled earlier in the pipeline: their window arithmetic is
//! `jsonwebtoken`'s own (`validation_for`, in `id_token.rs`, explains both
//! in detail), and their canonical-integer *shape* is checked even earlier
//! still, before `jsonwebtoken::decode` ever runs (`VerificationError::
//! MalformedTimeClaim`) -- by the time a `VerifiedIdTokenClaims` exists for
//! this module to read, both have already passed or the call never got
//! this far. `iat` has no equivalent upstream check at all; this module is
//! where the whole rule lives.
//!
//! **No durable mutation, same as `identity_claims`.** `created_at` (the
//! attempt's own creation time) and `now` (the trusted wall-clock reading)
//! both arrive as parameters -- RFC 096 `:63-66`. The durable attempt record
//! that would supply `created_at` in production is 096-B1's, and reading a
//! clock inside a function this stage calls "pure" would be exactly the
//! kind of hidden dependency the cache stages (4b-4d) were careful to avoid.
//!
//! Reachable only by tests, same as the rest of 096-A until 096-B1 routes
//! live traffic through it.

use chrono::{DateTime, Utc};

use crate::id_token::{NumericDateError, VerifiedIdTokenClaims, numeric_date};

/// RFC 096 `:661`'s `iat` rule, in named rejections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeClaimsError {
    Missing,
    WrongType,
    NotAnInteger,
    Negative,
    OutOfRange,
    /// `iat < created_at - 60s`. RFC 096 `:676`: the substitution defence --
    /// a token minted before this attempt started is refused even when its
    /// signature, issuer, audience and expiry are all otherwise perfect.
    TooOld,
    /// `iat > now + 60s`.
    TooNew,
}

impl std::fmt::Display for TimeClaimsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "iat is absent"),
            Self::WrongType => write!(f, "iat is not a number"),
            Self::NotAnInteger => write!(f, "iat is not a canonical integer"),
            Self::Negative => write!(f, "iat is negative"),
            Self::OutOfRange => write!(f, "iat is outside the representable clock range"),
            Self::TooOld => write!(f, "iat is before this attempt's creation, minus skew"),
            Self::TooNew => write!(f, "iat is after now, plus skew"),
        }
    }
}

impl std::error::Error for TimeClaimsError {}

impl From<NumericDateError> for TimeClaimsError {
    fn from(e: NumericDateError) -> Self {
        match e {
            NumericDateError::WrongType => Self::WrongType,
            NumericDateError::NotAnInteger => Self::NotAnInteger,
            NumericDateError::Negative => Self::Negative,
            NumericDateError::OutOfRange => Self::OutOfRange,
        }
    }
}

/// RFC 096 `:675`: the 60-second skew, fixed and not provider-controlled.
/// Named once; both of `iat`'s bounds use it.
const SKEW_SECONDS: i64 = 60;

/// Validates `iat` against `created_at` (the lower bound, the substitution
/// defence) and `now` (the upper bound, ordinary clock skew). Both are
/// parameters -- see the module doc comment for why neither is read from a
/// clock inside this function.
pub fn validate_iat(
    claims: &VerifiedIdTokenClaims,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), TimeClaimsError> {
    let value = claims.claim("iat").ok_or(TimeClaimsError::Missing)?;
    let iat_secs = numeric_date(value)?;

    let lower_bound = created_at.timestamp() - SKEW_SECONDS;
    let upper_bound = now.timestamp() + SKEW_SECONDS;

    if iat_secs < lower_bound {
        return Err(TimeClaimsError::TooOld);
    }
    if iat_secs > upper_bound {
        return Err(TimeClaimsError::TooNew);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "time_claims/tests.rs"]
mod tests;
