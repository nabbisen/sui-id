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

use serde::Deserialize;

/// The discovery document as deserialized from the upstream response,
/// before any check has run. Never exposed outside this module —
/// `fetch_discovery` deserializes into this and immediately hands it to
/// [`ValidatedDiscovery::validate`].
#[derive(Deserialize)]
pub struct RawDiscovery {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    #[serde(default)]
    pub jwks_uri: Option<String>,
}

/// Why an issuer or an endpoint was rejected. Carries enough detail for a
/// server-side log line (provider slug, field name, offending origin);
/// callers must never put this `Display` output in front of the browser —
/// the offending value is attacker-influenced content.
#[derive(Debug)]
pub enum DiscoveryError {
    /// The issuer itself does not parse as an absolute `https` URL.
    InvalidIssuer(String),
    /// An endpoint does not parse as an absolute URL.
    NotAbsolute { field: &'static str, value: String },
    /// An endpoint parses but its scheme is not `https`.
    NotHttps { field: &'static str, value: String },
    /// An endpoint's origin is not in the provider's allowed set.
    OriginNotAllowed { field: &'static str, origin: String },
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
            Self::OriginNotAllowed { field, origin } => {
                write!(f, "{field}'s origin {origin:?} is not in the allowed set")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// A discovery document whose three endpoints have each been checked:
/// parses as an absolute URL, scheme is `https`, and the origin (scheme,
/// host, and port, with default ports normalized by [`url::Url::origin`])
/// is in the provider's allowed set. No public field, no other
/// constructor — see the module doc comment.
#[derive(Debug)]
pub struct ValidatedDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: Option<String>,
    jwks_uri: Option<String>,
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

        Ok(Self {
            authorization_endpoint,
            token_endpoint,
            userinfo_endpoint,
            jwks_uri,
        })
    }
}

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
