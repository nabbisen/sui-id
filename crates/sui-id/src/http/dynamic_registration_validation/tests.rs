use super::*;

// ── 1b: the envelope ─────────────────────────────────────────────────────

fn obj(json: &str) -> &[u8] {
    json.as_bytes()
}

#[test]
fn a_well_formed_object_is_accepted() {
    let map = parse_envelope(obj(r#"{"redirect_uris":["https://rp.example/cb"]}"#)).unwrap();
    assert_eq!(map.len(), 1);
}

#[test]
fn too_large_a_body_is_rejected() {
    let padding = "x".repeat(MAX_ENVELOPE_BYTES + 1);
    let body = format!(r#"{{"client_name":"{padding}"}}"#);
    assert!(matches!(
        parse_envelope(body.as_bytes()),
        Err(EnvelopeError::TooLarge)
    ));
}

#[test]
fn exactly_the_byte_limit_is_not_rejected_for_being_large() {
    // Pad to exactly the limit; must not trip `TooLarge` (it may still
    // fail on member count/depth if it did, but it does not here).
    let overhead = r#"{"client_name":""}"#.len();
    let padding = "x".repeat(MAX_ENVELOPE_BYTES - overhead);
    let body = format!(r#"{{"client_name":"{padding}"}}"#);
    assert_eq!(body.len(), MAX_ENVELOPE_BYTES);
    assert!(!matches!(
        parse_envelope(body.as_bytes()),
        Err(EnvelopeError::TooLarge)
    ));
}

#[test]
fn a_top_level_array_is_rejected_as_not_an_object() {
    assert!(matches!(
        parse_envelope(obj(r#"[1,2,3]"#)),
        Err(EnvelopeError::NotAnObject)
    ));
}

#[test]
fn a_top_level_string_is_rejected_as_not_an_object() {
    assert!(matches!(
        parse_envelope(obj(r#""hello""#)),
        Err(EnvelopeError::NotAnObject)
    ));
}

#[test]
fn malformed_json_is_rejected() {
    assert!(matches!(
        parse_envelope(obj(r#"{"redirect_uris": ["#)),
        Err(EnvelopeError::Malformed(_))
    ));
}

#[test]
fn a_duplicate_declared_member_is_rejected() {
    let err = parse_envelope(obj(
        r#"{"client_name":"a","client_name":"b","redirect_uris":[]}"#,
    ))
    .unwrap_err();
    assert!(matches!(err, EnvelopeError::DuplicateMember(ref k) if k == "client_name"));
}

/// The matrix says "duplicate member", unqualified -- not just a
/// duplicate of a field this deployment happens to model. A plain
/// `serde_json::Value` parse would silently last-key-win this; this
/// is exactly what [`DuplicateCheckedObject`] exists to catch instead.
#[test]
fn a_duplicate_unknown_member_is_rejected_not_silently_overwritten() {
    let err = parse_envelope(obj(
        r#"{"totally_unknown_field":"a","totally_unknown_field":"b"}"#,
    ))
    .unwrap_err();
    assert!(matches!(err, EnvelopeError::DuplicateMember(ref k) if k == "totally_unknown_field"));
}

#[test]
fn too_many_top_level_members_is_rejected() {
    let members: String = (0..MAX_ENVELOPE_MEMBERS + 1)
        .map(|i| format!(r#""k{i}":true"#))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!("{{{members}}}");
    assert!(matches!(
        parse_envelope(body.as_bytes()),
        Err(EnvelopeError::TooManyMembers)
    ));
}

#[test]
fn exactly_the_member_limit_is_accepted() {
    let members: String = (0..MAX_ENVELOPE_MEMBERS)
        .map(|i| format!(r#""k{i}":true"#))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!("{{{members}}}");
    assert!(parse_envelope(body.as_bytes()).is_ok());
}

fn nested(depth: usize) -> String {
    let mut s = String::from("1");
    for _ in 0..depth {
        s = format!("[{s}]");
    }
    format!(r#"{{"k":{s}}}"#)
}

#[test]
fn too_deep_a_body_is_rejected() {
    // The outer object is depth 1; `k`'s value nested `MAX_ENVELOPE_DEPTH`
    // arrays deep makes the whole document `MAX_ENVELOPE_DEPTH + 1`.
    let body = nested(MAX_ENVELOPE_DEPTH);
    assert!(matches!(
        parse_envelope(body.as_bytes()),
        Err(EnvelopeError::TooDeep)
    ));
}

#[test]
fn exactly_the_depth_limit_is_accepted() {
    let body = nested(MAX_ENVELOPE_DEPTH - 1);
    assert!(parse_envelope(body.as_bytes()).is_ok());
}

#[test]
fn software_statement_is_rejected_with_its_own_error_not_invalid_client_metadata() {
    assert!(matches!(
        parse_envelope(obj(r#"{"software_statement":"ignored.jwt.value"}"#)),
        Err(EnvelopeError::UnapprovedSoftwareStatement)
    ));
}

#[test]
fn a_known_unsupported_member_is_rejected_by_name() {
    let err = parse_envelope(obj(r#"{"jwks_uri":"https://rp.example/jwks"}"#)).unwrap_err();
    assert!(matches!(err, EnvelopeError::UnsupportedMember(ref k) if k == "jwks_uri"));
}

#[test]
fn every_supported_member_is_accepted_by_name() {
    for member in SUPPORTED_MEMBERS {
        let body = format!(r#"{{"{member}":null}}"#);
        let err = parse_envelope(body.as_bytes());
        assert!(
            !matches!(err, Err(EnvelopeError::UnsupportedMember(_))),
            "{member} must not be treated as unsupported"
        );
    }
}

#[test]
fn an_unknown_extension_member_is_ignored_not_rejected() {
    let map = parse_envelope(obj(
        r#"{"redirect_uris":["https://rp.example/cb"],"x_totally_custom":"whatever"}"#,
    ))
    .unwrap();
    // `parse_envelope` itself does not drop it -- that happens at the
    // typed-struct conversion step, which simply never reads it. Prove
    // that conversion step's own behaviour here with a tiny local
    // struct, independent of the real `RegistrationRequest` (which
    // lives in the handler module, deliberately not depended on from
    // here).
    #[derive(serde::Deserialize)]
    struct Minimal {
        redirect_uris: Vec<String>,
    }
    let typed: Minimal = serde_json::from_value(serde_json::Value::Object(map)).unwrap();
    assert_eq!(typed.redirect_uris, vec!["https://rp.example/cb"]);
}

#[test]
fn a_wrong_scalar_type_fails_at_the_typed_conversion_step() {
    #[derive(serde::Deserialize)]
    struct Minimal {
        #[allow(dead_code)]
        redirect_uris: Vec<String>,
    }
    let map = parse_envelope(obj(r#"{"redirect_uris": 42}"#)).unwrap();
    assert!(serde_json::from_value::<Minimal>(serde_json::Value::Object(map)).is_err());
}

// ── bearer token format ──────────────────────────────────────────────────

fn headers_with(values: &[&str]) -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    for v in values {
        h.append(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(v).unwrap(),
        );
    }
    h
}

const VALID_TOKEN: &str = "abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab\
                                abababab";

#[test]
fn a_well_formed_bearer_token_is_extracted() {
    assert_eq!(
        VALID_TOKEN.len(),
        64,
        "test fixture must itself be 64 chars"
    );
    let h = headers_with(&[&format!("Bearer {VALID_TOKEN}")]);
    assert_eq!(
        extract_registration_bearer_token(&h),
        Some(VALID_TOKEN.to_string())
    );
}

#[test]
fn a_missing_authorization_header_yields_none() {
    let h = headers_with(&[]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

#[test]
fn an_oversized_token_is_rejected() {
    let too_long = format!("{VALID_TOKEN}0");
    let h = headers_with(&[&format!("Bearer {too_long}")]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

#[test]
fn a_non_hex_token_is_rejected() {
    let not_hex = "g".repeat(64);
    let h = headers_with(&[&format!("Bearer {not_hex}")]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

#[test]
fn an_uppercase_token_is_rejected() {
    let upper = VALID_TOKEN.to_uppercase();
    let h = headers_with(&[&format!("Bearer {upper}")]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

#[test]
fn multiple_authorization_headers_are_rejected() {
    let h = headers_with(&[
        &format!("Bearer {VALID_TOKEN}"),
        &format!("Bearer {VALID_TOKEN}"),
    ]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

#[test]
fn a_wrong_scheme_is_rejected() {
    let h = headers_with(&[&format!("Basic {VALID_TOKEN}")]);
    assert_eq!(extract_registration_bearer_token(&h), None);
}

// ── 1a: the derived closed profile ───────────────────────────────────────

#[test]
fn confidential_https_is_accepted() {
    let uris = vec!["https://rp.example/cb".to_string()];
    assert_eq!(
        derive_closed_profile("client_secret_post", &uris),
        Ok(RedirectProfile::ConfidentialHttps)
    );
    assert_eq!(
        derive_closed_profile("client_secret_basic", &uris),
        Ok(RedirectProfile::ConfidentialHttps)
    );
}

#[test]
fn public_https_is_accepted() {
    let uris = vec!["https://rp.example/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicHttps)
    );
}

#[test]
fn public_native_loopback_ipv4_is_accepted() {
    let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicNativeLoopback)
    );
}

#[test]
fn public_native_loopback_ipv6_is_accepted() {
    let uris = vec!["http://[::1]:49152/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicNativeLoopback)
    );
}

#[test]
fn confidential_with_any_http_loopback_is_rejected() {
    let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("client_secret_post", &uris),
        Err(RedirectRejection::ConfidentialWithLoopback)
    ));
}

#[test]
fn mixed_https_and_loopback_is_rejected() {
    let uris = vec![
        "https://rp.example/cb".to_string(),
        "http://127.0.0.1:49152/cb".to_string(),
    ];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::MixedProfile)
    ));
}

#[test]
fn named_localhost_is_rejected_not_treated_as_numeric_loopback() {
    let uris = vec!["http://localhost:49152/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn http_on_a_remote_or_private_host_is_rejected() {
    for host in ["http://rp.example/cb", "http://10.0.0.5:8080/cb"] {
        let uris = vec![host.to_string()];
        assert!(
            matches!(
                derive_closed_profile("none", &uris),
                Err(RedirectRejection::Malformed { .. })
            ),
            "{host} must be rejected"
        );
    }
}

#[test]
fn loopback_without_an_explicit_port_is_rejected() {
    let uris = vec!["http://127.0.0.1/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn loopback_with_port_zero_is_rejected() {
    let uris = vec!["http://127.0.0.1:0/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn a_fragment_is_rejected() {
    let uris = vec!["https://rp.example/cb#frag".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn userinfo_is_rejected() {
    let uris = vec!["https://user:pass@rp.example/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn a_wildcard_pattern_is_rejected() {
    let uris = vec!["https://rp.example/*".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn a_custom_scheme_is_rejected() {
    for uri in [
        "file:///etc/passwd",
        "data:text/plain,hi",
        "javascript:alert(1)",
        "custom-scheme://foo/cb",
    ] {
        let uris = vec![uri.to_string()];
        assert!(
            matches!(
                derive_closed_profile("none", &uris),
                Err(RedirectRejection::Malformed { .. })
            ),
            "{uri} must be rejected"
        );
    }
}

#[test]
fn a_unicode_hostname_is_rejected() {
    let uris = vec!["https://例え.jp/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn canonical_ascii_punycode_is_accepted() {
    let uris = vec!["https://xn--r8jz45g.jp/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicHttps)
    );
}

#[test]
fn a_malformed_percent_escape_is_rejected() {
    let uris = vec!["https://rp.example/cb%".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn a_backslash_is_rejected() {
    let uris = vec!["https://rp.example\\evil.com/cb".to_string()];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn an_explicit_default_port_is_rejected() {
    for uri in ["https://rp.example:443/cb", "http://127.0.0.1:80/cb"] {
        let uris = vec![uri.to_string()];
        assert!(
            matches!(
                derive_closed_profile("none", &uris),
                Err(RedirectRejection::Malformed { .. })
            ),
            "{uri} must be rejected"
        );
    }
}

#[test]
fn a_non_default_explicit_port_on_https_is_still_accepted() {
    // The default-port check must not become "reject any explicit port".
    let uris = vec!["https://rp.example:8443/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicHttps)
    );
}

#[test]
fn an_exact_duplicate_in_one_list_is_rejected() {
    let uris = vec![
        "https://rp.example/cb".to_string(),
        "https://rp.example/cb".to_string(),
    ];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::DuplicateRedirect(_))
    ));
}

#[test]
fn a_uri_over_2048_bytes_is_rejected() {
    let long_path = "a".repeat(MAX_REDIRECT_URI_BYTES);
    let uri = format!("https://rp.example/{long_path}");
    assert!(uri.len() > MAX_REDIRECT_URI_BYTES);
    let uris = vec![uri];
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::Malformed { .. })
    ));
}

#[test]
fn a_list_over_16_entries_is_rejected() {
    let uris: Vec<String> = (0..MAX_REDIRECT_URIS + 1)
        .map(|i| format!("https://rp.example/cb{i}"))
        .collect();
    assert!(matches!(
        derive_closed_profile("none", &uris),
        Err(RedirectRejection::TooManyRedirects)
    ));
}

#[test]
fn exactly_16_entries_is_accepted() {
    let uris: Vec<String> = (0..MAX_REDIRECT_URIS)
        .map(|i| format!("https://rp.example/cb{i}"))
        .collect();
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicHttps)
    );
}

/// Public-native authorization redirects plus an independent HTTPS
/// logout URI: the authorization profile is unchanged by what
/// `post_logout_redirect_uris` contains, since the two are validated
/// independently.
#[test]
fn a_native_loopback_profile_is_unaffected_by_an_independent_https_logout_uri() {
    let uris = vec!["http://127.0.0.1:49152/cb".to_string()];
    assert_eq!(
        derive_closed_profile("none", &uris),
        Ok(RedirectProfile::PublicNativeLoopback)
    );
    assert!(reject_http_post_logout_uris(&["https://rp.example/logout".to_string()]).is_ok());
}

#[test]
fn any_http_post_logout_uri_is_rejected_including_numeric_loopback() {
    assert!(reject_http_post_logout_uris(&["http://rp.example/logout".to_string()]).is_err());
    assert!(reject_http_post_logout_uris(&["http://127.0.0.1:49152/logout".to_string()]).is_err());
}

#[test]
fn an_https_post_logout_uri_is_accepted() {
    assert!(reject_http_post_logout_uris(&["https://rp.example/logout".to_string()]).is_ok());
}

#[test]
fn an_empty_post_logout_list_is_accepted() {
    assert!(reject_http_post_logout_uris(&[]).is_ok());
}
