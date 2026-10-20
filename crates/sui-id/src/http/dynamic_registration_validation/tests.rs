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
    assert!(
        validate_post_logout_redirect_uris(Some(&["https://rp.example/logout".to_string()]))
            .is_ok()
    );
}

#[test]
fn any_http_post_logout_uri_is_rejected_including_numeric_loopback() {
    assert!(
        validate_post_logout_redirect_uris(Some(&["http://rp.example/logout".to_string()]))
            .is_err()
    );
    assert!(
        validate_post_logout_redirect_uris(Some(&["http://127.0.0.1:49152/logout".to_string()]))
            .is_err()
    );
}

#[test]
fn an_https_post_logout_uri_is_accepted() {
    assert!(
        validate_post_logout_redirect_uris(Some(&["https://rp.example/logout".to_string()]))
            .is_ok()
    );
}

#[test]
fn an_empty_post_logout_list_is_accepted() {
    assert!(validate_post_logout_redirect_uris(Some(&[])).is_ok());
}

#[test]
fn an_absent_post_logout_list_is_accepted() {
    assert_eq!(validate_post_logout_redirect_uris(None), Ok(Vec::new()));
}

// ── 2: client_name ───────────────────────────────────────────────────────

/// `tag` makes every filler unique; padded with `'x'` to reach `width`
/// bytes exactly (ASCII, 1 byte per `char`, so `width` chars == `width`
/// bytes here).
fn ascii_filler(tag: char, width: usize) -> String {
    let mut s = String::with_capacity(width);
    s.push(tag);
    s.extend(std::iter::repeat_n('x', width - 1));
    s
}

#[test]
fn leading_and_trailing_whitespace_is_trimmed_and_accepted() {
    assert_eq!(
        validate_client_name(Some("  Example Inc.  ")),
        Ok("Example Inc.".to_string())
    );
}

#[test]
fn an_absent_name_is_rejected_as_blank() {
    assert_eq!(validate_client_name(None), Err(ClientNameRejection::Blank));
}

#[test]
fn a_whitespace_only_name_is_rejected_as_blank() {
    assert_eq!(
        validate_client_name(Some("   ")),
        Err(ClientNameRejection::Blank)
    );
}

#[test]
fn combining_characters_are_accepted() {
    // "e" + COMBINING ACUTE ACCENT (U+0301) -- two scalars, one grapheme.
    let name = "Caf\u{0301}e";
    assert!(validate_client_name(Some(name)).is_ok());
}

#[test]
fn emoji_are_accepted() {
    assert!(validate_client_name(Some("Rocket \u{1F680} App")).is_ok());
}

#[test]
fn html_is_accepted_not_rejected_as_a_shape() {
    // The corpus's own note: valid values are escaped by the UI and
    // bounded/sanitized before audit, so this validator's job is the
    // control-character floor, not content sanitisation.
    assert!(validate_client_name(Some("<b>Bold</b> Co")).is_ok());
}

