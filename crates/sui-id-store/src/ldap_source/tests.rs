use super::escape_filter_value;

#[test]
fn plain_username_passes_through() {
    assert_eq!(escape_filter_value("alice"), "alice");
}

#[test]
fn metacharacters_are_escaped() {
    // RFC 4515 §3: special characters must be percent-escaped as \XX
    assert_eq!(escape_filter_value("a*(b)c\\d"), "a\\2a\\28b\\29c\\5cd");
}

#[test]
fn nul_byte_is_escaped() {
    assert_eq!(escape_filter_value("a\0b"), "a\\00b");
}

#[test]
fn injection_attempt_is_neutered() {
    // A classic LDAP injection: closing the filter and adding an OR clause.
    let malicious = "alice)(|(objectClass=*)";
    let escaped = escape_filter_value(malicious);
    // The ( ) * characters must all be escaped.
    assert!(!escaped.contains('('));
    assert!(!escaped.contains(')'));
    assert!(!escaped.contains('*'));
    // Reconstruct with the filter template to confirm it is safe.
    let filter = format!("(uid={})", escaped);
    assert_eq!(filter, "(uid=alice\\29\\28|\\28objectClass=\\2a\\29)");
}
