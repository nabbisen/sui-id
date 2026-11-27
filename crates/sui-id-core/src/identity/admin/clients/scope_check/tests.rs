use super::*;

#[test]
fn identical_strings_are_zero_apart() {
    assert_eq!(restricted_edit_distance("email", "email"), 0);
    assert_eq!(restricted_edit_distance("", ""), 0);
}

#[test]
fn a_single_insertion_deletion_or_substitution_is_one() {
    assert_eq!(restricted_edit_distance("profle", "profile"), 1); // deletion
    assert_eq!(restricted_edit_distance("profile", "profle"), 1); // insertion
    assert_eq!(restricted_edit_distance("email", "emaill"), 1); // insertion
    assert_eq!(restricted_edit_distance("emzil", "email"), 1); // substitution
}

#[test]
fn an_adjacent_transposition_is_one_not_two() {
    // "emial" is "email" with the 3rd and 4th characters swapped. Plain
    // Levenshtein would call this two edits (two substitutions); the
    // restricted variant this function implements calls it one.
    assert_eq!(restricted_edit_distance("emial", "email"), 1);
}

#[test]
fn two_edits_are_not_collapsed_to_one() {
    assert_eq!(restricted_edit_distance("xyz", "email"), 5);
    assert_eq!(restricted_edit_distance("prfle", "profile"), 2);
}

#[test]
fn an_exact_standard_scope_is_reported_as_such() {
    assert_eq!(
        classify_against_standard_scopes("email"),
        StandardScopeMatch::ExactStandardScope("email")
    );
    assert_eq!(
        classify_against_standard_scopes("openid"),
        StandardScopeMatch::ExactStandardScope("openid")
    );
}

#[test]
fn a_one_edit_typo_is_a_near_miss() {
    assert_eq!(
        classify_against_standard_scopes("profle"),
        StandardScopeMatch::NearMiss("profile")
    );
    assert_eq!(
        classify_against_standard_scopes("emial"),
        StandardScopeMatch::NearMiss("email")
    );
}

#[test]
fn a_case_only_difference_is_a_near_miss_not_an_exact_match() {
    assert_eq!(
        classify_against_standard_scopes("Profile"),
        StandardScopeMatch::NearMiss("profile")
    );
}

#[test]
fn a_genuine_additional_scope_matches_nothing() {
    assert_eq!(
        classify_against_standard_scopes("my_api.read"),
        StandardScopeMatch::None
    );
}
