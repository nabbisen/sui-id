//! RFC 148 §5: the CLI reference page, rendered from the same registry
//! `cli.rs` declares (via [`super::help`]'s shared grouping/notes data and
//! [`super::command_tree`]) — never a second hand-kept list. `tests.rs`'s
//! drift test fails if the committed page differs from a fresh render;
//! running it with `SUI_ID_REGENERATE_CLI_REFERENCE=1` set writes the
//! fresh render instead of comparing. **No command in the operator's
//! binary does this** (cycle O2a shipped a hidden `generate-cli-reference`
//! command; O2b's review found that maintainer tooling in the production
//! binary is something an operator could stumble on, and moved
//! regeneration into the test itself).

#[cfg(test)]
use anyhow::Context;
#[cfg(test)]
use std::fmt::Write as _;

#[cfg(test)]
use super::command_tree;
#[cfg(test)]
use super::help::{COMMAND_NOTES, GROUPS, Leaf, collect_leaf_lines};

/// The page's path relative to the repository root, named in every
/// failure message that points at it. `#[cfg(test)]` below: nothing in
/// the operator's binary reads or writes this page any more (§6/C3) --
/// only the drift test does, so this whole module is test-only now.
#[cfg(test)]
pub(crate) const PAGE_PATH: &str = "docs/src/reference/cli.md";

#[cfg(test)]
pub(crate) fn repo_root() -> anyhow::Result<std::path::PathBuf> {
    // `CARGO_MANIFEST_DIR` is `<repo>/crates/sui-id` at build time.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(|p| p.to_path_buf())
        .context("CARGO_MANIFEST_DIR has fewer than two ancestors")
}

/// The page's full content, built fresh from the registry every call --
/// the one render `tests.rs`'s drift check (and its own regeneration
/// mode) uses, so there is exactly one way this page's text is produced.
#[cfg(test)]
pub(crate) fn render() -> String {
    let root = command_tree();
    let mut out = String::new();

    let _ = writeln!(out, "# CLI reference");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Generated from `sui-id`'s own command registry (RFC 148 §5). Do not \
         edit by hand — a test fails if this page differs from a fresh \
         render; run it with `SUI_ID_REGENERATE_CLI_REFERENCE=1` set to \
         write the fresh render instead of comparing."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "`--config PATH` and `--no-backup` are global: valid before or after \
         any command below."
    );

    for (title, names) in GROUPS {
        let _ = writeln!(out);
        let _ = writeln!(out, "## {title}");
        for name in *names {
            // Proven unreachable by `help_groups_cover_every_top_level_command_exactly`
            // (`cli/help/tests.rs`), which fails first if `GROUPS` ever
            // names something the registry does not.
            #[allow(clippy::expect_used)]
            let sub = root
                .get_subcommands()
                .find(|s| s.get_name() == *name)
                .expect("GROUPS names only real top-level commands");
            let mut leaves: Vec<Leaf> = Vec::new();
            collect_leaf_lines(sub, &[], &mut leaves);
            for leaf in &leaves {
                render_leaf_section(&mut out, leaf);
            }
        }
    }

    out
}

#[cfg(test)]
fn render_leaf_section(out: &mut String, leaf: &Leaf) {
    let joined = leaf.path.join(" ");
    let anchor = joined.replace(' ', "-");
    let _ = writeln!(out);
    let _ = writeln!(out, "### `sui-id {joined}` {{#{anchor}}}");
    let _ = writeln!(out);
    if !leaf.about.is_empty() {
        let _ = writeln!(out, "{}", leaf.about);
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "```text");
    let _ = writeln!(out, "sui-id {}", leaf.usage);
    let _ = writeln!(out, "```");

    if let Some((_, env_vars, example)) = COMMAND_NOTES.iter().find(|(name, _, _)| *name == joined)
    {
        if !env_vars.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "Environment:");
            for var in *env_vars {
                let _ = writeln!(out, "- `{var}`");
            }
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "Example:");
        let _ = writeln!(out);
        let _ = writeln!(out, "```sh");
        let _ = writeln!(out, "{example}");
        let _ = writeln!(out, "```");
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "reference/tests.rs"]
mod tests;
