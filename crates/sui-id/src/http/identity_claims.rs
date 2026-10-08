//! RFC 096-A stage 6a: the four required identity claims, RFC 096 `:656-659`
//! verbatim -- `iss`, `sub`, `aud`, `azp`. A new sibling module, not more of
//! `id_token.rs`: that module is already structural-parse-plus-signature, and
//! 6b/6c/7 will each add their own claim-validation surface beside this one,
//! so claim validation gets its own module from the start rather than
//! growing `id_token.rs` indefinitely.
//!
//! **No durable mutation, same as `verify_id_token_against_jwks`.** The
//! expected issuer and client ID are parameters here, not a database read or
//! a provider lookup -- RFC 096 `:63-66`.
//!
//! Reachable only by tests, same as the rest of 096-A until 096-B1 routes
//! live traffic through it.

use crate::id_token::VerifiedIdTokenClaims;

/// RFC 096 `:656-659`'s four required-claim rules, each its own variant so a
/// test can assert the specific one rather than `is_err()`. A repeated
/// member for any of the four is not among them: that is
/// `VerificationError::DuplicatePayloadMember`, checked one layer earlier,
/// inside `verify_id_token_against_jwks`, before a `VerifiedIdTokenClaims`
/// can even exist to be passed in here -- see that variant's doc comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityClaimsError {
    IssuerMissing,
    IssuerNotString,
    /// The claim and the configured issuer differ by at least one byte.
    /// There is no normalisation step before this comparison runs.
    IssuerMismatch,
    SubjectMissing,
    SubjectNotString,
    /// Not 1-255 UTF-8 bytes. Both edges share one variant, matching stage
    /// 2's `KidInvalid` precedent for a single length rule.
    SubjectLength,
    SubjectControlCharacter,
    AudienceMissing,
    /// Neither a string nor an array of strings, or an array containing a
    /// non-string element.
    AudienceInvalidShape,
    AudienceEmpty,
    /// More than 8 entries.
    AudienceTooMany,
    /// The same string appears more than once in the `aud` array.
    AudienceDuplicateValue,
    /// None of the audience values equals the configured client ID.
    AudienceDoesNotContainClientId,
    /// `aud` has more than one value and `azp` is absent.
    AzpMissing,
    AzpNotString,
    /// `azp` is present (required for multi-valued `aud`, optional and still
    /// checked for single-valued `aud`) but is not the exact client ID.
    AzpMismatch,
}

impl std::fmt::Display for IdentityClaimsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IssuerMissing => write!(f, "iss is absent"),
            Self::IssuerNotString => write!(f, "iss is not a string"),
            Self::IssuerMismatch => {
                write!(f, "iss does not byte-exactly match the configured issuer")
            }
            Self::SubjectMissing => write!(f, "sub is absent"),
            Self::SubjectNotString => write!(f, "sub is not a string"),
            Self::SubjectLength => write!(f, "sub is not 1-255 UTF-8 bytes"),
            Self::SubjectControlCharacter => write!(f, "sub contains a control character"),
            Self::AudienceMissing => write!(f, "aud is absent"),
            Self::AudienceInvalidShape => {
                write!(f, "aud is not a string or an array of strings")
            }
            Self::AudienceEmpty => write!(f, "aud is an empty array"),
            Self::AudienceTooMany => write!(f, "aud has more than 8 entries"),
            Self::AudienceDuplicateValue => write!(f, "aud repeats the same value"),
            Self::AudienceDoesNotContainClientId => {
                write!(f, "aud does not contain the configured client ID")
            }
            Self::AzpMissing => write!(f, "azp is absent and aud has more than one value"),
            Self::AzpNotString => write!(f, "azp is not a string"),
            Self::AzpMismatch => write!(f, "azp is not the exact client ID"),
        }
    }
}

impl std::error::Error for IdentityClaimsError {}

/// RFC 096 `:656-659`'s `sub` requirement, proven rather than merely held:
/// there is no public constructor and no public field, the same shape as
/// `VerifiedIdTokenClaims` and `RetainedEntry`, so the only way to hold one
/// is [`validate_identity_claims`]'s success path. `sub` is carried
/// unmodified from the payload -- RFC 096 `:693` makes `(provider_id, sub)`
/// the lookup key, so this type exists specifically so nothing downstream
/// can normalise it by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredIdentityClaims {
    sub: String,
}

