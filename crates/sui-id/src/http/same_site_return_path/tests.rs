use super::same_site_return_path;

#[test]
fn empty_is_rejected() {
    assert_eq!(same_site_return_path(""), None);
}

#[test]
fn root_is_accepted() {
    assert_eq!(same_site_return_path("/"), Some("/"));
}

#[test]
fn a_plain_path_is_accepted() {
    assert_eq!(same_site_return_path("/x"), Some("/x"));
}

#[test]
fn not_starting_with_a_slash_is_rejected() {
    assert_eq!(same_site_return_path("x"), None);
    assert_eq!(same_site_return_path("https://x/"), None);
}

#[test]
fn protocol_relative_double_slash_is_rejected() {
    assert_eq!(same_site_return_path("//x"), None);
}

#[test]
fn backslash_right_after_the_leading_slash_is_rejected() {
    assert_eq!(same_site_return_path("/\\x"), None);
}

#[test]
fn backslash_anywhere_else_is_also_rejected() {
    assert_eq!(same_site_return_path("/a\\b"), None);
}

#[test]
fn a_control_character_is_rejected() {
    assert_eq!(same_site_return_path("/a\nb"), None);
    assert_eq!(same_site_return_path("/a\rb"), None);
    assert_eq!(same_site_return_path("/a\0b"), None);
}

/// A URL parser deletes every ASCII tab and newline before it parses
/// (WHATWG URL Standard, basic URL parser), and a header value may carry a
/// tab, so `/<TAB>/x` would be followed as `//x`. This is why rule 4 is
/// `char::is_control` and not the three characters `sanitise_return_to`
/// used to name: narrowing it back turns this test red.
#[test]
fn a_tab_is_rejected_because_a_url_parser_deletes_it() {
    assert_eq!(same_site_return_path("/\t/x"), None);
    assert_eq!(same_site_return_path("/a\tb"), None);
}

#[test]
fn a_percent_encoded_backslash_is_accepted() {
    // Not decoded here, so it is still a same-origin path to a browser,
    // which never decodes a path segment before following `Location`.
    // Asserted so a later "fix" that over-rejects becomes visible.
    assert_eq!(same_site_return_path("/a%0Ab"), Some("/a%0Ab"));
    assert_eq!(same_site_return_path("/%5Cx"), Some("/%5Cx"));
}
