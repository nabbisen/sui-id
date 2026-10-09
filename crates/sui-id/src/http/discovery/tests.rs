use super::*;

const ISSUER: &str = "https://idp.example.com";

/// Every stage 8 metadata field, filled with values that satisfy every
/// unconditional rule in `validate` -- tests focused on the three
/// endpoints (this module's original scope) build on this rather than
/// repeating all eight fields in every test.
fn raw(auth: &str, token: &str, userinfo: Option<&str>) -> RawDiscovery {
    RawDiscovery {
        issuer: ISSUER.to_string(),
        authorization_endpoint: auth.into(),
        token_endpoint: token.into(),
        userinfo_endpoint: userinfo.map(str::to_owned),
        jwks_uri: None,
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: None,
        code_challenge_methods_supported: vec!["S256".to_string()],
        authorization_response_iss_parameter_supported: true,
        token_endpoint_auth_methods_supported: vec!["client_secret_basic".to_string()],
        id_token_signing_alg_values_supported: vec!["RS256".to_string()],
        subject_types_supported: vec!["public".to_string()],
    }
}

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

/// Stage 8-fix: the explicit-`:443`-as-endpoint form this test used to use
/// is now itself rejected as noncanonical (RFC 096 `:553`) before origin
/// comparison ever runs -- it is the dispatch's own first example. The
/// port-equivalence this test actually proves still holds and is tested
/// from the only side the canonical check doesn't reach: an
/// *admin-configured* `allowed_origins` entry written with an explicit
/// `:443` (never passed through `check_endpoint`) still matches a
/// canonical, no-port endpoint, because `url::Origin`'s own equality
/// compares resolved port numbers, not raw strings.
#[test]
fn an_allowed_origin_with_an_explicit_443_matches_a_canonical_no_port_endpoint() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "https://idp.example.com:443").is_ok());
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

// ---- RFC 096-A stage 3a: jwks_uri ------------------------------------------

fn with_jwks(jwks: Option<&str>) -> RawDiscovery {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.jwks_uri = jwks.map(str::to_owned);
    doc
}

/// Production guard: a document without `jwks_uri` must still deserialize and
/// validate. Making the field required would fail every provider that omits it,
/// on the live path, which RFC 096-A is not allowed to change.
#[test]
fn a_document_without_jwks_uri_still_deserializes_and_validates() {
    let body = r#"{
        "issuer": "https://idp.example.com",
        "authorization_endpoint": "https://idp.example.com/authorize",
        "token_endpoint": "https://idp.example.com/token",
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"]
    }"#;
    let raw: RawDiscovery = serde_json::from_str(body).expect("absent jwks_uri must deserialize");
    let doc = ValidatedDiscovery::validate(raw, ISSUER, "").expect("and validate");
    assert_eq!(doc.jwks_uri(), None);
}

#[test]
fn a_valid_https_jwks_uri_in_the_issuer_origin_is_accepted() {
    let doc =
        ValidatedDiscovery::validate(with_jwks(Some("https://idp.example.com/jwks")), ISSUER, "")
            .expect("a valid jwks_uri");
    assert_eq!(doc.jwks_uri(), Some("https://idp.example.com/jwks"));
}

#[test]
fn a_jwks_uri_that_is_not_https_is_rejected() {
    let result =
        ValidatedDiscovery::validate(with_jwks(Some("http://idp.example.com/jwks")), ISSUER, "");
    assert!(matches!(
        result,
        Err(DiscoveryError::NotHttps {
            field: "jwks_uri",
            ..
        })
    ));
}

#[test]
fn a_jwks_uri_outside_the_allowed_origins_is_rejected() {
    let result = ValidatedDiscovery::validate(
        with_jwks(Some("https://keys.evil.example/jwks")),
        ISSUER,
        "",
    );
    assert!(matches!(
        result,
        Err(DiscoveryError::OriginNotAllowed {
            field: "jwks_uri",
            ..
        })
    ));
}

