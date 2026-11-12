use super::*;

const ISSUER: &str = "https://accounts.google.com";

fn new(
    issuer: &str,
    algs: &[&str],
    origins: &[&str],
) -> Result<FederationTrustPolicy, PolicyError> {
    FederationTrustPolicy::new(
        issuer,
        "client-abc",
        true,
        "openid email",
        algs,
        origins,
        ProvisionMode::LinkOnly,
    )
}

// ── the issuer rule ──────────────────────────────────────────────────────

#[test]
fn issuer_with_no_path_is_accepted() {
    let p = new(ISSUER, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.issuer(), ISSUER);
}

#[test]
fn issuer_with_a_real_path_is_accepted() {
    let issuer = "https://login.example.com/tenant/v2.0";
    let p = new(issuer, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.issuer(), issuer);
}

#[test]
fn issuer_mixed_case_host_is_refused() {
    assert!(new("https://Accounts.google.com", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_explicit_default_port_is_refused() {
    assert!(new("https://accounts.google.com:443", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_with_dot_segments_is_refused() {
    assert!(new("https://accounts.google.com/a/../b", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_http_scheme_is_refused() {
    assert!(new("http://accounts.google.com", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_with_query_is_refused() {
    assert!(new("https://accounts.google.com?x=1", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_with_fragment_is_refused() {
    assert!(new("https://accounts.google.com#frag", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_with_userinfo_is_refused() {
    assert!(new("https://user@accounts.google.com", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_empty_is_refused() {
    assert!(new("", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_not_a_url_is_refused() {
    assert!(new("not a url", &["RS256"], &[]).is_err());
}

// ── the issuer host rule (RFC 096 :355, :374-377) ────────────────────────

#[test]
fn issuer_ipv4_literal_is_refused() {
    assert!(new("https://127.0.0.1", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_ipv6_literal_is_refused() {
    assert!(new("https://[::1]", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_private_ipv4_literal_is_refused() {
    assert!(new("https://10.0.0.1", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_localhost_is_refused() {
    assert!(new("https://localhost", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_localhost_suffix_is_refused() {
    assert!(new("https://idp.localhost", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_local_suffix_is_refused() {
    assert!(new("https://idp.local", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_internal_suffix_is_refused() {
    assert!(new("https://idp.internal", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_home_arpa_suffix_is_refused() {
    assert!(new("https://idp.home.arpa", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_local_suffix_is_refused_case_insensitively() {
    assert!(new("https://IDP.LOCAL", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_dot_local_suffix_with_a_trailing_dot_is_still_refused() {
    assert!(new("https://idp.local.", &["RS256"], &[]).is_err());
}

#[test]
fn issuer_host_that_merely_contains_local_as_a_label_is_accepted() {
    assert!(new("https://local.example.com", &["RS256"], &[]).is_ok());
}

#[test]
fn issuer_host_with_internal_idp_as_a_label_is_accepted() {
    assert!(new("https://internal-idp.example.com", &["RS256"], &[]).is_ok());
}

#[test]
fn issuer_host_fixture_test_is_accepted() {
    assert!(new("https://fixture.test", &["RS256"], &[]).is_ok());
}

// ── the origin canonical rule ────────────────────────────────────────────

#[test]
fn empty_origins_defaults_to_the_issuers_own_origin() {
    let p = new(ISSUER, &["RS256"], &[]).expect("accepted");
    assert_eq!(
        p.allowed_origins(),
        &["https://accounts.google.com".to_string()]
    );
}

#[test]
fn an_explicit_origin_matching_the_issuer_is_accepted() {
    let p = new(ISSUER, &["RS256"], &["https://accounts.google.com"]).expect("accepted");
    assert_eq!(p.allowed_origins().len(), 1);
}

#[test]
fn a_second_origin_alongside_the_issuers_is_accepted() {
    let p = new(
        ISSUER,
        &["RS256"],
        &[
            "https://accounts.google.com",
            "https://oauth2.googleapis.com",
        ],
    )
    .expect("accepted");
    assert_eq!(p.allowed_origins().len(), 2);
}

#[test]
fn origin_mixed_case_host_is_refused() {
    assert!(new(ISSUER, &["RS256"], &["https://Accounts.google.com"]).is_err());
}

#[test]
fn origin_explicit_default_port_is_refused() {
    assert!(new(ISSUER, &["RS256"], &["https://accounts.google.com:443"]).is_err());
}

#[test]
fn origin_trailing_slash_is_refused() {
    assert!(new(ISSUER, &["RS256"], &["https://accounts.google.com/"]).is_err());
}

#[test]
fn origin_set_missing_the_issuers_own_origin_is_refused() {
    assert!(new(ISSUER, &["RS256"], &["https://oauth2.googleapis.com"]).is_err());
}

#[test]
fn more_than_eight_origins_is_refused() {
    let base = "https://accounts.google.com";
    let extra: Vec<String> = (0..8).map(|i| format!("https://o{i}.example")).collect();
    let mut origins: Vec<&str> = vec![base];
    origins.extend(extra.iter().map(String::as_str));
    assert_eq!(origins.len(), 9);
    assert!(new(ISSUER, &["RS256"], &origins).is_err());
}

#[test]
fn a_duplicate_origin_is_refused() {
    assert!(
        new(
            ISSUER,
            &["RS256"],
            &["https://accounts.google.com", "https://accounts.google.com"]
        )
        .is_err()
    );
}

#[test]
fn a_non_https_origin_is_refused() {
    assert!(new(ISSUER, &["RS256"], &["http://accounts.google.com"]).is_err());
}

// ── https_ports, derived ─────────────────────────────────────────────────

#[test]
fn default_port_origin_derives_443() {
    let p = new(ISSUER, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.https_ports(), &[443]);
}

#[test]
fn a_non_default_port_origin_derives_that_port() {
    let issuer = "https://idp.example:8443";
    let p = new(issuer, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.https_ports(), &[8443]);
}

#[test]
fn two_origins_with_the_same_port_derive_one_port() {
    let p = new(
        ISSUER,
        &["RS256"],
        &[
            "https://accounts.google.com",
            "https://oauth2.googleapis.com",
        ],
    )
    .expect("accepted");
    assert_eq!(p.https_ports(), &[443]);
}

// ── scopes ────────────────────────────────────────────────────────────────

#[test]
fn openid_alone_is_accepted() {
    let p = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    )
    .expect("accepted");
    assert_eq!(p.scopes(), &["openid".to_string()]);
}

#[test]
fn scopes_are_reordered_into_the_canonical_order() {
    let p = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "profile email openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    )
    .expect("accepted");
    assert_eq!(p.scopes_canonical(), "openid email profile");
}

#[test]
fn scopes_missing_openid_is_refused() {
    let r = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "email profile",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    );
    assert!(r.is_err());
}

#[test]
fn an_unknown_scope_is_refused() {
    let r = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "openid groups",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    );
    assert!(r.is_err());
}

#[test]
fn a_duplicate_scope_is_refused() {
    let r = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "openid openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    );
    assert!(r.is_err());
}

// ── id_token_algs ─────────────────────────────────────────────────────────

#[test]
fn a_single_permitted_alg_is_accepted() {
    let p = new(ISSUER, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.id_token_algs(), &["RS256".to_string()]);
}

#[test]
fn algs_are_reordered_into_the_canonical_order() {
    let p = new(ISSUER, &["EdDSA", "RS256"], &[]).expect("accepted");
    assert_eq!(p.id_token_algs_canonical(), "RS256 EdDSA");
}

#[test]
fn empty_algs_is_refused() {
    assert!(new(ISSUER, &[], &[]).is_err());
}

#[test]
fn more_than_four_algs_is_refused() {
    assert!(new(ISSUER, &["RS256", "PS256", "ES256", "EdDSA", "RS256"], &[]).is_err());
}

#[test]
fn alg_none_is_refused() {
    assert!(new(ISSUER, &["none"], &[]).is_err());
}

#[test]
fn alg_hs256_is_refused() {
    assert!(new(ISSUER, &["HS256"], &[]).is_err());
}

#[test]
fn an_unpermitted_alg_is_refused() {
    assert!(new(ISSUER, &["RS384"], &[]).is_err());
}

#[test]
fn a_duplicate_alg_is_refused() {
    assert!(new(ISSUER, &["RS256", "RS256"], &[]).is_err());
}

// ── client_id ────────────────────────────────────────────────────────────

#[test]
fn empty_client_id_is_refused() {
    let r = FederationTrustPolicy::new(
        ISSUER,
        "",
        true,
        "openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    );
    assert!(r.is_err());
}

#[test]
fn a_control_character_in_client_id_is_refused() {
    let r = FederationTrustPolicy::new(
        ISSUER,
        "client\u{0007}abc",
        true,
        "openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    );
    assert!(r.is_err());
}

// ── token_auth_method, derived ───────────────────────────────────────────

#[test]
fn a_client_secret_derives_client_secret_basic() {
    let p = new(ISSUER, &["RS256"], &[]).expect("accepted");
    assert_eq!(p.token_auth_method(), TokenAuthMethod::ClientSecretBasic);
}

#[test]
fn no_client_secret_derives_none() {
    let p = FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        false,
        "openid",
        &["RS256"],
        &[],
        ProvisionMode::LinkOnly,
    )
    .expect("accepted");
    assert_eq!(p.token_auth_method(), TokenAuthMethod::None);
}

// ── the stored-row reader (review fold-in) ────────────────────────────────

fn stored_row(
    id_token_algs: Option<&str>,
    allowed_origins: &str,
) -> crate::models::FederationProviderRow {
    let now = chrono::Utc::now();
    crate::models::FederationProviderRow {
        id: sui_id_shared::ids::FederationProviderId::new(),
        slug: "google".into(),
        display_name: "Google".into(),
        issuer: ISSUER.into(),
        client_id: "client-abc".into(),
        client_secret_enc: Some(vec![1, 2, 3]),
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: allowed_origins.into(),
        config_version: 0,
        activation_generation: 0,
        validation_status: crate::models::ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: id_token_algs.map(String::from),
    }
}

#[test]
fn a_stored_row_with_no_algorithms_recorded_has_no_usable_policy() {
    assert_eq!(
        from_stored(&stored_row(None, "")),
        Err(StoredPolicyError::NotRecorded)
    );
}

#[test]
fn a_stored_row_with_a_refused_algorithm_has_no_usable_policy() {
    let e = from_stored(&stored_row(Some("HS256"), "")).expect_err("refused");
    assert_eq!(e.field(), "id_token_algs");
    assert!(matches!(
        e,
        StoredPolicyError::Invalid(PolicyError::IdTokenAlgs(_))
    ));
}

#[test]
fn a_stored_row_with_an_issuer_host_the_rule_refuses_has_no_usable_policy() {
    let mut row = stored_row(Some("RS256"), "");
    row.issuer = "https://idp.internal".into();
    let e = from_stored(&row).expect_err("refused");
    assert_eq!(e.field(), "issuer");
    assert!(matches!(
        e,
        StoredPolicyError::Invalid(PolicyError::Issuer(_))
    ));
}

#[test]
fn a_stored_row_with_empty_origins_reads_as_the_issuers_origin_alone() {
    let p = from_stored(&stored_row(Some("RS256 ES256"), "")).expect("usable");
    assert_eq!(p.allowed_origins(), ["https://accounts.google.com"]);
    assert_eq!(p.id_token_algs(), ["RS256", "ES256"]);
    assert_eq!(p.token_auth_method(), TokenAuthMethod::ClientSecretBasic);
}

#[test]
fn canonical_scopes_compares_by_meaning_not_spelling() {
    assert_eq!(
        canonical_scopes("email openid").as_deref(),
        Ok("openid email")
    );
    assert_eq!(
        canonical_scopes("profile openid email").as_deref(),
        Ok("openid email profile")
    );
    assert!(canonical_scopes("openid groups").is_err());
}

// ── `changed_fields`, per field (S2b review fold-in) ─────────────────────

fn proposed(origins: &[&str]) -> FederationTrustPolicy {
    FederationTrustPolicy::new(
        ISSUER,
        "client-abc",
        true,
        "openid email",
        &["RS256"],
        origins,
        ProvisionMode::LinkOnly,
    )
    .expect("valid")
}

/// The hash-chained event names what changed. A row migration 0051
/// copied has only `id_token_algs` unrecorded, so that is all it reports.
#[test]
fn a_row_with_only_its_algorithms_unrecorded_reports_only_that_field() {
    let changed = changed_fields(&stored_row(None, ""), &proposed(&[]));
    assert_eq!(changed.into_iter().collect::<Vec<_>>(), ["id_token_algs"]);
}

#[test]
fn a_matching_row_reports_nothing() {
    assert!(changed_fields(&stored_row(Some("RS256"), ""), &proposed(&[])).is_empty());
}

/// `""` means the issuer's origin alone, so it equals that origin written out.
#[test]
fn an_empty_origin_list_and_the_issuers_origin_written_out_are_the_same() {
    let stored = stored_row(Some("RS256"), "https://accounts.google.com");
    assert!(changed_fields(&stored, &proposed(&[])).is_empty());
}

/// A reorder is not a policy change, so it must not disable a provider.
#[test]
fn reordered_origins_are_not_a_change() {
    let stored = stored_row(
        Some("RS256"),
        "https://accounts.google.com https://oauth2.googleapis.com",
    );
    let p = proposed(&[
        "https://oauth2.googleapis.com",
        "https://accounts.google.com",
    ]);
    assert!(changed_fields(&stored, &p).is_empty());
}

/// The derived field moves with the one it is derived from.
#[test]
fn an_origin_moving_port_changes_the_origins_and_the_ports() {
    let stored = stored_row(
        Some("RS256"),
        "https://accounts.google.com https://oauth2.googleapis.com",
    );
    let p = proposed(&[
        "https://accounts.google.com",
        "https://oauth2.googleapis.com:8443",
    ]);
    assert_eq!(
        changed_fields(&stored, &p).into_iter().collect::<Vec<_>>(),
        ["endpoint_origins", "https_ports"]
    );
}

/// S6b review: the bare reserved zones are refused as well as names inside
/// them, since each check used to require a leading dot.
#[test]
fn issuer_bare_reserved_zones_are_refused() {
    for host in ["local", "internal", "home.arpa", "LOCAL", "internal."] {
        let issuer = format!("https://{host}");
        assert!(
            new(&issuer, &["RS256"], &[]).is_err(),
            "{issuer} must be refused"
        );
    }
}

/// Names that merely end in the same letters are not the zone.
#[test]
fn issuer_names_that_only_end_in_the_same_letters_are_accepted() {
    for host in [
        "mylocal.example.com",
        "notinternal.example.com",
        "myhome.arpa.example.com",
    ] {
        let issuer = format!("https://{host}");
        assert!(
            new(&issuer, &["RS256"], &[]).is_ok(),
            "{issuer} must be accepted"
        );
    }
}
