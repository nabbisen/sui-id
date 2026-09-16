use super::*;

#[test]
fn plain_value_passes_through() {
    assert_eq!(escape_filter_value("alice"), "alice");
}

#[test]
fn metacharacters_are_escaped() {
    assert_eq!(
        escape_filter_value("a*(b)c\\d\0"),
        "a\\2a\\28b\\29c\\5cd\\00"
    );
}

#[test]
fn stable_id_filter_escapes_the_id() {
    assert_eq!(
        stable_id_filter("entryUUID", "x)(|(uid=*", "(uid={username})"),
        "(&(entryUUID=x\\29\\28|\\28uid=\\2a)(uid=*))"
    );
}

#[test]
fn stable_id_filter_keeps_a_group_clause() {
    assert_eq!(
        stable_id_filter(
            "entryUUID",
            "6f1c-1",
            "(&(uid={username})(memberOf=cn=sso,ou=groups,dc=example,dc=com))"
        ),
        "(&(entryUUID=6f1c-1)(&(uid=*)(memberOf=cn=sso,ou=groups,dc=example,dc=com)))"
    );
}

#[test]
fn stable_id_filter_replaces_every_username_placeholder() {
    assert_eq!(
        stable_id_filter("entryUUID", "id", "(|(uid={username})(mail={username}))"),
        "(&(entryUUID=id)(|(uid=*)(mail=*)))"
    );
}

#[test]
fn a_template_without_parentheses_is_wrapped() {
    assert_eq!(any_user_filter("uid={username}"), "(uid=*)");
}

#[test]
fn dn_under_base_is_case_and_space_insensitive() {
    assert!(dn_is_under_base(
        "UID=bob, OU=People,DC=Example,dc=com",
        "ou=people,dc=example,dc=com"
    ));
}

#[test]
fn dn_equal_to_base_or_outside_is_not_under() {
    let base = "ou=people,dc=example,dc=com";
    assert!(!dn_is_under_base("ou=people,dc=example,dc=com", base));
    assert!(!dn_is_under_base(
        "uid=bob,ou=admins,dc=example,dc=com",
        base
    ));
    assert!(!dn_is_under_base(
        "uid=bob,ou=people,dc=example,dc=org",
        base
    ));
}

#[test]
fn a_suffix_match_inside_a_value_is_not_containment() {
    // "xou=people" must not match the base's "ou=people" RDN.
    assert!(!dn_is_under_base(
        "uid=bob,xou=people,dc=example,dc=com",
        "ou=people,dc=example,dc=com"
    ));
}

#[test]
fn an_escaped_comma_does_not_split_an_rdn() {
    // "cn=Smith\, John" is one RDN; the entry is directly under the base.
    assert!(dn_is_under_base(
        "cn=Smith\\, John,ou=people,dc=example,dc=com",
        "ou=people,dc=example,dc=com"
    ));
    assert_eq!(
        normalized_rdns("cn=Smith\\, John,dc=x").map(|r| r.len()),
        Some(2)
    );
}

#[test]
fn a_malformed_dn_is_never_under_the_base() {
    assert!(!dn_is_under_base(
        "not a dn,ou=people,dc=example,dc=com",
        "ou=people,dc=example,dc=com"
    ));
    assert!(!dn_is_under_base(
        "entryUUID-value",
        "ou=people,dc=example,dc=com"
    ));
}

#[test]
fn same_dn_normalises() {
    assert!(same_dn(
        "UID=bob, ou=People,dc=example",
        "uid=bob,ou=people,dc=example"
    ));
    assert!(!same_dn("uid=bob,dc=example", "uid=bobby,dc=example"));
}
