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
