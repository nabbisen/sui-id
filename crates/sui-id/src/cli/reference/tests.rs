use super::*;

/// RFC 148 §5: the committed page must match a fresh render. This is the
/// test named in `regenerate`'s own `--check` failure message, so a
/// maintainer who hits either one is pointed at the same fix. Mutation,
/// confirmed by hand (not re-run automatically, to avoid a test that
/// edits files during the suite): editing one line of
/// `docs/src/reference/cli.md` and rerunning this test, or `sui-id
/// generate-cli-reference --check`, turns it red; `sui-id
/// generate-cli-reference` (no `--check`) then restores it.
#[test]
fn the_committed_page_matches_a_fresh_render() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR has at least two ancestors");
    let path = root.join(PAGE_PATH);
    let committed =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let fresh = render();
    assert_eq!(
        committed, fresh,
        "{} is stale -- regenerate it with `sui-id generate-cli-reference`",
        PAGE_PATH
    );
}

/// A render is never empty and always opens with the page's own title --
/// a sanity floor under the equality check above, so a `render()` that
/// degenerated to an empty string could not pass by matching an
/// equally-broken committed file.
#[test]
fn a_fresh_render_is_well_formed() {
    let text = render();
    assert!(text.starts_with("# CLI reference"));
    assert!(text.contains("## Getting started"));
    assert!(text.contains("sui-id setup"));
}
