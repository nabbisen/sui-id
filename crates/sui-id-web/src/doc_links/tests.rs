#[test]
fn github_doc_url_is_pinned_to_this_binarys_own_version() {
    let url = crate::github_doc_url!("docs/src/reference/configuration.md");
    assert_eq!(
        url,
        concat!(
            "https://github.com/nabbisen/sui-id/blob/",
            env!("CARGO_PKG_VERSION"),
            "/docs/src/reference/configuration.md"
        )
    );
    assert!(url.contains(env!("CARGO_PKG_VERSION")));
    assert!(!url.contains("/main/"), "must never link a moving branch");
}
