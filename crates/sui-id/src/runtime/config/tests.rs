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
