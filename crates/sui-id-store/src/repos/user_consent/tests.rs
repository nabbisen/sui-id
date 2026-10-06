use super::*;

#[test]
fn covers_returns_true_when_all_requested_are_granted() {
    assert!(covers("openid profile email", "openid profile"));
    assert!(covers("openid profile email", "openid"));
    assert!(covers("openid profile email", "openid profile email"));
}

#[test]
fn covers_returns_false_when_new_scope_requested() {
    assert!(!covers("openid profile", "openid profile email"));
    assert!(!covers("openid", "openid offline_access"));
}

#[test]
fn covers_empty_requested_is_always_covered() {
    assert!(covers("openid", ""));
    assert!(covers("", ""));
}