impl RequiredIdentityClaims {
    pub fn sub(&self) -> &str {
        &self.sub
    }
}

fn validate_issuer(
    claims: &VerifiedIdTokenClaims,
    expected_issuer: &str,
) -> Result<(), IdentityClaimsError> {
    let iss = claims
        .claim("iss")
        .ok_or(IdentityClaimsError::IssuerMissing)?
        .as_str()
        .ok_or(IdentityClaimsError::IssuerNotString)?;
    if iss != expected_issuer {
        return Err(IdentityClaimsError::IssuerMismatch);
    }
    Ok(())
}

fn validate_subject(claims: &VerifiedIdTokenClaims) -> Result<String, IdentityClaimsError> {
    let sub = claims
        .claim("sub")
        .ok_or(IdentityClaimsError::SubjectMissing)?
        .as_str()
        .ok_or(IdentityClaimsError::SubjectNotString)?;
    if !(1..=255).contains(&sub.len()) {
        return Err(IdentityClaimsError::SubjectLength);
    }
    if sub.chars().any(char::is_control) {
        return Err(IdentityClaimsError::SubjectControlCharacter);
    }
    Ok(sub.to_owned())
}

/// Returns the audience list (1-8 unique strings, containing the client ID)
/// so [`validate_azp`] can apply its own rule based on how many there are.
fn validate_audience<'a>(
    claims: &'a VerifiedIdTokenClaims,
    expected_client_id: &str,
) -> Result<Vec<&'a str>, IdentityClaimsError> {
    let value = claims
        .claim("aud")
        .ok_or(IdentityClaimsError::AudienceMissing)?;
    let audiences: Vec<&str> = match value {
        serde_json::Value::String(s) => vec![s.as_str()],
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .ok_or(IdentityClaimsError::AudienceInvalidShape)
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(IdentityClaimsError::AudienceInvalidShape),
    };
    if audiences.is_empty() {
        return Err(IdentityClaimsError::AudienceEmpty);
    }
    if audiences.len() > 8 {
        return Err(IdentityClaimsError::AudienceTooMany);
    }
    let mut seen = std::collections::HashSet::with_capacity(audiences.len());
    for a in &audiences {
        if !seen.insert(*a) {
            return Err(IdentityClaimsError::AudienceDuplicateValue);
        }
    }
    if !audiences.contains(&expected_client_id) {
        return Err(IdentityClaimsError::AudienceDoesNotContainClientId);
    }
    Ok(audiences)
}

/// RFC 096 `:656-659`'s `azp` rule: required and exact when `audiences` has
/// more than one value; optional but still exact when present for one.
fn validate_azp(
    claims: &VerifiedIdTokenClaims,
    audiences: &[&str],
    expected_client_id: &str,
) -> Result<(), IdentityClaimsError> {
    let azp_claim = claims.claim("azp");
    if audiences.len() > 1 && azp_claim.is_none() {
        return Err(IdentityClaimsError::AzpMissing);
    }
    let Some(azp_value) = azp_claim else {
        return Ok(());
    };
    let azp = azp_value
        .as_str()
        .ok_or(IdentityClaimsError::AzpNotString)?;
    if azp != expected_client_id {
        return Err(IdentityClaimsError::AzpMismatch);
    }
    Ok(())
}

/// Validates the four required identity claims against the caller-supplied
/// expected issuer and client ID. No lookup, no mutation: both are
/// parameters, exactly as `verify_id_token_against_jwks` takes the JWKS as a
/// parameter.
pub fn validate_identity_claims(
    claims: &VerifiedIdTokenClaims,
    expected_issuer: &str,
    expected_client_id: &str,
) -> Result<RequiredIdentityClaims, IdentityClaimsError> {
    validate_issuer(claims, expected_issuer)?;
    let sub = validate_subject(claims)?;
    let audiences = validate_audience(claims, expected_client_id)?;
    validate_azp(claims, &audiences, expected_client_id)?;
    Ok(RequiredIdentityClaims { sub })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "identity_claims/tests.rs"]
mod tests;
