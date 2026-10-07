//! RFC 096-A stage 3a: the JWKS structure bounds, each at its boundary and one past.
//! Structural only. Nothing here touches a key's contents; that is stage 3b.

use super::*;

fn parse(text: &str) -> Result<Jwks, JwksError> {
    parse_jwks(text.as_bytes())
}

/// `{"keys":[{"kty":"RSA", ...}]}` with `x` nested `n` arrays deep. The root is
/// depth 1, `keys` depth 2, the key object depth 3, and each array adds one.
fn doc_with_nesting(n: usize) -> String {
    format!(
        r#"{{"keys":[{{"kty":"RSA","x":{}{}}}]}}"#,
        "[".repeat(n),
        "]".repeat(n)
    )
}

fn doc_with_keys(count: usize) -> String {
    let keys: Vec<String> = (0..count)
        .map(|i| format!(r#"{{"kty":"RSA","kid":"k{i}"}}"#))
        .collect();
    format!(r#"{{"keys":[{}]}}"#, keys.join(","))
}

#[test]
fn a_well_formed_jwks_is_accepted_and_its_keys_are_returned() {
    let jwks =
        parse(r#"{"keys":[{"kty":"RSA","kid":"a","n":"x","e":"AQAB"}]}"#).expect("a valid JWKS");
    assert_eq!(jwks.keys.len(), 1);
    assert_eq!(jwks.keys[0]["kid"], "a");
}

#[test]
fn a_body_that_is_not_json_is_refused() {
    assert!(matches!(parse("{not json"), Err(JwksError::NotJson)));
}

#[test]
fn a_root_that_is_not_an_object_is_refused() {
    assert!(matches!(parse("[1,2]"), Err(JwksError::NotObject)));
}

#[test]
fn a_missing_or_non_array_keys_member_is_refused() {
    assert!(matches!(parse("{}"), Err(JwksError::NoKeys)));
    assert!(matches!(parse(r#"{"keys":{}}"#), Err(JwksError::NoKeys)));
}

#[test]
fn a_body_over_the_byte_cap_is_refused_before_it_is_parsed() {
    let body = vec![b' '; MAX_RESPONSE_BYTES + 1];
    assert!(matches!(
        parse_jwks(&body),
        Err(JwksError::Bounds(BoundsError::TooLarge { .. }))
    ));
}

// ---- depth: 16 is the limit, the root object is 1 ---------------------------

#[test]
fn nesting_of_sixteen_levels_is_accepted() {
    // root 1, keys 2, key 3, then 13 arrays: 16 in all.
    parse(&doc_with_nesting(13)).expect("depth 16 is within the limit");
}

#[test]
fn nesting_of_seventeen_levels_is_refused() {
    assert!(matches!(
        parse(&doc_with_nesting(14)),
        Err(JwksError::TooDeep {
            limit: MAX_JWKS_DEPTH
        })
    ));
}

// ---- keys: 32 is the limit ---------------------------------------------------

#[test]
fn thirty_two_keys_are_accepted() {
    parse(&doc_with_keys(MAX_JWKS_KEYS)).expect("32 keys is within the limit");
}

#[test]
fn thirty_three_keys_are_refused() {
    assert!(matches!(
        parse(&doc_with_keys(MAX_JWKS_KEYS + 1)),
        Err(JwksError::TooManyKeys {
            limit: MAX_JWKS_KEYS
        })
    ));
}

#[test]
fn a_key_entry_that_is_not_an_object_is_refused_by_index() {
    assert!(matches!(
        parse(r#"{"keys":[{"kty":"RSA"},1]}"#),
        Err(JwksError::KeyNotObject { index: 1 })
    ));
}

// ---- the transport caps, reused from response_bounds --------------------------

#[test]
fn a_key_with_more_than_128_members_is_refused_by_the_shared_cap() {
    let members: Vec<String> = (0..129).map(|i| format!(r#""m{i}":1"#)).collect();
    let key = format!("{{{}}}", members.join(","));
    assert!(matches!(
        parse(&format!(r#"{{"keys":[{key}]}}"#)),
        Err(JwksError::Bounds(BoundsError::TooManyMembers { .. }))
    ));
}

// ---- duplicate members: declared and unknown, at any depth -------------------

#[test]
fn a_repeated_declared_member_is_refused() {
    assert!(matches!(
        parse(r#"{"keys":[{"kty":"RSA","kty":"EC"}]}"#),
        Err(JwksError::DuplicateMember(name)) if name == "kty"
    ));
}

/// `serde_json` does not catch a repeated unknown member. This does.
#[test]
fn a_repeated_unknown_member_is_refused() {
    assert!(matches!(
        parse(r#"{"keys":[{"kty":"RSA","zz":1,"zz":2}]}"#),
        Err(JwksError::DuplicateMember(name)) if name == "zz"
    ));
}

#[test]
fn a_repeated_member_in_a_nested_object_is_refused() {
    assert!(matches!(
        parse(r#"{"keys":[{"kty":"RSA","x":{"a":1,"a":2}}]}"#),
        Err(JwksError::DuplicateMember(name)) if name == "a"
    ));
}

#[test]
fn a_repeated_root_member_is_refused() {
    assert!(matches!(
        parse(r#"{"keys":[],"keys":[]}"#),
        Err(JwksError::DuplicateMember(name)) if name == "keys"
    ));
}

#[test]
fn the_same_name_in_two_different_objects_is_not_a_repeat() {
    parse(r#"{"keys":[{"kty":"RSA"},{"kty":"RSA"}]}"#).expect("different objects");
}

// ---- kid: a duplicate nonempty kid is refused; two empty kids are not ---------

#[test]
fn two_keys_with_the_same_nonempty_kid_are_refused() {
    assert!(matches!(
        parse(r#"{"keys":[{"kid":"a"},{"kid":"a"}]}"#),
        Err(JwksError::DuplicateKid(kid)) if kid == "a"
    ));
}

#[test]
fn a_repeated_kid_that_is_not_adjacent_is_refused() {
    assert!(matches!(
        parse(r#"{"keys":[{"kid":"a"},{"kid":"b"},{"kid":"a"}]}"#),
        Err(JwksError::DuplicateKid(kid)) if kid == "a"
    ));
}

#[test]
fn two_keys_with_empty_kids_are_not_a_duplicate() {
    parse(r#"{"keys":[{"kid":""},{"kid":""}]}"#).expect("RFC 096 says nonempty");
}

#[test]
fn a_key_without_a_kid_is_not_a_duplicate_of_another_without_one() {
    parse(r#"{"keys":[{"kty":"RSA"},{"kty":"EC"}]}"#).expect("no kid to repeat");
}

// ---- RFC 096-A stage 3b: key selection -------------------------------------
//
// Every rule runs on the key's raw JSON. The happy-path tests confirm the
// single surviving candidate converts to a `DecodingKey` of the right family;
// every refusal test confirms the specific rule named, not merely that the
// call failed.

use base64ct::{Base64UrlUnpadded, Encoding};
use jsonwebtoken::AlgorithmFamily;

fn b64(bytes: &[u8]) -> String {
    Base64UrlUnpadded::encode_string(bytes)
}

/// 256 bytes: the minimum accepted RSA modulus (2,048 bits).
fn n_256() -> String {
    b64(&[0x01; 256])
}

/// 255 bytes: one short of the minimum.
fn n_255() -> String {
    b64(&[0x01; 255])
}

/// 65537, the common and accepted RSA exponent.
fn e_65537() -> String {
    b64(&[0x01, 0x00, 0x01])
}

fn rsa_key(kid: &str) -> serde_json::Value {
    serde_json::json!({"kty": "RSA", "kid": kid, "n": n_256(), "e": e_65537()})
}

fn ec_key(kid: &str) -> serde_json::Value {
    serde_json::json!({"kty": "EC", "kid": kid, "crv": "P-256", "x": b64(&[1; 32]), "y": b64(&[2; 32])})
}

fn okp_key(kid: &str) -> serde_json::Value {
    serde_json::json!({"kty": "OKP", "kid": kid, "crv": "Ed25519", "x": b64(&[3; 32])})
}

fn jwks_of(keys: Vec<serde_json::Value>) -> Jwks {
    Jwks { keys }
}

// ---- the happy path, one per accepted algorithm ------------------------------

#[test]
fn rs256_selects_the_matching_rsa_key() {
    let key = select_key(&jwks_of(vec![rsa_key("k1")]), "k1", "RS256").expect("RS256 key selects");
    assert_eq!(key.family(), AlgorithmFamily::Rsa);
}

#[test]
fn ps256_selects_the_matching_rsa_key() {
    let key = select_key(&jwks_of(vec![rsa_key("k1")]), "k1", "PS256").expect("PS256 key selects");
    assert_eq!(key.family(), AlgorithmFamily::Rsa);
}

#[test]
fn es256_selects_the_matching_ec_key() {
    let key = select_key(&jwks_of(vec![ec_key("k1")]), "k1", "ES256").expect("ES256 key selects");
    assert_eq!(key.family(), AlgorithmFamily::Ec);
}

#[test]
fn eddsa_selects_the_matching_okp_key() {
    let key = select_key(&jwks_of(vec![okp_key("k1")]), "k1", "EdDSA").expect("EdDSA key selects");
    assert_eq!(key.family(), AlgorithmFamily::Ed);
}

#[test]
fn an_unrelated_key_in_the_set_is_ignored() {
    let key = select_key(
        &jwks_of(vec![ec_key("other"), rsa_key("k1")]),
        "k1",
        "RS256",
    )
    .expect("the matching key is found among others");
    assert_eq!(key.family(), AlgorithmFamily::Rsa);
}

// ---- kid --------------------------------------------------------------------

#[test]
fn no_key_with_the_header_kid_is_refused() {
    assert!(matches!(
        select_key(&jwks_of(vec![rsa_key("other")]), "k1", "RS256"),
        Err(KeySelectionError::KidNotFound)
    ));
}

/// Stage 3a's `parse_jwks` already refuses a document with a duplicate
/// nonempty `kid`. This confirms it, so the refusal below is read as
/// defence in depth, not as the only guard.
#[test]
fn parse_jwks_itself_refuses_a_duplicate_kid() {
    let body = serde_json::json!({"keys": [rsa_key("k1"), ec_key("k1")]}).to_string();
    assert!(matches!(
        parse_jwks(body.as_bytes()),
        Err(JwksError::DuplicateKid(kid)) if kid == "k1"
    ));
}

/// A `Jwks` built directly, bypassing `parse_jwks`, can still carry a
/// duplicate `kid` -- `Jwks`'s field is public precisely so the e2e fetch test
/// can construct one. Selection must still refuse it, not pick either key.
#[test]
fn a_duplicate_kid_built_directly_cannot_reach_a_selection() {
    assert!(matches!(
        select_key(&jwks_of(vec![rsa_key("k1"), ec_key("k1")]), "k1", "RS256"),
        Err(KeySelectionError::AmbiguousKid { count: 2 })
    ));
}

// ---- private key members -----------------------------------------------------

#[test]
fn every_private_key_member_is_refused_by_its_own_name() {
    for member in PRIVATE_KEY_MEMBERS {
        let mut key = rsa_key("k1");
        key.as_object_mut()
            .expect("object")
            .insert(member.to_string(), serde_json::json!("x"));
        assert!(
            matches!(
                select_key(&jwks_of(vec![key]), "k1", "RS256"),
                Err(KeySelectionError::PrivateKeyMember(name)) if name == member
            ),
            "{member} must be refused under its own name"
        );
    }
}

// ---- use / key_ops / multi-use -----------------------------------------------

#[test]
fn a_key_marked_for_encryption_is_refused() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("use".into(), serde_json::json!("enc"));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::UseNotSig)
    ));
}

#[test]
fn key_ops_naming_only_sign_is_refused_for_missing_verify() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("key_ops".into(), serde_json::json!(["sign"]));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::KeyOpsMissingVerify)
    ));
}

#[test]
fn key_ops_naming_both_sign_and_verify_is_multi_use() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("key_ops".into(), serde_json::json!(["sign", "verify"]));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::MultiUse)
    ));
}

#[test]
fn use_sig_with_key_ops_naming_encrypt_disagrees() {
    let mut key = rsa_key("k1");
    let obj = key.as_object_mut().expect("object");
    obj.insert("use".into(), serde_json::json!("sig"));
    obj.insert("key_ops".into(), serde_json::json!(["verify", "encrypt"]));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::MultiUse)
    ));
}

#[test]
fn use_sig_and_key_ops_verify_together_are_accepted() {
    let mut key = rsa_key("k1");
    let obj = key.as_object_mut().expect("object");
    obj.insert("use".into(), serde_json::json!("sig"));
    obj.insert("key_ops".into(), serde_json::json!(["verify"]));
    select_key(&jwks_of(vec![key]), "k1", "RS256").expect("use=sig with key_ops=[verify] agree");
}

// ---- alg on the key -----------------------------------------------------------

#[test]
fn a_key_alg_that_does_not_match_the_header_is_refused() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("alg".into(), serde_json::json!("RS384"));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::AlgMismatch)
    ));
}

#[test]
fn a_key_alg_that_matches_the_header_is_accepted() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("alg".into(), serde_json::json!("RS256"));
    select_key(&jwks_of(vec![key]), "k1", "RS256").expect("a matching alg must be accepted");
}

// ---- algorithm family and curve ------------------------------------------------

#[test]
fn an_ec_key_is_refused_for_an_rsa_algorithm() {
    assert!(matches!(
        select_key(&jwks_of(vec![ec_key("k1")]), "k1", "RS256"),
        Err(KeySelectionError::AlgorithmFamilyMismatch)
    ));
}

#[test]
fn an_rsa_key_is_refused_for_es256() {
    assert!(matches!(
        select_key(&jwks_of(vec![rsa_key("k1")]), "k1", "ES256"),
        Err(KeySelectionError::AlgorithmFamilyMismatch)
    ));
}

#[test]
fn an_unrecognized_header_algorithm_is_refused() {
    assert!(matches!(
        select_key(&jwks_of(vec![rsa_key("k1")]), "k1", "HS256"),
        Err(KeySelectionError::AlgorithmFamilyMismatch)
    ));
}

#[test]
fn es256_with_a_curve_other_than_p256_is_refused() {
    let mut key = ec_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("crv".into(), serde_json::json!("P-384"));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "ES256"),
        Err(KeySelectionError::CurveMismatch)
    ));
}

