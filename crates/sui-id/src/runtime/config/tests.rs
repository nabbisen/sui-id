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
        id_token_algs: default_id_token_algs(),
    }
}

/// RFC 134 D3: "Cap the set at 8 ... Reject a longer list at config load
/// with a clear error, not at first sign-in."
#[test]
fn nine_allowed_origins_fails_at_load_not_first_sign_in() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    // RFC 096-B2 S2a: the issuer's own origin must be one of the set --
    // included here (plus 8 others, 9 total) to isolate the count rule
    // this test is actually about from the newer "issuer included" one.
    provider.allowed_origins = std::iter::once("https://idp.example.com".to_string())
        .chain((0..8).map(|i| format!("https://origin-{i}.example.com")))
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
    // RFC 096-B2 S2a: the issuer's own origin must be one of the set --
    // included here (plus 7 others, 8 total) to isolate the count rule
    // this test is actually about from the newer "issuer included" one.
    provider.allowed_origins = std::iter::once("https://idp.example.com".to_string())
        .chain((0..7).map(|i| format!("https://origin-{i}.example.com")))
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

/// RFC 096 Provider trust configuration, `id_token_algs` row. Each refusal below
/// is one row of RFC 096's Provider trust configuration: refuse to start,
/// never fall back to the default.
fn config_with_algs(algs: &[&str]) -> Config {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.id_token_algs = algs.iter().map(|s| s.to_string()).collect();
    c.federation_providers.push(provider);
    c
}

fn rejected_message(algs: &[&str]) -> String {
    config_with_algs(algs)
        .validate()
        .expect_err("must be refused at load")
        .to_string()
}

#[test]
fn an_empty_id_token_algs_list_is_refused_and_says_omit_for_rs256() {
    let msg = rejected_message(&[]);
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("configuration error"), "{msg}");
    assert!(
        msg.contains("RS256"),
        "{msg}: must say omitting the key gives RS256"
    );
}

#[test]
fn more_than_four_id_token_algs_is_refused_with_count_and_maximum() {
    let msg = rejected_message(&["RS256", "PS256", "ES256", "EdDSA", "RS256"]);
    assert!(msg.contains("idp"), "{msg}");
    assert!(msg.contains("5 entries"), "{msg}");
    assert!(msg.contains("maximum of 4"), "{msg}");
}

#[test]
fn a_permitted_outside_entry_is_refused_with_the_value_and_the_four_permitted() {
    for bad in ["RS384", "ES384", "rs256", "RSA"] {
        let msg = rejected_message(&[bad]);
        assert!(msg.contains(&format!("{bad:?}")), "{msg}: must quote {bad}");
        assert!(msg.contains("RS256, PS256, ES256, EdDSA"), "{msg}");
    }
}

#[test]
fn every_hs_algorithm_is_refused_and_the_reason_is_given() {
    for hs in ["HS256", "HS384", "HS512"] {
        let msg = rejected_message(&[hs]);
        assert!(msg.contains(hs), "{msg}");
        assert!(msg.contains("symmetric"), "{msg}: must say why");
        assert!(msg.contains("algorithm-confusion"), "{msg}: must say why");
        assert!(msg.contains("RS256, PS256, ES256, EdDSA"), "{msg}");
    }
}

#[test]
fn none_is_refused_and_the_reason_is_given() {
    let msg = rejected_message(&["none"]);
    assert!(msg.contains("\"none\""), "{msg}");
    assert!(
        msg.contains("unsigned"),
        "{msg}: must say an unsigned token is never acceptable"
    );
}

#[test]
fn a_duplicate_entry_is_refused_and_named() {
    let msg = rejected_message(&["RS256", "RS256"]);
    assert!(msg.contains("\"RS256\" twice"), "{msg}");
    assert!(msg.contains("set"), "{msg}: must say it is a set");
}

#[test]
fn the_default_applies_when_the_key_is_omitted() {
    let body = r#"
slug = "idp"
display_name = "IdP"
issuer = "https://idp.example.com"
client_id = "client"
"#;
    let provider: FederationProviderConfig = toml::from_str(body).expect("parses without the key");
    assert_eq!(provider.id_token_algs, vec!["RS256".to_string()]);
    let mut c = Config::sample();
    c.federation_providers.push(provider);
    c.validate()
        .expect("omitted key is the valid RS256 default");
}