#[test]
fn a_relative_jwks_uri_is_rejected_as_not_absolute() {
    let result = ValidatedDiscovery::validate(with_jwks(Some("/jwks")), ISSUER, "");
    assert!(matches!(
        result,
        Err(DiscoveryError::NotAbsolute {
            field: "jwks_uri",
            ..
        })
    ));
}

// ---- RFC 096-A stage 8: the eleven-row metadata table ---------------------

#[test]
fn an_issuer_that_does_not_match_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.issuer = "https://a-different-issuer.example.com".to_string();
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::IssuerMismatch)
    ));
}

#[test]
fn a_matching_issuer_is_accepted() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

#[test]
fn response_types_supported_missing_code_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.response_types_supported = vec!["id_token".to_string()];
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::ResponseTypesMissingCode)
    ));
}

#[test]
fn response_types_supported_with_code_among_extras_is_accepted() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.response_types_supported = vec!["id_token".to_string(), "code".to_string()];
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

#[test]
fn an_absent_grant_types_supported_is_not_an_error() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.grant_types_supported = None;
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

#[test]
fn a_present_grant_types_supported_missing_authorization_code_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.grant_types_supported = Some(vec!["implicit".to_string()]);
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::GrantTypesMissingAuthorizationCode)
    ));
}

#[test]
fn a_present_grant_types_supported_with_authorization_code_is_accepted() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.grant_types_supported = Some(vec!["authorization_code".to_string()]);
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

#[test]
fn code_challenge_methods_supported_missing_s256_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.code_challenge_methods_supported = vec!["plain".to_string()];
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::CodeChallengeMethodsMissingS256)
    ));
}

#[test]
fn code_challenge_methods_supported_with_s256_is_accepted() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

#[test]
fn authorization_response_iss_parameter_supported_false_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.authorization_response_iss_parameter_supported = false;
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::AuthorizationResponseIssNotTrue)
    ));
}

#[test]
fn authorization_response_iss_parameter_supported_true_is_accepted() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

/// `authorization_response_iss_parameter_supported` is declared `bool` in
/// `RawDiscovery`, the same convention every pre-existing field in this
/// struct already uses (a wrong-shaped `authorization_endpoint` has never
/// had its own named `DiscoveryError` either): a non-boolean value fails
/// deserialization generically, before a `RawDiscovery` -- and so a
/// `DiscoveryError::AuthorizationResponseIssNotTrue` -- can even exist.
/// Documented here as a test, not left to be rediscovered as a gap.
#[test]
fn authorization_response_iss_parameter_supported_wrong_type_fails_deserialization_generically() {
    let body = r#"{
        "issuer": "https://idp.example.com",
        "authorization_endpoint": "https://idp.example.com/authorize",
        "token_endpoint": "https://idp.example.com/token",
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": "true",
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"]
    }"#;
    assert!(serde_json::from_str::<RawDiscovery>(body).is_err());
}

#[test]
fn subject_types_supported_missing_public_is_rejected() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.subject_types_supported = vec!["pairwise".to_string()];
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::SubjectTypesMissingPublic)
    ));
}

#[test]
fn subject_types_supported_with_public_is_accepted() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

// ---- the two deferred cross-checks ----------------------------------------

#[test]
fn supports_token_endpoint_auth_method_true_for_a_contained_method() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    let validated = ValidatedDiscovery::validate(doc, ISSUER, "").expect("ok");
    assert!(validated.supports_token_endpoint_auth_method("client_secret_basic"));
}

#[test]
fn supports_token_endpoint_auth_method_false_for_an_uncontained_method() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    let validated = ValidatedDiscovery::validate(doc, ISSUER, "").expect("ok");
    assert!(!validated.supports_token_endpoint_auth_method("none"));
}

#[test]
fn id_token_algs_intersection_keeps_only_the_shared_algorithms_in_configured_order() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.id_token_signing_alg_values_supported = vec!["RS256".to_string(), "ES256".to_string()];
    let validated = ValidatedDiscovery::validate(doc, ISSUER, "").expect("ok");
    let configured = vec![
        "PS256".to_string(),
        "ES256".to_string(),
        "RS256".to_string(),
    ];
    assert_eq!(
        validated.id_token_algs_intersection(&configured),
        vec!["ES256".to_string(), "RS256".to_string()]
    );
}

