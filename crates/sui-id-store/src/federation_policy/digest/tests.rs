use super::*;
use crate::federation_policy::FederationTrustPolicy;
use crate::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use chrono::Utc;
use sui_id_shared::ids::FederationProviderId;

fn fixture_policy() -> FederationTrustPolicy {
    FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &["https://accounts.google.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy")
}

/// 48 arbitrary, fixed bytes -- matches
/// `scripts/federation-policy-digest-known-answer.py`'s own fixture
/// exactly.
fn fixture_secret_envelope() -> Vec<u8> {
    (0u8..48).collect()
}

/// `scripts/federation-policy-digest-known-answer.py`'s own printed
/// output, for the identical fixture policy and secret envelope --
/// re-derived with `python3.14 scripts/federation-policy-digest-known-
/// answer.py`, not hand-computed. Two independent implementations of
/// RFC 096 `:424-437`'s encoding agreeing is the evidence; one compared
/// with itself proves nothing.
const KNOWN_ANSWER: [u8; 32] = [
    0x36, 0x2a, 0x8c, 0x16, 0x60, 0xdc, 0x3b, 0x20, 0x40, 0x94, 0xab, 0xff, 0xc6, 0xde, 0x60, 0xe4,
    0xd3, 0x75, 0x17, 0x1b, 0x9c, 0x4e, 0x22, 0x0f, 0x3f, 0x2f, 0x8f, 0x13, 0x09, 0x06, 0x71, 0x47,
];

#[test]
fn known_answer_vector_matches_the_independent_python_implementation() {
    let digest = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    assert_eq!(digest.as_bytes(), &KNOWN_ANSWER);
}

/// Vector 2 (S3 review): what one-item lists cannot exercise. The input
/// is deliberately non-canonical: scopes and algorithms out of order,
/// origins unsorted, and two ports. Pinned from the same Python script's
/// second line.
const KNOWN_ANSWER_2: [u8; 32] = [
    0x44, 0xc7, 0x28, 0x08, 0x45, 0xe2, 0xb6, 0x80, 0x48, 0xbc, 0xb7, 0x8e, 0x13, 0x2c, 0x86, 0x9f,
    0x80, 0xc5, 0xf8, 0xa1, 0x17, 0x31, 0x93, 0x75, 0x2e, 0x19, 0xcc, 0x08, 0xfd, 0xc1, 0x33, 0x51,
];

#[test]
fn known_answer_vector_2_matches_the_independent_python_implementation() {
    let policy = FederationTrustPolicy::new(
        "https://login.example.com/tenant/v2.0",
        "public-client",
        false,
        "profile openid email",
        &["EdDSA", "RS256"],
        &[
            "https://login.example.com:8443",
            "https://api.example.com",
            "https://login.example.com",
        ],
        ProvisionMode::ProvisionOnFirstLogin,
    )
    .expect("valid policy");
    assert_eq!(compute(&policy, None).as_bytes(), &KNOWN_ANSWER_2);
}

#[test]
fn changing_the_issuer_changes_the_digest() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://idp.example.com",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &["https://idp.example.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

