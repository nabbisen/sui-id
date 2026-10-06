//! RFC 095 M3 stage 1b / RFC 8252: [`redirect_uri_matches`]'s one
//! exception to [`is_redirect_uri_registered`]'s exact match -- the
//! port, for a `PublicNativeLoopback` client only.

use super::{ClientRow, redirect_uri_matches};
use sui_id_shared::ids::ClientId;
use sui_id_store::models::{ConsentPolicy, RegistrationSource};

fn client(confidential: bool, redirect_uris: &[&str]) -> ClientRow {
    let now = chrono::Utc::now();
    ClientRow {
        id: ClientId::new(),
        name: "test".into(),
        confidential,
        secret_hash: None,
        redirect_uris: redirect_uris.iter().map(|s| s.to_string()).collect(),
        allowed_scopes: String::new(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: ConsentPolicy::None,
        registered_via: RegistrationSource::Dynamic,
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn a_differing_port_matches_for_a_public_native_loopback_client() {
    let c = client(false, &["http://127.0.0.1:49152/cb"]);
    assert!(redirect_uri_matches(&c, "http://127.0.0.1:51234/cb"));
}

#[test]
fn a_differing_path_does_not_match_even_with_the_right_port() {
    let c = client(false, &["http://127.0.0.1:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "http://127.0.0.1:49152/other"));
}

#[test]
fn a_differing_query_does_not_match() {
    let c = client(false, &["http://127.0.0.1:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "http://127.0.0.1:49152/cb?x=1"));
}

#[test]
fn a_differing_host_family_does_not_match() {
    let c = client(false, &["http://127.0.0.1:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "http://[::1]:49152/cb"));
}

#[test]
fn a_differing_scheme_does_not_match() {
    let c = client(false, &["http://127.0.0.1:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "https://127.0.0.1:49152/cb"));
}

/// Pins the fix for a real operator-precedence bug caught while
/// writing this: `scheme == "http" && ipv4_loopback || ipv6_loopback`
/// (without the explicit grouping now in the source) would accept an
/// IPv6-loopback host on *any* scheme, including `https`, since `&&`
/// binds tighter than `||`.
#[test]
fn an_ipv6_loopback_host_on_https_does_not_match_via_the_loopback_exception() {
    let c = client(false, &["http://[::1]:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "https://[::1]:49152/cb"));
}

#[test]
fn a_confidential_client_gets_exact_port_matching_not_the_exception() {
    // A confidential client cannot derive the loopback profile at
    // all, even though its stored redirect happens to be
    // loopback-shaped -- `derive_closed_profile` would have rejected
    // this combination at registration time, but this function must
    // not assume that and must still refuse the exception on its own
    // terms.
    let c = client(true, &["http://127.0.0.1:49152/cb"]);
    assert!(!redirect_uri_matches(&c, "http://127.0.0.1:51234/cb"));
}

#[test]
fn a_public_https_client_gets_exact_port_matching_not_the_exception() {
    let c = client(false, &["https://rp.example:8443/cb"]);
    assert!(!redirect_uri_matches(&c, "https://rp.example:9443/cb"));
}

#[test]
fn an_exact_match_still_works_for_every_profile() {
    assert!(redirect_uri_matches(
        &client(true, &["https://rp.example/cb"]),
        "https://rp.example/cb"
    ));
    assert!(redirect_uri_matches(
        &client(false, &["http://127.0.0.1:49152/cb"]),
        "http://127.0.0.1:49152/cb"
    ));
}