#[test]
fn eddsa_with_a_curve_other_than_ed25519_is_refused() {
    let mut key = okp_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("crv".into(), serde_json::json!("X25519"));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "EdDSA"),
        Err(KeySelectionError::CurveMismatch)
    ));
}

// ---- RSA size and exponent -----------------------------------------------------

#[test]
fn an_rsa_modulus_of_exactly_256_bytes_is_accepted() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("n".into(), serde_json::json!(n_256()));
    select_key(&jwks_of(vec![key]), "k1", "RS256").expect("256 bytes is 2048 bits, the minimum");
}

#[test]
fn an_rsa_modulus_of_255_bytes_is_refused() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("n".into(), serde_json::json!(n_255()));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::RsaTooSmall)
    ));
}

#[test]
fn an_rsa_exponent_of_65537_is_accepted() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("e".into(), serde_json::json!(e_65537()));
    select_key(&jwks_of(vec![key]), "k1", "RS256").expect("65537 is odd and >= 3");
}

#[test]
fn an_even_rsa_exponent_of_2_is_refused() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("e".into(), serde_json::json!(b64(&[2])));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::RsaInvalidExponent)
    ));
}

#[test]
fn an_rsa_exponent_of_1_is_refused() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("e".into(), serde_json::json!(b64(&[1])));
    assert!(matches!(
        select_key(&jwks_of(vec![key]), "k1", "RS256"),
        Err(KeySelectionError::RsaInvalidExponent)
    ));
}

/// Not in the handoff's list; added because it is the exact boundary the rule
/// states ("less than 3" is refused, so 3 itself must be accepted) and the
/// handoff's three cases (65537, 2, 1) do not exercise it.
#[test]
fn an_rsa_exponent_of_exactly_3_is_accepted() {
    let mut key = rsa_key("k1");
    key.as_object_mut()
        .expect("object")
        .insert("e".into(), serde_json::json!(b64(&[3])));
    select_key(&jwks_of(vec![key]), "k1", "RS256").expect("3 is odd and not less than 3");
}

#[test]
fn a_missing_rsa_n_or_e_is_refused() {
    let mut no_n = rsa_key("k1");
    no_n.as_object_mut().expect("object").remove("n");
    assert!(matches!(
        select_key(&jwks_of(vec![no_n]), "k1", "RS256"),
        Err(KeySelectionError::RsaParameterInvalid)
    ));

    let mut no_e = rsa_key("k1");
    no_e.as_object_mut().expect("object").remove("e");
    assert!(matches!(
        select_key(&jwks_of(vec![no_e]), "k1", "RS256"),
        Err(KeySelectionError::RsaParameterInvalid)
    ));
}
