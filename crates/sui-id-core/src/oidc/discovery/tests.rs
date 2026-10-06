use super::*;

#[tokio::test]
async fn discovery_uses_issuer_without_trailing_slash() {
    let d = Discovery::build("https://idp.example.com/");
    assert_eq!(d.issuer, "https://idp.example.com");
    assert_eq!(d.token_endpoint, "https://idp.example.com/oauth2/token");
    assert_eq!(d.jwks_uri, "https://idp.example.com/.well-known/jwks.json");
}

#[tokio::test]
async fn implicit_and_hybrid_flows_are_not_advertised() {
    let d = Discovery::build("https://x");
    assert!(!d.response_types_supported.contains(&"id_token"));
    assert!(!d.response_types_supported.contains(&"token"));
    assert!(!d.response_types_supported.contains(&"code id_token"));
}