#[test]
fn quotes_are_accepted() {
    assert!(validate_client_name(Some(r#"O'Brien's "App""#)).is_ok());
}

#[test]
fn a_pipe_character_is_accepted_not_treated_as_a_log_delimiter() {
    assert!(validate_client_name(Some("Name | With | Pipes")).is_ok());
}

#[test]
fn a_c0_control_character_is_rejected() {
    assert_eq!(
        validate_client_name(Some("Bad\u{0007}Name")),
        Err(ClientNameRejection::ForbiddenCharacter)
    );
}

#[test]
fn a_newline_is_rejected_as_a_c0_control() {
    assert_eq!(
        validate_client_name(Some("Bad\nName")),
        Err(ClientNameRejection::ForbiddenCharacter)
    );
}

#[test]
fn a_c1_control_character_is_rejected() {
    assert_eq!(
        validate_client_name(Some("Bad\u{0085}Name")),
        Err(ClientNameRejection::ForbiddenCharacter)
    );
}

#[test]
fn a_bidi_override_character_is_rejected() {
    // U+202E RIGHT-TO-LEFT OVERRIDE.
    assert_eq!(
        validate_client_name(Some("Bad\u{202E}Name")),
        Err(ClientNameRejection::ForbiddenCharacter)
    );
}

#[test]
fn a_bidi_isolate_character_is_rejected() {
    // U+2066 LEFT-TO-RIGHT ISOLATE.
    assert_eq!(
        validate_client_name(Some("Bad\u{2066}Name")),
        Err(ClientNameRejection::ForbiddenCharacter)
    );
}

/// Boundary pair 1 of 5: 128/129 scalars. ASCII (1 byte/scalar) keeps the
/// byte count far under 512, isolating the scalar check.
#[test]
fn exactly_128_scalars_is_accepted() {
    let name = "a".repeat(128);
    assert_eq!(name.chars().count(), 128);
    assert_eq!(validate_client_name(Some(&name)), Ok(name));
}

#[test]
fn exactly_129_scalars_is_rejected() {
    let name = "a".repeat(129);
    assert_eq!(
        validate_client_name(Some(&name)),
        Err(ClientNameRejection::TooManyScalars)
    );
}

/// Boundary pair 2 of 5: 512/513 bytes. A UTF-8 scalar is at most 4 bytes,
/// so 513 bytes is unreachable below 129 scalars -- `validate_client_name`
/// checks bytes before scalars for exactly this reason (see
/// `MAX_CLIENT_NAME_BYTES`'s own doc comment), which is what lets this
/// input's rejection name `TooManyBytes` specifically rather than
/// `TooManyScalars`.
#[test]
fn exactly_512_bytes_is_accepted() {
    // 128 four-byte scalars (U+20000, outside the BMP) = 512 bytes, 128
    // scalars -- both exactly at their own limit, both valid together.
    let name: String = std::iter::repeat_n('\u{20000}', 128).collect();
    assert_eq!(name.len(), 512);
    assert_eq!(name.chars().count(), 128);
    assert_eq!(validate_client_name(Some(&name)), Ok(name));
}

#[test]
fn exactly_513_bytes_is_rejected_as_too_many_bytes_not_too_many_scalars() {
    // 128 four-byte scalars + one one-byte scalar = 129 scalars, 513
    // bytes. 129 scalars would also trip TooManyScalars if that check ran
    // first; it does not, by design.
    let mut name: String = std::iter::repeat_n('\u{20000}', 128).collect();
    name.push('a');
    assert_eq!(name.len(), 513);
    assert_eq!(name.chars().count(), 129);
    assert_eq!(
        validate_client_name(Some(&name)),
        Err(ClientNameRejection::TooManyBytes)
    );
}

// ── 2: token_endpoint_auth_method ─────────────────────────────────────────

#[test]
fn an_absent_auth_method_defaults_to_client_secret_basic() {
    assert_eq!(validate_auth_method(None), Ok("client_secret_basic"));
}

#[test]
fn none_is_accepted() {
    assert_eq!(validate_auth_method(Some("none")), Ok("none"));
}

#[test]
fn client_secret_basic_is_accepted() {
    assert_eq!(
        validate_auth_method(Some("client_secret_basic")),
        Ok("client_secret_basic")
    );
}

#[test]
fn client_secret_post_is_accepted() {
    assert_eq!(
        validate_auth_method(Some("client_secret_post")),
        Ok("client_secret_post")
    );
}

/// The matrix's own table cell says "basic"/"post"; this deployment's real
/// wire vocabulary, used everywhere else, is the RFC 7591 strings. The
/// shorthand is rejected like any other unsupported value -- see
/// `validate_auth_method`'s own doc comment for why the shorthand is not
/// implemented literally.
#[test]
fn the_matrix_s_own_shorthand_is_not_a_real_wire_value() {
    assert!(validate_auth_method(Some("basic")).is_err());
    assert!(validate_auth_method(Some("post")).is_err());
}

#[test]
fn an_unrecognised_auth_method_is_rejected() {
    assert_eq!(
        validate_auth_method(Some("private_key_jwt")),
        Err(InvalidAuthMethod("private_key_jwt".to_string()))
    );
}

// ── 2: grant_types ─────────────────────────────────────────────────────────

#[test]
fn an_absent_grant_types_defaults_to_authorization_code_alone() {
    assert_eq!(
        validate_grant_types(None),
        Ok(vec!["authorization_code".to_string()])
    );
}

#[test]
fn authorization_code_alone_is_accepted() {
    let grants = vec!["authorization_code".to_string()];
    assert_eq!(validate_grant_types(Some(&grants)), Ok(grants));
}

#[test]
fn authorization_code_plus_refresh_token_is_accepted() {
    let grants = vec![
        "authorization_code".to_string(),
        "refresh_token".to_string(),
    ];
    assert_eq!(validate_grant_types(Some(&grants)), Ok(grants));
}

#[test]
fn an_empty_grant_types_list_is_rejected() {
    assert_eq!(
        validate_grant_types(Some(&[])),
        Err(GrantTypesRejection::Empty)
    );
}

#[test]
fn a_duplicate_grant_type_is_rejected() {
    let grants = vec![
        "authorization_code".to_string(),
        "authorization_code".to_string(),
    ];
    assert_eq!(
        validate_grant_types(Some(&grants)),
        Err(GrantTypesRejection::Duplicate(
            "authorization_code".to_string()
        ))
    );
}

#[test]
fn refresh_token_without_authorization_code_is_rejected() {
    let grants = vec!["refresh_token".to_string()];
    assert_eq!(
        validate_grant_types(Some(&grants)),
        Err(GrantTypesRejection::RefreshWithoutCode)
    );
}

#[test]
fn any_other_grant_is_rejected() {
    let grants = vec!["implicit".to_string()];
    assert_eq!(
        validate_grant_types(Some(&grants)),
        Err(GrantTypesRejection::OtherGrant("implicit".to_string()))
    );
}

// ── 2: response_types ────────────────────────────────────────────────────

#[test]
fn an_absent_response_types_defaults_to_code() {
    assert_eq!(validate_response_types(None), Ok(vec!["code".to_string()]));
}

#[test]
fn code_alone_is_accepted() {
    let types = vec!["code".to_string()];
    assert_eq!(validate_response_types(Some(&types)), Ok(types));
}

#[test]
fn an_empty_response_types_list_is_rejected() {
    assert_eq!(
        validate_response_types(Some(&[])),
        Err(ResponseTypesRejection::Empty)
    );
}

#[test]
fn a_duplicate_response_type_is_rejected() {
    let types = vec!["code".to_string(), "code".to_string()];
    assert_eq!(
        validate_response_types(Some(&types)),
        Err(ResponseTypesRejection::Duplicate("code".to_string()))
    );
}

#[test]
fn token_response_type_is_rejected() {
    let types = vec!["token".to_string()];
    assert_eq!(
        validate_response_types(Some(&types)),
        Err(ResponseTypesRejection::NotCodeOnly("token".to_string()))
    );
}

#[test]
fn a_hybrid_response_type_is_rejected() {
    let types = vec!["code".to_string(), "id_token".to_string()];
    assert_eq!(
        validate_response_types(Some(&types)),
        Err(ResponseTypesRejection::NotCodeOnly("id_token".to_string()))
    );
}

#[test]
fn any_other_response_type_is_rejected() {
    let types = vec!["id_token".to_string()];
    assert_eq!(
        validate_response_types(Some(&types)),
        Err(ResponseTypesRejection::NotCodeOnly("id_token".to_string()))
    );
}

// ── 2: scope ─────────────────────────────────────────────────────────────

fn catalog(names: &[&str]) -> HashSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn an_absent_scope_is_rejected_as_empty() {
    assert_eq!(
        validate_scope(None, &catalog(&["openid"])),
        Err(ScopeRejection::Empty)
    );
}

#[test]
fn an_empty_scope_string_is_rejected() {
    assert_eq!(
        validate_scope(Some(""), &catalog(&["openid"])),
        Err(ScopeRejection::Empty)
    );
}

#[test]
fn a_space_character_inside_a_token_is_impossible_by_construction() {
    // Space is the separator, not a token character -- a run of two
    // spaces produces an empty token between them, which is the
    // "malformed" case, exercised below.
    assert_eq!(
        validate_scope(Some("openid  profile"), &catalog(&["openid", "profile"])),
        Err(ScopeRejection::Malformed)
    );
}

#[test]
fn a_leading_or_trailing_space_is_malformed() {
    assert_eq!(
        validate_scope(Some(" openid"), &catalog(&["openid"])),
        Err(ScopeRejection::Malformed)
    );
    assert_eq!(
        validate_scope(Some("openid "), &catalog(&["openid"])),
        Err(ScopeRejection::Malformed)
    );
}

#[test]
fn a_double_quote_in_a_token_is_rejected_as_malformed() {
    assert_eq!(
        validate_scope(Some("openid bad\"scope"), &catalog(&["openid"])),
        Err(ScopeRejection::Malformed)
    );
}

#[test]
fn a_backslash_in_a_token_is_rejected_as_malformed() {
    assert_eq!(
        validate_scope(Some("openid bad\\scope"), &catalog(&["openid"])),
        Err(ScopeRejection::Malformed)
    );
}

/// Every byte RFC 6749 §3.3 allows in a scope-token, in one token. The
/// catalog deliberately does not contain it -- this asserts the
/// *character* check accepts it (the rejection is `Unknown`, proving the
/// request got all the way to the catalog check, not `Malformed`).
#[test]
fn every_allowed_scope_token_character_is_accepted_by_the_charset_check() {
    let allowed: Vec<char> = (0x21u8..=0x7E)
        .filter(|&b| is_scope_token_byte(b))
        .map(|b| b as char)
        .collect();
    // Split into two tokens, each comfortably under the 64-byte
    // per-token cap, so a too-long rejection never masks what this test
    // is actually about: whether every allowed byte passes the charset
    // check. Both must reach `Unknown` (the catalog check), not
    // `Malformed` or `TokenTooLong`.
    let (first_half, second_half) = allowed.split_at(allowed.len() / 2);
    let token_a: String = first_half.iter().collect();
    let token_b: String = second_half.iter().collect();
    assert!(token_a.len() <= MAX_SCOPE_TOKEN_BYTES);
    assert!(token_b.len() <= MAX_SCOPE_TOKEN_BYTES);
    let scope = format!("openid {token_a} {token_b}");
    assert_eq!(
        validate_scope(Some(&scope), &catalog(&["openid"])),
        Err(ScopeRejection::Unknown(token_a))
    );
}

#[test]
fn a_duplicate_scope_token_is_rejected() {
    assert_eq!(
        validate_scope(Some("openid openid"), &catalog(&["openid"])),
        Err(ScopeRejection::Duplicate("openid".to_string()))
    );
}

#[test]
fn an_unknown_catalog_value_is_rejected() {
    assert_eq!(
        validate_scope(Some("openid not_a_real_scope"), &catalog(&["openid"])),
        Err(ScopeRejection::Unknown("not_a_real_scope".to_string()))
    );
}

#[test]
fn scope_tokens_that_are_all_known_but_omit_openid_are_rejected() {
    assert_eq!(
        validate_scope(
            Some("profile email"),
            &catalog(&["openid", "profile", "email"])
        ),
        Err(ScopeRejection::MissingOpenid)
    );
}

/// "Concurrent catalog deletion/default change": `validate_scope` takes
/// whatever catalog snapshot its caller hands it, rather than reading
/// global state itself -- so a scope valid against one snapshot and
/// invalid against a later one (the catalog having been edited between
/// the two calls) is exactly this function behaving as a pure function of
/// its argument, not a bug. This is the stage's answer to the corpus case,
/// demonstrated rather than asserted in prose.
#[test]
fn a_scope_valid_against_one_catalog_snapshot_can_be_invalid_against_a_later_one() {
    let before_deletion = catalog(&["openid", "custom_scope"]);
    assert_eq!(
        validate_scope(Some("openid custom_scope"), &before_deletion),
        Ok(vec!["openid".to_string(), "custom_scope".to_string()])
    );

    let after_deletion = catalog(&["openid"]);
    assert_eq!(
        validate_scope(Some("openid custom_scope"), &after_deletion),
        Err(ScopeRejection::Unknown("custom_scope".to_string()))
    );
}

/// Boundary pair 3 of 5: 32/33 tokens. 31 unique single-character fillers
/// (well under the 64-byte-per-token and 1024-byte-total bounds) plus
/// `openid` isolates the token-count check from the other two.
#[test]
fn exactly_32_tokens_is_accepted() {
    let fillers: Vec<String> = ('a'..='z')
        .chain('A'..='E')
        .map(|c| c.to_string())
        .collect();
    assert_eq!(fillers.len(), 31);
    let mut names: Vec<&str> = fillers.iter().map(String::as_str).collect();
    names.push("openid");
    let scope = names.join(" ");
    let cat = catalog(&names);
    assert_eq!(
        validate_scope(Some(&scope), &cat),
        Ok(names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    );
}

#[test]
fn exactly_33_tokens_is_rejected() {
    let fillers: Vec<String> = ('a'..='z')
        .chain('A'..='F')
        .map(|c| c.to_string())
        .collect();
    assert_eq!(fillers.len(), 32);
    let mut names: Vec<&str> = fillers.iter().map(String::as_str).collect();
    names.push("openid");
    let scope = names.join(" ");
    // The catalog is irrelevant here: the count check runs before the
    // catalog check, so an empty catalog still proves TooManyTokens, not
    // some catalog-shaped rejection standing in for it.
    assert_eq!(
        validate_scope(Some(&scope), &HashSet::new()),
        Err(ScopeRejection::TooManyTokens)
    );
}

/// Boundary pair 4 of 5: 64/65-byte token. Two tokens total (`openid` plus
/// the one under test) keeps token count and total bytes far from their
/// own limits.
#[test]
fn a_64_byte_token_is_accepted() {
    let long = ascii_filler('q', 64);
    let scope = format!("openid {long}");
    let cat = catalog(&["openid", &long]);
    assert_eq!(
        validate_scope(Some(&scope), &cat),
        Ok(vec!["openid".to_string(), long])
    );
}

#[test]
fn a_65_byte_token_is_rejected() {
    let long = ascii_filler('q', 65);
    let scope = format!("openid {long}");
    // Catalog irrelevant -- TokenTooLong runs before the catalog check.
    assert_eq!(
        validate_scope(Some(&scope), &HashSet::new()),
        Err(ScopeRejection::TokenTooLong(long))
    );
}

/// Boundary pair 5 of 5: 1,024/1,025-byte total. 15 fillers at 63 bytes +
/// 1 filler at 57 (1024) or 58 (1025) bytes + `openid` -- 17 tokens, every
/// one of them at most 63 bytes, so neither the 32-token nor the 64-byte
/// bound interferes.
#[test]
fn exactly_1024_bytes_total_is_accepted() {
    let mut fillers: Vec<String> = ('a'..='o').map(|c| ascii_filler(c, 63)).collect();
    assert_eq!(fillers.len(), 15);
    fillers.push(ascii_filler('p', 57));
    let mut names: Vec<&str> = fillers.iter().map(String::as_str).collect();
    names.push("openid");
    let scope = names.join(" ");
    assert_eq!(scope.len(), 1024);
    let cat = catalog(&names);
    assert_eq!(
        validate_scope(Some(&scope), &cat),
        Ok(names.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    );
}

#[test]
fn exactly_1025_bytes_total_is_rejected() {
    let mut fillers: Vec<String> = ('a'..='o').map(|c| ascii_filler(c, 63)).collect();
    fillers.push(ascii_filler('p', 58));
    let mut names: Vec<&str> = fillers.iter().map(String::as_str).collect();
    names.push("openid");
    let scope = names.join(" ");
    assert_eq!(scope.len(), 1025);
    // Catalog irrelevant -- the total-bytes check runs before the token
    // loop even starts.
    assert_eq!(
        validate_scope(Some(&scope), &HashSet::new()),
        Err(ScopeRejection::TooManyBytes)
    );
}

// ── 2: the refresh_token / offline_access coupling ───────────────────────

#[test]
fn refresh_without_offline_access_is_rejected() {
    let grants = vec![
        "authorization_code".to_string(),
        "refresh_token".to_string(),
    ];
    let scopes = vec!["openid".to_string()];
    assert_eq!(
        check_grant_scope_coupling(&grants, &scopes),
        Err(GrantScopeCouplingRejection::RefreshWithoutOfflineAccess)
    );
}

#[test]
fn offline_access_without_refresh_is_rejected() {
    let grants = vec!["authorization_code".to_string()];
    let scopes = vec!["openid".to_string(), "offline_access".to_string()];
    assert_eq!(
        check_grant_scope_coupling(&grants, &scopes),
        Err(GrantScopeCouplingRejection::OfflineAccessWithoutRefresh)
    );
}

#[test]
fn refresh_with_offline_access_is_accepted() {
    let grants = vec![
        "authorization_code".to_string(),
        "refresh_token".to_string(),
    ];
    let scopes = vec!["openid".to_string(), "offline_access".to_string()];
    assert_eq!(check_grant_scope_coupling(&grants, &scopes), Ok(()));
}

#[test]
fn neither_refresh_nor_offline_access_is_accepted() {
    let grants = vec!["authorization_code".to_string()];
    let scopes = vec!["openid".to_string()];
    assert_eq!(check_grant_scope_coupling(&grants, &scopes), Ok(()));
}

// ── 2: application-identity URI fields ────────────────────────────────────

#[test]
fn an_absent_application_uri_is_accepted_as_none() {
    assert_eq!(validate_application_uri(None, "logo_uri"), Ok(None));
}

#[test]
fn an_empty_application_uri_is_accepted_as_none() {
    assert_eq!(validate_application_uri(Some(""), "logo_uri"), Ok(None));
}

#[test]
fn a_canonical_https_application_uri_is_accepted() {
    assert_eq!(
        validate_application_uri(Some("https://rp.example/logo.png"), "logo_uri"),
        Ok(Some("https://rp.example/logo.png".to_string()))
    );
}

#[test]
fn an_http_application_uri_is_rejected() {
    assert!(validate_application_uri(Some("http://rp.example/logo.png"), "logo_uri").is_err());
}

#[test]
fn a_numeric_loopback_http_application_uri_is_also_rejected() {
    // Unlike redirect_uris, there is no loopback allowance here at all --
    // these are passive display/link targets, never a redirect target.
    assert!(validate_application_uri(Some("http://127.0.0.1:8080/logo.png"), "logo_uri").is_err());
}

#[test]
fn an_application_uri_with_userinfo_is_rejected() {
    assert!(
        validate_application_uri(Some("https://user:pass@rp.example/logo.png"), "logo_uri")
            .is_err()
    );
}

#[test]
fn an_application_uri_with_a_fragment_is_rejected() {
    assert!(validate_application_uri(Some("https://rp.example/logo.png#x"), "logo_uri").is_err());
}

#[test]
fn a_noncanonical_application_uri_is_rejected() {
    assert!(validate_application_uri(Some("https://rp.example:443/logo.png"), "logo_uri").is_err());
}

#[test]
fn an_oversized_application_uri_is_rejected() {
    let padding = "x".repeat(MAX_APPLICATION_URI_BYTES);
    let uri = format!("https://rp.example/{padding}");
    assert!(validate_application_uri(Some(&uri), "logo_uri").is_err());
}

// ── 2: post_logout_redirect_uris, the full policy ─────────────────────────

#[test]
fn exactly_16_post_logout_uris_is_accepted() {
    let uris: Vec<String> = (0..16)
        .map(|i| format!("https://rp.example/logout{i}"))
        .collect();
    assert!(validate_post_logout_redirect_uris(Some(&uris)).is_ok());
}

#[test]
fn seventeen_post_logout_uris_is_rejected() {
    let uris: Vec<String> = (0..17)
        .map(|i| format!("https://rp.example/logout{i}"))
        .collect();
    assert_eq!(
        validate_post_logout_redirect_uris(Some(&uris)),
        Err(PostLogoutRejection::TooMany)
    );
}

#[test]
fn a_duplicate_post_logout_uri_is_rejected() {
    let uris = vec![
        "https://rp.example/logout".to_string(),
        "https://rp.example/logout".to_string(),
    ];
    assert_eq!(
        validate_post_logout_redirect_uris(Some(&uris)),
        Err(PostLogoutRejection::Duplicate(
            "https://rp.example/logout".to_string()
        ))
    );
}

#[test]
fn a_noncanonical_post_logout_uri_is_rejected() {
    let uris = vec!["https://rp.example:443/logout".to_string()];
    assert!(validate_post_logout_redirect_uris(Some(&uris)).is_err());
}
