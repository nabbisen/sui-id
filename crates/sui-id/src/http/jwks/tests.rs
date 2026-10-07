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
