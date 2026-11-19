//! RFC 148 §5: the CLI reference page, rendered from the same registry
//! `cli.rs` declares (via [`super::help`]'s shared grouping/notes data and
//! [`super::command_tree`]) — never a second hand-kept list. `tests.rs`
//! fails if the committed page differs from a fresh render; regenerate it
//! with `sui-id generate-cli-reference` (a hidden, maintenance-only
//! command — see the review package's note on this deviation from the
//! dispatch's own suggestion of an `xtask` subcommand: `xtask` depends on
//! no workspace crate today, by design, and `Cli`'s registry lives in
//! this crate's own lib target, so generating from it here avoids making
//! `xtask` depend on the entire server for one doc page).

use anyhow::{Context, Result, bail};
use std::fmt::Write as _;

use super::command_tree;
use super::help::{COMMAND_NOTES, GROUPS, Leaf, collect_leaf_lines};

/// The page's path relative to the repository root, named in every
/// failure message that points at it.
pub(crate) const PAGE_PATH: &str = "docs/src/reference/cli.md";

/// `sui-id generate-cli-reference [--check]`. `--check` compares the
/// committed page with a fresh render and exits non-zero if they differ,
/// naming this command in the error — the same shape `tests.rs`'s own
/// drift test uses, so a maintainer sees the identical message either
/// way.
pub(crate) fn regenerate(check: bool) -> Result<()> {
    let root = repo_root()?;
    let path = root.join(PAGE_PATH);
    let fresh = render();
    if check {
        let committed = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        if committed != fresh {
            bail!(
                "{} is stale. Regenerate it with `sui-id generate-cli-reference` \
                 and commit the result.",
                PAGE_PATH
            );
        }
        println!("{PAGE_PATH} is current.");
    } else {
        std::fs::write(&path, &fresh).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote {PAGE_PATH}.");
    }
    Ok(())
}

fn repo_root() -> Result<std::path::PathBuf> {
    // `CARGO_MANIFEST_DIR` is `<repo>/crates/sui-id` at build time.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(|p| p.to_path_buf())
        .context("CARGO_MANIFEST_DIR has fewer than two ancestors")
}

/// The page's full content, built fresh from the registry every call —
/// this is the one render both `regenerate` and `tests.rs`'s drift check
/// use, so there is exactly one way this page's text is produced.
pub(crate) fn render() -> String {
    let root = command_tree();
    let mut out = String::new();

    let _ = writeln!(out, "# CLI reference");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Generated from `sui-id`'s own command registry (RFC 148 §5) by \
         `sui-id generate-cli-reference`. Do not edit by hand — a test fails \
         if this page differs from a fresh render, and the regeneration \
         command is named in that test's own failure message."
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