#[test]
fn a_singleton_rs256_list_is_accepted() {
    config_with_algs(&["RS256"])
        .validate()
        .expect("singleton RS256 is valid");
}

#[test]
fn a_four_element_list_is_accepted() {
    config_with_algs(&["RS256", "PS256", "ES256", "EdDSA"])
        .validate()
        .expect("all four permitted algorithms together are valid");
}

// ── RFC 096-B2 S2a: newly enforced rules, via `FederationTrustPolicy`
// (`Config::validate` wires every one of these up; `federation_policy`'s
// own test module is the exhaustive per-rule suite for each) ───────────

#[test]
fn a_noncanonical_issuer_is_refused_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.issuer = "https://Idp.example.com".into(); // mixed-case host
    c.federation_providers.push(provider);
    let msg = c
        .validate()
        .expect_err("mixed-case issuer host")
        .to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("issuer"), "{msg}");
}

/// RFC 096-B2 S6b: the issuer host rule, wired through the same
/// `Config::validate` path as every other `FederationTrustPolicy` rule --
/// an operator who points `issuer` at an internal IdP by its loopback or
/// `.internal` name learns this at startup, not only when a user tries to
/// sign in.
#[test]
fn an_issuer_host_the_host_rule_refuses_is_refused_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.issuer = "https://idp.internal".into();
    c.federation_providers.push(provider);
    let msg = c
        .validate()
        .expect_err("an .internal issuer host must refuse to start")
        .to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("issuer host rule"), "{msg}");
}

#[test]
fn an_empty_client_id_is_refused_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.client_id = String::new();
    c.federation_providers.push(provider);
    let msg = c.validate().expect_err("empty client_id").to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("client_id"), "{msg}");
}

#[test]
fn scopes_missing_openid_is_refused_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.scopes = "email profile".into();
    c.federation_providers.push(provider);
    let msg = c.validate().expect_err("scopes without openid").to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("scopes"), "{msg}");
}

#[test]
fn an_unknown_provision_mode_is_refused_at_load_not_silently_link_only() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    provider.provision_mode = "provision_on_every_login".into(); // not a real value
    c.federation_providers.push(provider);
    let msg = c
        .validate()
        .expect_err("unknown provision_mode must refuse, not silently become link_only")
        .to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("provision_mode"), "{msg}");
}

#[test]
fn an_explicit_origin_set_missing_the_issuers_own_origin_is_refused_at_load() {
    let mut c = Config::sample();
    let mut provider = a_federation_provider();
    // The issuer is https://idp.example.com; this set never names it.
    provider.allowed_origins = vec!["https://other.example.com".into()];
    c.federation_providers.push(provider);
    let msg = c
        .validate()
        .expect_err("an explicit origin set missing the issuer's own origin must refuse")
        .to_string();
    assert!(msg.contains("idp"), "{msg}: must name the provider");
    assert!(msg.contains("issuer"), "{msg}");
}

/// S5b review: a slug is a URL path segment, so it is held to what the
/// configuration reference documents: lowercase alphanumeric with optional
/// hyphens. The login page's button links to it.
#[test]
fn a_slug_that_is_not_url_safe_is_refused_at_load() {
    for bad in [
        "",
        "has space",
        "a/b",
        "q?x",
        "dot.dot",
        "Google",
        "under_score",
        &"x".repeat(65),
    ] {
        let mut c = Config::sample();
        let mut provider = a_federation_provider();
        provider.slug = bad.to_string();
        c.federation_providers.push(provider);
        let msg = c.validate().expect_err(bad).to_string();
        assert!(msg.contains("slug"), "{bad:?}: {msg}");
    }
    for good in ["google", "entra-id", "idp2", "x"] {
        let mut c = Config::sample();
        let mut provider = a_federation_provider();
        provider.slug = good.to_string();
        c.federation_providers.push(provider);
        assert!(c.validate().is_ok(), "{good:?} must be accepted");
    }
}

/// S5b review: two blocks may not share a slug; the second would fail to
/// be created at startup, with nothing saying why.
#[test]
fn a_slug_used_by_two_blocks_is_refused_at_load() {
    let mut c = Config::sample();
    c.federation_providers.push(a_federation_provider());
    c.federation_providers.push(a_federation_provider());
    let msg = c.validate().expect_err("duplicate slug").to_string();
    assert!(msg.contains("more than one"), "{msg}");
}