#[test]
fn changing_the_client_id_changes_the_digest() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "a-different-client-id",
        true,
        "openid email",
        &["RS256"],
        &["https://accounts.google.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

/// Also proves token_auth_method changes with it -- the two are
/// derived from the same presence bit, not independently settable.
#[test]
fn changing_the_secret_presence_changes_the_digest() {
    let with_secret = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let without_secret_policy = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        false,
        "openid email",
        &["RS256"],
        &["https://accounts.google.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let without_secret = compute(&without_secret_policy, None);
    assert!(!with_secret.ct_eq(&without_secret));
}

/// RFC 096-B2 S3 dispatch §A: "resealing the same secret does change it
/// (the envelope hash). That is the design, and the reason S2b does not
/// reseal an unchanged secret." Two different envelopes -- standing in
/// for a reseal of the identical plaintext, which always produces a
/// fresh nonce and so different bytes -- must produce different
/// digests, even though nothing else about the policy moved.
#[test]
fn a_different_secret_envelope_changes_the_digest_even_for_the_same_plaintext() {
    let a = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let resealed_envelope: Vec<u8> = (200u8..248).collect();
    let b = compute(&fixture_policy(), Some(&resealed_envelope));
    assert!(!a.ct_eq(&b));
}

#[test]
fn changing_scopes_changes_the_digest() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email profile",
        &["RS256"],
        &["https://accounts.google.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

#[test]
fn changing_id_token_algs_changes_the_digest() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email",
        &["ES256"],
        &["https://accounts.google.com"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

/// Also changes `https_ports` (443 -> 8443), since ports are derived
/// from origins, not an independent field.
#[test]
fn changing_origins_changes_the_digest_and_its_derived_ports() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://accounts.google.com:8443",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &["https://accounts.google.com:8443"],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    assert_eq!(changed_policy.https_ports(), &[8443]);
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

#[test]
fn changing_provision_mode_changes_the_digest() {
    let base = compute(&fixture_policy(), Some(&fixture_secret_envelope()));
    let changed_policy = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &["https://accounts.google.com"],
        ProvisionMode::ProvisionOnFirstLogin,
    )
    .expect("valid policy");
    let changed = compute(&changed_policy, Some(&fixture_secret_envelope()));
    assert!(!base.ct_eq(&changed));
}

/// RFC 096 `:430-432`: lists are "canonical items sorted by the closed
/// enum order or canonical bytes" -- a reorder in the config file is not
/// a policy change. S2b's own review fold-in 2 found exactly this bug
/// in the C23 comparison; this pins the digest side of the same rule.
#[test]
fn reordering_origins_does_not_change_the_digest() {
    let ordered = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &[
            "https://accounts.google.com",
            "https://oauth2.googleapis.com",
        ],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let reordered = FederationTrustPolicy::new(
        "https://accounts.google.com",
        "test-client-id",
        true,
        "openid email",
        &["RS256"],
        &[
            "https://oauth2.googleapis.com",
            "https://accounts.google.com",
        ],
        ProvisionMode::LinkOnly,
    )
    .expect("valid policy");
    let a = compute(&ordered, Some(&fixture_secret_envelope()));
    let b = compute(&reordered, Some(&fixture_secret_envelope()));
    assert!(a.ct_eq(&b));
}

/// RFC 096 `:430`: "Empty and absent are distinct." The one field where
/// the distinction is reachable (every other field refuses empty at
/// `FederationTrustPolicy::new` time already) -- an absent secret and a
/// present-but-empty envelope must not collide.
#[test]
fn an_absent_secret_and_an_empty_envelope_are_distinct() {
    let absent = compute(&fixture_policy(), None);
    let empty = compute(&fixture_policy(), Some(&[]));
    assert!(!absent.ct_eq(&empty));
}

fn seeded_row(
    config_version: i64,
    activation_generation: i64,
    enabled: bool,
    validation_status: ValidationStatus,
    display_name: &str,
) -> FederationProviderRow {
    let now = Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: "google".into(),
        display_name: display_name.into(),
        issuer: "https://accounts.google.com".into(),
        client_id: "test-client-id".into(),
        client_secret_enc: Some(fixture_secret_envelope()),
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled,
        allowed_origins: "https://accounts.google.com".into(),
        config_version,
        activation_generation,
        validation_status,
        evidence_config_version: Some(config_version),
        evidence_activation_generation: Some(activation_generation),
        evidence_policy_digest: Some(vec![0xAB; 32]),
        evidence_observed_at: Some(now),
        evidence_metadata_fingerprint: Some(vec![0xAB; 32]),
        evidence_jwks_fingerprint: Some(vec![0xAB; 32]),
        created_at: now,
        updated_at: now,
        id_token_algs: Some("RS256".into()),
    }
}

/// RFC 096 `:433-434`: "Enabled/review status, both counters, evidence,
/// and display labels are excluded because they are bound separately or
/// are not trust policy." Two rows that differ in every one of those
/// fields, but agree on the nine trust-policy fields, must digest
/// identically.
#[test]
fn excluded_fields_do_not_change_the_digest() {
    let a = seeded_row(0, 0, false, ValidationStatus::RequiresReview, "Google");
    let b = seeded_row(
        9,
        12,
        true,
        ValidationStatus::Validated,
        "A Totally Different Label",
    );
    let policy_a = crate::federation_policy::from_stored(&a).expect("decodes");
    let policy_b = crate::federation_policy::from_stored(&b).expect("decodes");
    let digest_a = compute(&policy_a, a.client_secret_enc.as_deref());
    let digest_b = compute(&policy_b, b.client_secret_enc.as_deref());
    assert!(digest_a.ct_eq(&digest_b));
}
