//! RFC 096-A stage 6c, half two: the construction capability, RFC 096
//! `:687-689` verbatim:
//!
//! > Successful validation returns a non-cloneable construction capability
//! > holding provider ID/version/activation generation, exact `sub`,
//! > verified email state, bounded display hints, and validation time. It
//! > contains no raw token, nonce, or upstream access token.
//!
//! A new sibling rather than a fifth thing bolted onto `identity_claims.rs`
//! or `optional_claims.rs`: this type's job -- being the one object the
//! rest of the system is allowed to hold once validation succeeds -- is
//! distinct from either module's job of producing one ingredient of it.
//!
//! **Why the constructor's parameter types are the real proof of
//! "no raw token, nonce, or upstream access token", not a runtime
//! assertion.** [`construct_identity_capability`] never takes a
//! `VerifiedIdTokenClaims`, a raw token `String`, or anything else wide
//! enough to carry one -- only [`RequiredIdentityClaims`] (holds `sub`
//! alone), [`ValidatedOptionalClaims`] (holds the two display hints and the
//! verified email), a [`CacheKey`], and a `DateTime<Utc>`. The nonce and the
//! raw token are not merely absent from the result; there is no parameter
//! they could have arrived through. A test still demonstrates this against
//! the real pipeline (signing a token whose nonce is a distinctive marker
//! and confirming it appears nowhere in the capability's `Debug` output),
//! but the type signature is what makes the absence structural rather than
//! a property that could regress.
//!
//! Reachable only by tests, same as the rest of 096-A until 096-B1 routes
//! live traffic through it.

use chrono::{DateTime, Utc};

use crate::cache_freshness::CacheKey;
use crate::identity_claims::RequiredIdentityClaims;
use crate::optional_claims::ValidatedOptionalClaims;

/// Sealed the same way as its three predecessors
/// (`VerifiedIdTokenClaims`, `RetainedEntry`, `RequiredIdentityClaims`):
/// private fields, no public constructor, the only way to hold one is
/// [`construct_identity_capability`]'s return value.
///
/// **No `Clone`, no `Copy`, on purpose** -- RFC 096 `:687`'s "non-cloneable".
/// Every field type here (`CacheKey`, `String`, `Option<String>`,
/// `DateTime<Utc>`) is itself `Clone`, so this is not a case of cloning
/// being impossible by accident; it is this type specifically declining to
/// derive it. A caller holding one cannot produce a second one via
/// `.clone()`, and cannot reassemble an equivalent one through the public
/// accessors either, because [`construct_identity_capability`] is the only
/// constructor and it is not part of this module's public re-export
/// surface for a reason the caller cannot route around: it requires a
/// [`RequiredIdentityClaims`] and a [`ValidatedOptionalClaims`], both
/// themselves sealed with no public constructor of their own, so a caller
/// without a byte-identical validation run has nothing to feed it.
///
/// **Hand-written `Debug`, not derived.** RFC 096 `:687`'s own nonce
/// framing is only the narrow case; `:707` is broader -- "raw email, `sub`,
/// ID token, or upstream error text never appears in log or metric
/// labels". A derived `Debug` would print `sub` and `verified_email`
/// verbatim the first time anyone wrote `{:?}` on this type in a log
/// statement, which is exactly `:707`'s forbidden case. `provider`
/// (opaque IDs, no PII), the two display hints (not named in `:707`), and
/// `validated_at` print plainly; `sub` and `verified_email` are redacted.
#[derive(PartialEq, Eq)]
pub struct IdentityCapability {
    provider: CacheKey,
    sub: String,
    verified_email: Option<String>,
    preferred_username: Option<String>,
    name: Option<String>,
    validated_at: DateTime<Utc>,
}

impl IdentityCapability {
    pub fn provider(&self) -> CacheKey {
        self.provider
    }

    pub fn sub(&self) -> &str {
        &self.sub
    }

    pub fn verified_email(&self) -> Option<&str> {
        self.verified_email.as_deref()
    }

    pub fn preferred_username(&self) -> Option<&str> {
        self.preferred_username.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn validated_at(&self) -> DateTime<Utc> {
        self.validated_at
    }
}

impl std::fmt::Debug for IdentityCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityCapability")
            .field("provider", &self.provider)
            .field("sub", &"[redacted, RFC 096 :707]")
            .field(
                "verified_email",
                &self
                    .verified_email
                    .as_ref()
                    .map(|_| "[redacted, RFC 096 :707]"),
            )
            .field("preferred_username", &self.preferred_username)
            .field("name", &self.name)
            .field("validated_at", &self.validated_at)
            .finish()
    }
}

/// The only way to produce an [`IdentityCapability`]. Takes the already-
/// validated pieces from `identity_claims` (6a) and `optional_claims` (6c
/// half one), plus the provider binding and the validation time as
/// parameters -- 096-A performs no durable mutation (RFC 096 `:63-66`), and
/// `validated_at` being a parameter rather than a clock read inside this
/// function is what lets it be tested deterministically (stage 6b found
/// what an in-function clock read does to a boundary test; this function
/// has no boundary to get wrong, but the same principle applies).
pub fn construct_identity_capability(
    identity: &RequiredIdentityClaims,
    optional: &ValidatedOptionalClaims,
    provider: CacheKey,
    validated_at: DateTime<Utc>,
) -> IdentityCapability {
    IdentityCapability {
        provider,
        sub: identity.sub().to_owned(),
        verified_email: optional.verified_email().map(str::to_owned),
        preferred_username: optional.preferred_username().map(str::to_owned),
        name: optional.name().map(str::to_owned),
        validated_at,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "identity_capability/tests.rs"]
mod tests;
