use super::*;

/// RFC 148 §5 (O2b's C3): the committed page must match a fresh render.
/// **This is the regeneration command**: run with
/// `SUI_ID_REGENERATE_CLI_REFERENCE=1` set (e.g. `SUI_ID_REGENERATE_CLI_REFERENCE=1
/// cargo test -p sui-id --lib cli::reference::tests::the_committed_page_matches_a_fresh_render`)
/// and it writes the fresh render instead of comparing — named in this
/// test's own failure message, so there is one way to fix either outcome.
/// No command in the operator's binary does this (§6's own reasoning: a
/// hidden maintainer-only command in the production binary is something
/// an operator could stumble on). Mutation, confirmed by hand (not
/// re-run automatically, to avoid a test that edits files during the
/// suite): editing one line of `docs/src/reference/cli.md` and rerunning
/// this test (without the env var) turns it red; setting the env var and
/// rerunning once restores it.
#[test]
fn the_committed_page_matches_a_fresh_render() {
    let root = repo_root().expect("repo root");
    let path = root.join(PAGE_PATH);
    let fresh = render();

    if std::env::var("SUI_ID_REGENERATE_CLI_REFERENCE").is_ok() {
        std::fs::write(&path, &fresh).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        return;
    }

    let committed =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert_eq!(
        committed, fresh,
        "{} is stale -- rerun this test with SUI_ID_REGENERATE_CLI_REFERENCE=1 set \
         and commit the result",
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
