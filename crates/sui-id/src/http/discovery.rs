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
//! `jwks_uri` is deliberately out of scope: [`RawDiscovery`] does not
//! carry it, nothing fetches it today, and adding it now would be
//! speculative. When JWKS verification arrives it inherits this same
//! validation by construction.

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

        Ok(Self {
            authorization_endpoint,
            token_endpoint,
            userinfo_endpoint,
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
mod tests {
    use super::*;

    fn raw(auth: &str, token: &str, userinfo: Option<&str>) -> RawDiscovery {
        RawDiscovery {
            authorization_endpoint: auth.into(),
            token_endpoint: token.into(),
            userinfo_endpoint: userinfo.map(str::to_owned),
        }
    }

    const ISSUER: &str = "https://idp.example.com";

    #[test]
    fn empty_allowed_origins_defaults_to_the_issuer_origin() {
        let good = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com/token",
            None,
        );
        assert!(ValidatedDiscovery::validate(good, ISSUER, "").is_ok());

        let bad = raw(
            "https://idp.example.com/authorize",
            "https://evil.example.com/token",
            None,
        );
        assert!(ValidatedDiscovery::validate(bad, ISSUER, "").is_err());
    }

    #[test]
    fn an_explicit_allowed_origin_is_required_even_for_the_issuer() {
        // Non-empty set: the issuer's own origin is NOT implicitly
        // included -- the admin must list everything they want allowed.
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com/token",
            None,
        );
        let result = ValidatedDiscovery::validate(doc, ISSUER, "https://oauth2.example.com");
        assert!(result.is_err());
    }

    #[test]
    fn token_endpoint_outside_the_set_is_rejected_independently() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://evil.example.com/token",
            None,
        );
        let err = ValidatedDiscovery::validate(doc, ISSUER, "").expect_err("must reject");
        assert!(matches!(
            err,
            DiscoveryError::OriginNotAllowed {
                field: "token_endpoint",
                ..
            }
        ));
    }

    #[test]
    fn authorization_endpoint_outside_the_set_is_rejected_independently() {
        let doc = raw(
            "https://evil.example.com/authorize",
            "https://idp.example.com/token",
            None,
        );
        let err = ValidatedDiscovery::validate(doc, ISSUER, "").expect_err("must reject");
        assert!(matches!(
            err,
            DiscoveryError::OriginNotAllowed {
                field: "authorization_endpoint",
                ..
            }
        ));
    }

    #[test]
    fn userinfo_endpoint_outside_the_set_is_rejected_independently() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com/token",
            Some("https://evil.example.com/userinfo"),
        );
        let err = ValidatedDiscovery::validate(doc, ISSUER, "").expect_err("must reject");
        assert!(matches!(
            err,
            DiscoveryError::OriginNotAllowed {
                field: "userinfo_endpoint",
                ..
            }
        ));
    }

    #[test]
    fn an_http_endpoint_is_rejected_even_when_its_host_is_in_the_set() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "http://idp.example.com/token",
            None,
        );
        let err = ValidatedDiscovery::validate(doc, ISSUER, "").expect_err("must reject");
        assert!(matches!(
            err,
            DiscoveryError::NotHttps {
                field: "token_endpoint",
                ..
            }
        ));
    }

    #[test]
    fn a_relative_endpoint_is_rejected_as_not_absolute() {
        let doc = raw("https://idp.example.com/authorize", "/token", None);
        let err = ValidatedDiscovery::validate(doc, ISSUER, "").expect_err("must reject");
        assert!(matches!(
            err,
            DiscoveryError::NotAbsolute {
                field: "token_endpoint",
                ..
            }
        ));
    }

    #[test]
    fn default_https_port_and_explicit_443_are_the_same_origin() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com:443/token",
            None,
        );
        assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
    }

    #[test]
    fn a_non_default_port_is_a_different_origin() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com:8443/token",
            None,
        );
        assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_err());
    }

    #[test]
    fn a_configured_second_origin_is_accepted() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://oauth2.example.com/token",
            None,
        );
        let allowed = "https://idp.example.com https://oauth2.example.com";
        assert!(ValidatedDiscovery::validate(doc, ISSUER, allowed).is_ok());
    }

    #[test]
    fn accessors_return_what_was_validated() {
        let doc = raw(
            "https://idp.example.com/authorize",
            "https://idp.example.com/token",
            Some("https://idp.example.com/userinfo"),
        );
        let validated = ValidatedDiscovery::validate(doc, ISSUER, "").expect("ok");
        assert_eq!(
            validated.authorization_endpoint(),
            "https://idp.example.com/authorize"
        );
        assert_eq!(validated.token_endpoint(), "https://idp.example.com/token");
        assert_eq!(
            validated.userinfo_endpoint(),
            Some("https://idp.example.com/userinfo")
        );
    }

    #[test]
    fn validate_issuer_accepts_canonical_https() {
        assert!(validate_issuer("https://idp.example.com").is_ok());
    }

    #[test]
    fn validate_issuer_rejects_http() {
        assert!(validate_issuer("http://idp.example.com").is_err());
    }

    #[test]
    fn validate_issuer_rejects_garbage() {
        assert!(validate_issuer("not a url").is_err());
    }
}