#[test]
fn id_token_algs_intersection_is_empty_when_nothing_overlaps() {
    let mut doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    doc.id_token_signing_alg_values_supported = vec!["HS256".to_string()];
    let validated = ValidatedDiscovery::validate(doc, ISSUER, "").expect("ok");
    let configured = vec!["RS256".to_string()];
    assert!(validated.id_token_algs_intersection(&configured).is_empty());
}

// ---- canonical-URL checks on every endpoint -------------------------------

#[test]
fn a_token_endpoint_with_embedded_credentials_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://user:pass@idp.example.com/token",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::HasUserinfo {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_token_endpoint_with_only_a_username_is_still_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://user@idp.example.com/token",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::HasUserinfo {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_token_endpoint_with_a_query_string_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token?a=b",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::HasQuery {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_token_endpoint_with_a_fragment_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token#frag",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::HasFragment {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_canonical_token_endpoint_is_accepted() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/token",
        None,
    );
    assert!(ValidatedDiscovery::validate(doc, ISSUER, "").is_ok());
}

// ---- RFC 096-A stage 8-fix: RFC 096 `:553`'s "noncanonical URL" -----------

#[test]
fn a_token_endpoint_with_a_redundant_default_port_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com:443/token",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::Noncanonical {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_token_endpoint_with_a_non_lowercase_host_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://IDP.EXAMPLE.COM/token",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::Noncanonical {
            field: "token_endpoint"
        })
    ));
}

#[test]
fn a_token_endpoint_with_a_dot_dot_path_segment_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com/a/../token",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::Noncanonical {
            field: "token_endpoint"
        })
    ));
}

/// The decided edge: `url` canonicalizes a bare origin to add a trailing
/// slash, so a declared endpoint with no path at all counts as
/// noncanonical too -- no special-cased normalization, per
/// `check_endpoint`'s own doc comment.
#[test]
fn a_bare_origin_token_endpoint_with_no_trailing_slash_is_rejected() {
    let doc = raw(
        "https://idp.example.com/authorize",
        "https://idp.example.com",
        None,
    );
    assert!(matches!(
        ValidatedDiscovery::validate(doc, ISSUER, ""),
        Err(DiscoveryError::Noncanonical {
            field: "token_endpoint"
        })
    ));
}

// ---- RFC 096-A stage 8: validate_discovery_bytes --------------------------

fn full_document_json() -> String {
    r#"{
        "issuer": "https://idp.example.com",
        "authorization_endpoint": "https://idp.example.com/authorize",
        "token_endpoint": "https://idp.example.com/token",
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"]
    }"#
    .to_string()
}

#[test]
fn validate_discovery_bytes_accepts_a_well_formed_document() {
    let bytes = full_document_json();
    assert!(validate_discovery_bytes(bytes.as_bytes(), ISSUER, "").is_ok());
}

#[test]
fn validate_discovery_bytes_rejects_bytes_that_are_not_json() {
    let bytes = b"not json";
    assert!(matches!(
        validate_discovery_bytes(bytes, ISSUER, ""),
        Err(DiscoveryError::NotJson)
    ));
}

#[test]
fn validate_discovery_bytes_rejects_a_body_over_the_shared_byte_cap() {
    let oversized = "a".repeat(response_bounds::MAX_RESPONSE_BYTES + 1);
    assert!(matches!(
        validate_discovery_bytes(oversized.as_bytes(), ISSUER, ""),
        Err(DiscoveryError::Bounds(
            response_bounds::BoundsError::TooLarge { .. }
        ))
    ));
}

