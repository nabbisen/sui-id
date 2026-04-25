//! OIDC Discovery document construction.
//!
//! Only the metadata sui-id actually supports is advertised. We deliberately
//! omit fields that would imply features we have not implemented (the spec
//! permits provider metadata to describe a subset).

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Discovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub end_session_endpoint: String,
    pub jwks_uri: String,

    pub response_types_supported: Vec<&'static str>,
    pub subject_types_supported: Vec<&'static str>,
    pub id_token_signing_alg_values_supported: Vec<&'static str>,
    pub token_endpoint_auth_methods_supported: Vec<&'static str>,
    pub grant_types_supported: Vec<&'static str>,
    pub code_challenge_methods_supported: Vec<&'static str>,
    pub scopes_supported: Vec<&'static str>,
}

impl Discovery {
    pub fn build(issuer: &str) -> Self {
        let trimmed = issuer.trim_end_matches('/');
        Self {
            issuer: trimmed.to_owned(),
            authorization_endpoint: format!("{trimmed}/oauth2/authorize"),
            token_endpoint: format!("{trimmed}/oauth2/token"),
            userinfo_endpoint: format!("{trimmed}/oauth2/userinfo"),
            end_session_endpoint: format!("{trimmed}/oauth2/logout"),
            jwks_uri: format!("{trimmed}/.well-known/jwks.json"),

            response_types_supported: vec!["code"],
            subject_types_supported: vec!["public"],
            id_token_signing_alg_values_supported: vec!["EdDSA"],
            token_endpoint_auth_methods_supported: vec!["client_secret_basic", "client_secret_post", "none"],
            grant_types_supported: vec!["authorization_code", "refresh_token"],
            code_challenge_methods_supported: vec!["S256"],
            scopes_supported: vec!["openid", "profile", "email", "offline_access"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_uses_issuer_without_trailing_slash() {
        let d = Discovery::build("https://idp.example.com/");
        assert_eq!(d.issuer, "https://idp.example.com");
        assert_eq!(d.token_endpoint, "https://idp.example.com/oauth2/token");
        assert_eq!(d.jwks_uri, "https://idp.example.com/.well-known/jwks.json");
    }

    #[test]
    fn implicit_and_hybrid_flows_are_not_advertised() {
        let d = Discovery::build("https://x");
        assert!(!d.response_types_supported.contains(&"id_token"));
        assert!(!d.response_types_supported.contains(&"token"));
        assert!(!d.response_types_supported.contains(&"code id_token"));
    }
}
