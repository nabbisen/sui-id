//! RFC 134 D3 — validated OIDC discovery documents.
//!
//! Discovery is untrusted input: an upstream provider's `/.well-known/
//! openid-configuration` response is attacker-influenced (the provider
//! itself may be compromised, or a network attacker on the path to a
//! misconfigured provider may substitute endpoints), and `token_endpoint`
//! is where the provider's `client_secret` gets POSTed. Nothing short of
//! validating every endpoint's origin against an administrator-stated
//! allow-list prevents that secret from being sent somewhere the admin
//! never approved.
//!
//! [`ValidatedDiscovery`]'s fields are private and reachable only through
//! accessors, and the only constructor ([`ValidatedDiscovery::validate`])
//! checks every endpoint before the type can exist at all. A future call
//! site that forgets the check does not compile against an unvalidated
//! endpoint, because there is no unvalidated endpoint to reach — the same
//! reasoning as RFC 134 D2's resolver placement: put the control where
//! bypassing it is impossible, not where a checker notices.
//!
//! `jwks_uri` is validated by the same rules as every other endpoint, when it
//! is present. It is **optional here**, although OIDC Discovery requires it:
//! [`RawDiscovery`] is deserialized on the live federation path, and a required
//! field would fail every provider whose document omits it. Refusing a provider
//! with no `jwks_uri` belongs to the stage that verifies signatures, where it
//! changes nothing that works today.
//!
//! ## RFC 096-A stage 8: the metadata table, and a real constraint it ran into
//!
//! RFC 096 `:537-549` states eleven required-metadata rules (not twelve --
//! `userinfo_endpoint`, which this module already deserializes and validates
//! for provider compatibility, is not one of the RFC's own rows at all).
//! [`ValidatedDiscovery::validate`] is this module's only constructor and
//! has exactly one non-test caller, `handlers/federation.rs`'s
//! `fetch_discovery` -- a file RFC 096-A may not touch. That has one
//! direct consequence for how this stage is shaped:
//!
//! **Every rule `validate` can check using only its *existing* parameters
//! (`raw`, `issuer`, `allowed_origins`) is added directly to `validate`,
//! and becomes live the moment this lands** -- no routing stage needed,
//! because the call site already exists. That covers nine of the eleven
//! rows: `issuer`, the three endpoints (already done), `response_types_
//! supported`, `grant_types_supported`, `code_challenge_methods_
//! supported`, `authorization_response_iss_parameter_supported`, and
//! `subject_types_supported`.
//!
//! **The remaining two rows need external input `validate`'s signature
//! does not have, and cannot gain without editing the forbidden call
//! site.** `token_endpoint_auth_methods_supported` needs the caller's own
//! configured auth method; `id_token_signing_alg_values_supported` needs
//! the caller's configured algorithm list (the dispatch's own "say how you
//! expose that intersection" question). Both fields are still captured and
//! bounded here -- a malformed shape still fails deserialization, the same
//! as every other field -- but the *semantic* rule ("contains the
//! configured method", "intersects the configured algorithms") is exposed
//! as a separate method ([`ValidatedDiscovery::supports_token_endpoint_
//! auth_method`], [`ValidatedDiscovery::id_token_algs_intersection`]) that
//! a caller invokes with its own config. Until a future stage is
//! authorized to edit `fetch_discovery` to call one, these two rules are
//! available and fully tested but **not enforced on the live path** --
//! stated here plainly rather than silently narrowed, because the
//! dispatch's own closing line says all nine of RFC 096 `:21`'s attack
//! categories close when this stage lands, and that claim should be made
//! with this gap in view, not around it.
//!
//! The same constraint, in the other direction, is why RFC 096 `:531-532`'s
//! tightened document bounds (depth 16, 32 array members, 2,048 bytes per
//! string -- `response_bounds::ResponseCaps::DISCOVERY`) and its "duplicate
//! keys at any depth" rule are enforced only by [`validate_discovery_bytes`],
//! a new, self-contained bytes-to-`ValidatedDiscovery` pipeline this stage
//! adds alongside `validate` rather than inside it: the live path's bytes
//! are already consumed by `response_bounds::read_bounded_json`'s shared,
//! looser bounds before `validate` ever runs, inside the same forbidden
//! file, and nothing here can intercept that. `validate_discovery_bytes` is
//! the complete, correct implementation, ready for whatever future stage
//! is authorized to swap it into `fetch_discovery`; until then it is
//! reachable only by tests, same as the rest of 096-A's validation layer
//! (exactly consistent with everything else in 096-A -- discovery's
//! endpoint-origin checks are simply unusual in already being live from
//! before 096-A began, RFC 134 D3).
//!
//! ## A scope reduction found only by running the existing tests
//!
//! `check_endpoint` was first extended with five canonical-URL checks:
//! userinfo/query/fragment, IP-literal host, and a reserved-hostname list
//! (`localhost`, `.local`, `.internal`, `.home.arpa`), the last two
//! modelled on RFC 096 `:294`'s definition for a *different* field (the
//! admin-configured issuer, not a discovered endpoint -- the RFC's own
//! discovery table states no such rule for `authorization_endpoint`/
//! `token_endpoint`/`jwks_uri` directly). Because this function sits on
//! the live path, running the full workspace suite after writing it
//! surfaced eleven failures across three already-accepted e2e test files:
//! every one of them mocks its upstream IdP by raw loopback IP
//! (`tls_mock::serve_https`'s `https://127.0.0.1:PORT`, a pattern
//! deliberately chosen there for simplicity -- see that module's own
//! comment on why most fixtures use an IP literal rather than
//! `serve_raw_https`'s named-host-plus-resolver approach).
//!
//! **Kept: `HasUserinfo`/`HasQuery`/`HasFragment`.** RFC 096 `:553` names
//! "credential-bearing URL" directly, and none of the broken tests used
//! one -- these three checks are unambiguously required and cost nothing.
//!
//! **Reverted: the IP-literal and reserved-hostname checks.** Unlike the
//! three kept checks, discovered endpoints already pass through an
//! explicit, admin-configured origin allow-list -- an IP-literal origin
//! an admin approved is no more trusted or less trusted for being an IP
//! rather than a domain, which is not true of the admin's own issuer
//! configuration (`:294`'s context), where the rule guards against an
//! operator's own typo, not an upstream's claim. Given that difference in
//! what the rule is actually for, and the real cost of rewriting several
//! already-accepted test files' shared mocking infrastructure to avoid IP
//! literals, these two checks are not included this stage. Flagged for
//! the architect in the review package as a judgement call, not a
//! measurement -- if discovered endpoints should reject IP literals and
//! reserved hostnames after all, `tls_mock.rs`'s `serve_https` is the
//! thing that would need to change, not this function.