/// The discovery-specific array bound (32) is four times tighter than the
/// shared JWKS/transport bound (128) -- this array of 33 elements would
/// pass `response_bounds::ResponseCaps::JWKS_AND_TRANSPORT` but must fail
/// `ResponseCaps::DISCOVERY`, proving the tighter cap is actually wired
/// in, not just declared.
#[test]
fn validate_discovery_bytes_rejects_an_array_over_32_elements_even_though_jwks_would_allow_it() {
    let extras: Vec<String> = (0..33).map(|i| format!("\"extra{i}\"")).collect();
    let body = format!(
        r#"{{
            "issuer": "https://idp.example.com",
            "authorization_endpoint": "https://idp.example.com/authorize",
            "token_endpoint": "https://idp.example.com/token",
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
            "authorization_response_iss_parameter_supported": true,
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "subject_types_supported": ["public"],
            "some_extension_array": [{}]
        }}"#,
        extras.join(",")
    );
    assert!(matches!(
        validate_discovery_bytes(body.as_bytes(), ISSUER, ""),
        Err(DiscoveryError::Bounds(
            response_bounds::BoundsError::ArrayTooLong { limit: 32 }
        ))
    ));
    // Confirm the premise: the same array passes the looser shared cap.
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        response_bounds::check_caps(&value, &response_bounds::ResponseCaps::JWKS_AND_TRANSPORT)
            .is_ok()
    );
}

/// The discovery-specific string bound (2,048 bytes) is four times
/// tighter than the shared bound (8,192) -- same proof shape as the array
/// case above.
#[test]
fn validate_discovery_bytes_rejects_a_string_over_2048_bytes_even_though_jwks_would_allow_it() {
    let long = "a".repeat(2_049);
    let body = format!(
        r#"{{
            "issuer": "https://idp.example.com",
            "authorization_endpoint": "https://idp.example.com/authorize",
            "token_endpoint": "https://idp.example.com/token",
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
            "authorization_response_iss_parameter_supported": true,
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "subject_types_supported": ["public"],
            "some_extension_string": "{long}"
        }}"#
    );
    assert!(matches!(
        validate_discovery_bytes(body.as_bytes(), ISSUER, ""),
        Err(DiscoveryError::Bounds(
            response_bounds::BoundsError::StringTooLong { limit: 2_048 }
        ))
    ));
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        response_bounds::check_caps(&value, &response_bounds::ResponseCaps::JWKS_AND_TRANSPORT)
            .is_ok()
    );
}

#[test]
fn validate_discovery_bytes_rejects_nesting_deeper_than_16() {
    // Seventeen nested single-key objects, each wrapping the next, plus
    // the required top-level discovery fields alongside the deep one.
    let mut deep = "0".to_string();
    for i in 0..17 {
        deep = format!("{{\"l{i}\":{deep}}}");
    }
    let body = format!(
        r#"{{
            "issuer": "https://idp.example.com",
            "authorization_endpoint": "https://idp.example.com/authorize",
            "token_endpoint": "https://idp.example.com/token",
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
            "authorization_response_iss_parameter_supported": true,
            "token_endpoint_auth_methods_supported": ["client_secret_basic"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "subject_types_supported": ["public"],
            "deep": {deep}
        }}"#
    );
    assert!(matches!(
        validate_discovery_bytes(body.as_bytes(), ISSUER, ""),
        Err(DiscoveryError::TooDeep { limit: 16 })
    ));
}

/// "Duplicate keys at any depth are rejected" (RFC 096 `:532`) -- this
/// duplicate is nested two levels down, inside `"deep"`, not at the top
/// level, proving the check is genuinely recursive rather than only
/// covering the document's own top-level members (stage 6a's payload
/// scan, by contrast, is deliberately top-level only).
#[test]
fn validate_discovery_bytes_rejects_a_duplicate_key_nested_inside_another_object() {
    let body = r#"{
        "issuer": "https://idp.example.com",
        "authorization_endpoint": "https://idp.example.com/authorize",
        "token_endpoint": "https://idp.example.com/token",
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "authorization_response_iss_parameter_supported": true,
        "token_endpoint_auth_methods_supported": ["client_secret_basic"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "subject_types_supported": ["public"],
        "deep": {"inner": {"a": 1, "b": 2, "a": 3}}
    }"#;
    assert!(matches!(
        validate_discovery_bytes(body.as_bytes(), ISSUER, ""),
        Err(DiscoveryError::DuplicateMember(ref name)) if name == "a"
    ));
}
