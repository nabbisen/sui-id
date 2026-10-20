use super::*;

// ── nesting_depth ────────────────────────────────────────────────────────

#[test]
fn a_scalar_has_depth_zero() {
    assert_eq!(nesting_depth(&serde_json::json!(5)), 0);
    assert_eq!(nesting_depth(&serde_json::json!("x")), 0);
    assert_eq!(nesting_depth(&serde_json::json!(null)), 0);
}

#[test]
fn an_empty_object_or_array_has_depth_one() {
    assert_eq!(nesting_depth(&serde_json::json!({})), 1);
    assert_eq!(nesting_depth(&serde_json::json!([])), 1);
}

#[test]
fn depth_is_one_more_than_the_deepest_child() {
    let v = serde_json::json!({"a": {"b": {"c": 1}}});
    assert_eq!(nesting_depth(&v), 3);
}

/// The shallow element is written *first* in the array, deliberately --
/// a `Vec` always iterates in exactly the order it is written, so unlike
/// an object's keys (see [`depth_follows_the_deepest_branch_not_the_first`]'s
/// doc comment) there is no hidden reordering that could make the deep
/// element happen to come first regardless of how this literal is written.
/// Picking deep-first here would let `.map(nesting_depth).next()` agree
/// with `.map(nesting_depth).max()` by coincidence, exactly the way the
/// object test below used to.
#[test]
fn an_array_nests_the_same_way_an_object_does() {
    let v = serde_json::json!([3, [1, 2]]);
    assert_eq!(nesting_depth(&v), 2);
}

/// `serde_json`'s `Value::Object` is a `BTreeMap` (this workspace pins
/// `serde_json = "1"` with no `preserve_order`), so its keys always
/// iterate in sorted order, never insertion order. Two cases, not one,
/// so the result does not depend on which one happens to sort first
/// *today* -- the first below has the shallow key sort first (`"a_"` <
/// `"z_"`), the second has the deep key sort first. If `preserve_order`
/// were ever enabled, iteration would switch to insertion order; the
/// case below written with the deep branch *first in the source* would
/// then also become the one that exercises deep-first iteration, so at
/// least one of the two always catches a walker that returns the first
/// child instead of the deepest, under either ordering regime.
#[test]
fn depth_follows_the_deepest_branch_not_the_first() {
    let shallow_sorts_first =
        serde_json::json!({"a_shallow": 1, "z_deep": {"deeper": {"deepest": 1}}});
    assert_eq!(nesting_depth(&shallow_sorts_first), 3);

    let deep_sorts_first =
        serde_json::json!({"a_deep": {"deeper": {"deepest": 1}}, "z_shallow": 1});
    assert_eq!(nesting_depth(&deep_sorts_first), 3);
}

// ── first_repeated_member ────────────────────────────────────────────────

#[test]
fn a_document_with_no_repeated_member_returns_none() {
    assert_eq!(first_repeated_member(br#"{"a":1,"b":2}"#).unwrap(), None);
}

#[test]
fn a_top_level_repeated_member_is_found() {
    assert_eq!(
        first_repeated_member(br#"{"a":1,"a":2}"#).unwrap(),
        Some("a".to_string())
    );
}

#[test]
fn a_repeated_member_nested_inside_an_array_is_found() {
    assert_eq!(
        first_repeated_member(br#"{"items":[{"x":1,"x":2}]}"#).unwrap(),
        Some("x".to_string())
    );
}

#[test]
fn the_same_name_in_two_different_objects_is_not_a_repeat() {
    assert_eq!(
        first_repeated_member(br#"{"a":{"x":1},"b":{"x":2}}"#).unwrap(),
        None
    );
}

#[test]
fn bytes_that_are_not_json_are_reported_as_not_json() {
    assert!(first_repeated_member(b"not json").is_err());
}