use serde::Deserialize;

use crate::response_bounds;

/// The discovery document as deserialized from the upstream response,
/// before any check has run. Never exposed outside this module —
/// `fetch_discovery` deserializes into this and immediately hands it to
/// [`ValidatedDiscovery::validate`].
#[derive(Deserialize)]
pub struct RawDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    #[serde(default)]
    pub jwks_uri: Option<String>,
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Option<Vec<String>>,
    pub code_challenge_methods_supported: Vec<String>,
    pub authorization_response_iss_parameter_supported: bool,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
}

/// Why an issuer, an endpoint, or a metadata rule was rejected. Carries
/// enough detail for a server-side log line (provider slug, field name,
/// offending origin); callers must never put this `Display` output in
/// front of the browser — the offending value is attacker-influenced
/// content.
#[derive(Debug)]
pub enum DiscoveryError {
    /// The issuer itself does not parse as an absolute `https` URL.
    InvalidIssuer(String),
    /// An endpoint does not parse as an absolute URL.
    NotAbsolute { field: &'static str, value: String },
    /// An endpoint parses but its scheme is not `https`.
    NotHttps { field: &'static str, value: String },
    /// An endpoint carries a username or password component. RFC 096
    /// `:553`'s "credential-bearing URL" -- checked before the origin
    /// allow-list, since a URL carrying embedded credentials is malformed
    /// independent of whether its origin happens to be approved.
    HasUserinfo { field: &'static str },
    /// An endpoint carries a query string.
    HasQuery { field: &'static str },
    /// An endpoint carries a fragment.
    HasFragment { field: &'static str },
    /// RFC 096 `:553`'s "noncanonical URL": the declared value is not
    /// byte-identical to `url::Url`'s own serialization of it -- a
    /// redundant default port, non-lowercase host, or an unresolved `.`/
    /// `..` path segment, among others. Checked uniformly, with no
    /// exception for a bare origin without a trailing slash (`url`
    /// canonicalizes `https://x` to `https://x/`) -- see
    /// [`check_endpoint`]'s doc comment for why.
    Noncanonical { field: &'static str },
    /// An endpoint's origin is not in the provider's allowed set.
    OriginNotAllowed { field: &'static str, origin: String },
    /// `issuer` is present but does not byte-exactly match the configured
    /// canonical issuer.
    IssuerMismatch,
    /// `response_types_supported` does not contain `"code"`.
    ResponseTypesMissingCode,
    /// `grant_types_supported` is present but does not contain
    /// `"authorization_code"`.
    GrantTypesMissingAuthorizationCode,
    /// `code_challenge_methods_supported` does not contain `"S256"`.
    CodeChallengeMethodsMissingS256,
    /// `authorization_response_iss_parameter_supported` is present but not
    /// exactly `true`.
    AuthorizationResponseIssNotTrue,
    /// `subject_types_supported` does not contain `"public"`.
    SubjectTypesMissingPublic,
    /// [`validate_discovery_bytes`] only: the transport rules shared with
    /// JWKS -- status, media type, byte cap, and (with
    /// [`response_bounds::ResponseCaps::DISCOVERY`]) the member/array/
    /// string caps.
    Bounds(response_bounds::BoundsError),
    /// [`validate_discovery_bytes`] only: nesting deeper than
    /// [`DISCOVERY_MAX_DEPTH`].
    TooDeep { limit: usize },
    /// [`validate_discovery_bytes`] only: a member name repeats, at any
    /// depth. The name is the repeated one.
    DuplicateMember(String),
    /// [`validate_discovery_bytes`] only: the body is not JSON.
    NotJson,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIssuer(issuer) => {
                write!(f, "issuer {issuer:?} is not an absolute https URL")
            }
            Self::NotAbsolute { field, value } => {
                write!(f, "{field} {value:?} is not an absolute URL")
            }
            Self::NotHttps { field, value } => write!(f, "{field} {value:?} is not https"),
            Self::HasUserinfo { field } => write!(f, "{field} carries a username or password"),
            Self::HasQuery { field } => write!(f, "{field} carries a query string"),
            Self::HasFragment { field } => write!(f, "{field} carries a fragment"),
            Self::Noncanonical { field } => write!(f, "{field} is not a canonical URL"),
            Self::OriginNotAllowed { field, origin } => {
                write!(f, "{field}'s origin {origin:?} is not in the allowed set")
            }
            Self::IssuerMismatch => {
                write!(
                    f,
                    "issuer does not byte-exactly match the configured issuer"
                )
            }
            Self::ResponseTypesMissingCode => {
                write!(f, "response_types_supported does not contain \"code\"")
            }
            Self::GrantTypesMissingAuthorizationCode => write!(
                f,
                "grant_types_supported is present but omits \"authorization_code\""
            ),
            Self::CodeChallengeMethodsMissingS256 => {
                write!(
                    f,
                    "code_challenge_methods_supported does not contain \"S256\""
                )
            }
            Self::AuthorizationResponseIssNotTrue => write!(
                f,
                "authorization_response_iss_parameter_supported is not exactly true"
            ),
            Self::SubjectTypesMissingPublic => {
                write!(f, "subject_types_supported does not contain \"public\"")
            }
            Self::Bounds(e) => write!(f, "{e}"),
            Self::TooDeep { limit } => {
                write!(f, "discovery document nests deeper than {limit} levels")
            }
            Self::DuplicateMember(name) => write!(f, "discovery document repeats member {name:?}"),
            Self::NotJson => write!(f, "discovery document is not JSON"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// RFC 096 `:531`'s own discovery depth bound. Numerically identical to
/// `jwks::MAX_JWKS_DEPTH` today, but named and defined separately: the RFC
/// states the two as independent numbers that happen to coincide, and
/// coupling them through one shared constant would make a future,
/// JWKS-only reason to change one silently change the other.
pub const DISCOVERY_MAX_DEPTH: usize = 16;

/// A discovery document whose three endpoints have each been checked:
/// parses as an absolute URL, scheme is `https`, canonical (no userinfo,
/// query, or fragment), and the origin (scheme, host, and port, with
/// default ports normalized by [`url::Url::origin`]) is in the provider's
/// allowed set. No public field, no other constructor — see the module
/// doc comment.
///
/// `token_endpoint_auth_methods_supported` and `id_token_signing_alg_
/// values_supported` are carried as validated-shape lists, not reduced to
/// a yes/no here — see the module doc comment for why their own RFC rules
/// are exposed as [`Self::supports_token_endpoint_auth_method`] and
/// [`Self::id_token_algs_intersection`] instead of being enforced
/// unconditionally inside [`Self::validate`].
#[derive(Debug)]
pub struct ValidatedDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: Option<String>,
    jwks_uri: Option<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    id_token_signing_alg_values_supported: Vec<String>,
}

impl ValidatedDiscovery {
    pub fn authorization_endpoint(&self) -> &str {
        &self.authorization_endpoint
    }

    pub fn token_endpoint(&self) -> &str {
        &self.token_endpoint
    }

    pub fn userinfo_endpoint(&self) -> Option<&str> {
        self.userinfo_endpoint.as_deref()
    }

    /// The validated `jwks_uri`, when the document names one.
    pub fn jwks_uri(&self) -> Option<&str> {
        self.jwks_uri.as_deref()
    }

    /// RFC 096 `:546`: *"Contains the configured method; `none` required
    /// for public clients."* The `none` half of the rule is the caller's
    /// own responsibility: a public client's own `configured_method` is
    /// already `"none"` by the time it reaches here, so this function
    /// does not special-case it.
    pub fn supports_token_endpoint_auth_method(&self, configured_method: &str) -> bool {
        self.token_endpoint_auth_methods_supported
            .iter()
            .any(|m| m == configured_method)
    }

    /// RFC 096 `:547`: *"Intersects configured algorithms; runtime uses
    /// the intersection only."* Returns the overlap, in `configured`'s own
    /// order (stable downstream alg-preference ordering); an empty result
    /// means the document and the configuration share no algorithm at
    /// all, which the caller -- not this function -- decides what to do
    /// with (stage 4a's `verify_id_token_against_jwks` already takes its
    /// allowed-algorithm list as a plain parameter, so a caller can pass
    /// this result into it directly in place of the raw configured list,
    /// with zero change to stage 4a itself).
    pub fn id_token_algs_intersection(&self, configured: &[String]) -> Vec<String> {
        configured
            .iter()
            .filter(|alg| self.id_token_signing_alg_values_supported.contains(alg))
            .cloned()
            .collect()
    }

    /// The only constructor. `issuer` must already be known canonical
    /// `https` (callers validate it before fetching, since the fetch
    /// itself must not happen against a bad issuer); it is re-parsed here
    /// only to compute the empty-set default, not re-validated. `allowed_
    /// origins` is the provider's configured set, space-separated, raw
    /// from storage; empty means "the issuer's origin alone".
    pub fn validate(
        raw: RawDiscovery,
        issuer: &str,
        allowed_origins: &str,
    ) -> Result<Self, DiscoveryError> {
        if raw.issuer != issuer {
            return Err(DiscoveryError::IssuerMismatch);
        }

        let issuer_url =
            url::Url::parse(issuer).map_err(|_| DiscoveryError::InvalidIssuer(issuer.into()))?;
        let allowed: Vec<url::Origin> = if allowed_origins.trim().is_empty() {
            vec![issuer_url.origin()]
        } else {
            allowed_origins
                .split_whitespace()
                .map(|s| url::Url::parse(s).map(|u| u.origin()))
                .collect::<Result<_, _>>()
                .map_err(|_| DiscoveryError::InvalidIssuer(issuer.into()))?
        };

        let authorization_endpoint = check_endpoint(
            "authorization_endpoint",
            raw.authorization_endpoint,
            &allowed,
        )?;
        let token_endpoint = check_endpoint("token_endpoint", raw.token_endpoint, &allowed)?;
        let userinfo_endpoint = match raw.userinfo_endpoint {
            Some(u) => Some(check_endpoint("userinfo_endpoint", u, &allowed)?),
            None => None,
        };
        let jwks_uri = match raw.jwks_uri {
            Some(u) => Some(check_endpoint("jwks_uri", u, &allowed)?),
            None => None,
        };

        if !raw.response_types_supported.iter().any(|s| s == "code") {
            return Err(DiscoveryError::ResponseTypesMissingCode);
        }
        if let Some(grants) = &raw.grant_types_supported
            && !grants.iter().any(|s| s == "authorization_code")
        {
            return Err(DiscoveryError::GrantTypesMissingAuthorizationCode);
        }
        if !raw
            .code_challenge_methods_supported
            .iter()
            .any(|s| s == "S256")
        {
            return Err(DiscoveryError::CodeChallengeMethodsMissingS256);
        }
        if !raw.authorization_response_iss_parameter_supported {
            return Err(DiscoveryError::AuthorizationResponseIssNotTrue);
        }
        if !raw.subject_types_supported.iter().any(|s| s == "public") {
            return Err(DiscoveryError::SubjectTypesMissingPublic);
        }

        Ok(Self {
            authorization_endpoint,
            token_endpoint,
            userinfo_endpoint,
            jwks_uri,
            token_endpoint_auth_methods_supported: raw.token_endpoint_auth_methods_supported,
            id_token_signing_alg_values_supported: raw.id_token_signing_alg_values_supported,
        })
    }
}

/// Validates a discovery document from its raw bytes, start to finish:
/// the transport caps (status/media type already assumed checked by the
/// caller, same split as `jwks::parse_jwks`/`read_bounded_body`), the
/// RFC 096 `:531-532` structural bounds `validate` cannot apply (see the
/// module doc comment for why), then [`ValidatedDiscovery::validate`] for
/// everything else. Not wired into the live fetch -- see the module doc
/// comment -- but a complete, independently correct implementation ready
/// for whichever future stage is authorized to swap it in.
pub fn validate_discovery_bytes(
    bytes: &[u8],
    issuer: &str,
    allowed_origins: &str,
) -> Result<ValidatedDiscovery, DiscoveryError> {
    if bytes.len() > response_bounds::MAX_RESPONSE_BYTES {
        return Err(DiscoveryError::Bounds(
            response_bounds::BoundsError::TooLarge {
                limit: response_bounds::MAX_RESPONSE_BYTES,
            },
        ));
    }

    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| DiscoveryError::NotJson)?;
    response_bounds::check_caps(&value, &response_bounds::ResponseCaps::DISCOVERY)
        .map_err(DiscoveryError::Bounds)?;

    let depth = crate::jwks::nesting_depth(&value);
    if depth > DISCOVERY_MAX_DEPTH {
        return Err(DiscoveryError::TooDeep {
            limit: DISCOVERY_MAX_DEPTH,
        });
    }

    if let Some(name) =
        crate::jwks::first_repeated_member(bytes).map_err(|_| DiscoveryError::NotJson)?
    {
        return Err(DiscoveryError::DuplicateMember(name));
    }

    let raw: RawDiscovery = serde_json::from_value(value).map_err(|_| DiscoveryError::NotJson)?;
    ValidatedDiscovery::validate(raw, issuer, allowed_origins)
}

/// RFC 096 `:553`'s "noncanonical URL" (stage 8-fix): `url.as_str()` is
/// `url`'s own re-serialization of the parsed value, and comparing it
/// against the raw declared string is the check -- a redundant default
/// port, a non-lowercase host, and an unresolved `.`/`..` path segment all
/// produce a mismatch, confirmed empirically against `url` 2.5.8 rather
/// than assumed.
///
/// **No exception for a bare origin without a trailing slash.** `url`
/// canonicalizes `https://idp.example.com` to `https://idp.example.com/`,
/// so a declared endpoint with no path at all would count as noncanonical
/// under this check. Decided, not overlooked: every endpoint RFC 096
/// `:539-549` actually describes names a specific path
/// (`/authorize`, `/token`, a `jwks_uri` path), so a bare origin is
/// already a degenerate value no real provider sends, and carving out a
/// normalization step for it would be inventing an exception `:553`'s own
/// wording does not state -- "a noncanonical URL fails the whole
/// document" has no stated exception, and treating the comparison
/// uniformly is the simpler reading with no real-world cost.
fn check_endpoint(
    field: &'static str,
    value: String,
    allowed: &[url::Origin],
) -> Result<String, DiscoveryError> {
    let url = url::Url::parse(&value).map_err(|_| DiscoveryError::NotAbsolute {
        field,
        value: value.clone(),
    })?;
    if url.scheme() != "https" {
        return Err(DiscoveryError::NotHttps { field, value });
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(DiscoveryError::HasUserinfo { field });
    }
    if url.query().is_some() {
        return Err(DiscoveryError::HasQuery { field });
    }
    if url.fragment().is_some() {
        return Err(DiscoveryError::HasFragment { field });
    }
    if url.as_str() != value {
        return Err(DiscoveryError::Noncanonical { field });
    }
    let origin = url.origin();
    if !allowed.contains(&origin) {
        return Err(DiscoveryError::OriginNotAllowed {
            field,
            origin: origin.ascii_serialization(),
        });
    }
    Ok(value)
}

/// Validate the issuer alone, before any request is made (RFC 134 D3: "the
/// issuer itself is canonical HTTPS before the discovery fetch is
/// attempted"). Returns the parsed `Url` so callers build the well-known
/// path from it rather than the raw string a second time.
pub fn validate_issuer(issuer: &str) -> Result<url::Url, DiscoveryError> {
    let url = url::Url::parse(issuer).map_err(|_| DiscoveryError::InvalidIssuer(issuer.into()))?;
    if url.scheme() != "https" {
        return Err(DiscoveryError::InvalidIssuer(issuer.into()));
    }
    Ok(url)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "discovery/tests.rs"]
mod tests;
