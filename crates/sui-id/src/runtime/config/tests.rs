use super::*;

#[test]
fn sample_validates() {
    Config::sample().validate().expect("sample is valid");
}

#[test]
fn negative_lifetime_is_rejected() {
    let mut c = Config::sample();
    c.tokens.access_lifetime_secs = 0;
    assert!(c.validate().is_err());
}

#[test]
fn refresh_must_exceed_access() {
    let mut c = Config::sample();
    c.tokens.access_lifetime_secs = 100;
    c.tokens.refresh_lifetime_secs = 50;
    assert!(c.validate().is_err());
}

#[test]
fn issuer_must_be_absolute() {
    let mut c = Config::sample();
    c.server.issuer = "/not-absolute".into();
    assert!(c.validate().is_err());
}

/// RFC 098 dispatch 12a: a `[tokens]` table that sets one lifetime keeps the
/// documented defaults for the other two. Before per-field defaults this
/// failed with `missing field`, including the configuration reference's own
/// production example.
#[test]
fn partial_tokens_table_keeps_the_other_defaults() {
    let toml = r#"
[server]
listen_addr = "127.0.0.1:8801"
issuer = "https://id.example.com"

[storage]
db_path = "./sui-id.sqlite"
key_file = "./sui-id.key"

[tokens]
refresh_lifetime_secs = 86400
"#;
    let cfg: Config = toml::from_str(toml).expect("a partial [tokens] table parses");
    assert_eq!(cfg.tokens.refresh_lifetime_secs, 86400);
    assert_eq!(cfg.tokens.access_lifetime_secs, 900);
    assert_eq!(cfg.tokens.id_token_lifetime_secs, 900);
    cfg.validate().expect("and validates");
}

fn a_federation_provider() -> FederationProviderConfig {
    FederationProviderConfig {
        slug: "idp".into(),
        display_name: "IdP".into(),
        issuer: "https://idp.example.com".into(),
        client_id: "client".into(),
        client_secret_env: String::new(),
        scopes: "openid email".into(),
        provision_mode: "link_only".into(),
        enabled: true,
        allowed_origins: Vec::new(),
    }
}

/// RFC 134 D3: "Cap the set at 8 ... Reject a longer list at config load
/// with a clear error, not at first sign-in."
#[test]
fn nine_allowed_origins_fails_at_load_not_first_sign_in() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.allowed_origins = (0..9)
        .map(|i| format!("https://origin-{i}.example.com"))
        .collect();
    c.federation_providers.push(provider);
    let err = c.validate().expect_err("9 origins must be rejected");
    let msg = err.to_string();
    assert!(msg.contains('9'), "{msg}");
    assert!(
        msg.contains("idp"),
        "{msg}: must name the offending provider"
    );
}

#[test]
fn eight_allowed_origins_is_accepted() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.allowed_origins = (0..8)
        .map(|i| format!("https://origin-{i}.example.com"))
        .collect();
    c.federation_providers.push(provider);
    c.validate().expect("8 origins is within RFC 096's cap");
}

#[test]
fn a_non_https_allowed_origin_is_rejected_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.allowed_origins = vec!["http://origin.example.com".into()];
    c.federation_providers.push(provider);
    assert!(c.validate().is_err());
}

#[test]
fn an_unparseable_allowed_origin_is_rejected_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.allowed_origins = vec!["not a url".into()];
    c.federation_providers.push(provider);
    assert!(c.validate().is_err());
}
