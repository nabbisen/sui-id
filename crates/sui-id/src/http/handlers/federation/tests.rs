use super::attempt_is_superseded;

#[test]
fn matching_version_and_generation_are_not_superseded() {
    assert!(!attempt_is_superseded(0, 0, 0, 0));
    assert!(!attempt_is_superseded(3, 7, 3, 7));
}

#[test]
fn a_differing_version_alone_is_superseded() {
    assert!(attempt_is_superseded(1, 0, 2, 0));
}

#[test]
fn a_differing_generation_alone_is_superseded() {
    assert!(attempt_is_superseded(0, 1, 0, 2));
}

#[test]
fn both_differing_is_still_just_superseded() {
    assert!(attempt_is_superseded(1, 1, 2, 2));
}
