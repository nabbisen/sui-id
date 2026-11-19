//! RFC 148 §4/§5: help, rendered from the same registry `cli.rs`
//! declares (`clap::Command`, via [`super::command_tree`]) — never a
//! second hand-kept list. The short overview and the per-command usage
//! lines are generated from clap's own introspection, so they cannot
//! disagree with what the parser actually accepts. Only the grouping
//! (which section a command sits under) and each command's environment
//! variables / example are not derivable from clap and are named once
//! below, each checked for coverage by `tests.rs`.

use clap::Command;
use std::fmt::Write as _;

use super::command_tree;

/// RFC 148 §1's five named groups, in display order; each entry is a
/// top-level command's own name, as clap derives it. Checked against the
/// registry's own non-hidden top-level names by
/// `help_groups_cover_every_top_level_command` (`tests.rs`): a command
/// added to the registry and forgotten here fails that test rather than
/// silently missing from the overview.
pub(super) const GROUPS: &[(&str, &[&str])] = &[
    ("Getting started", &["init", "setup", "serve"]),
    ("Running it", &["backup", "upgrade", "config"]),
    ("Administering", &["admin"]),
    ("Developing", &["dev"]),
    ("Help", &["help", "version"]),
];

/// Environment variables and a worked example per leaf command, keyed by
/// its full space-joined path (`"admin master-key rotate"`, not just
/// `"rotate"` — several leaves share a last word). Checked for coverage by
/// `every_leaf_has_notes` (`tests.rs`): a leaf added to the registry and
/// forgotten here fails that test.
pub(super) const COMMAND_NOTES: &[(&str, &[&str], &str)] = &[
    (
        "init",
        &[],
        "sui-id init --production --output /etc/sui-id/sui-id.toml",
    ),
    (
        "setup",
        &["SUI_ID_ADMIN_PASSWORD"],
        "sui-id setup --admin-username admin",
    ),
    (
        "serve",
        &["SUI_ID_MASTER_KEY"],
        "sui-id --config sui-id.toml",
    ),
    (
        "backup create",
        &["SUI_ID_MASTER_KEY", "SUI_ID_BACKUP_PASSPHRASE"],
        "sui-id backup create --to ./sui-id.tar --encrypt",
    ),
    (
        "backup restore",
        &["SUI_ID_MASTER_KEY", "SUI_ID_BACKUP_PASSPHRASE"],
        "sui-id backup restore --from ./sui-id.tar",
    ),
    (
        "backup verify",
        &["SUI_ID_BACKUP_PASSPHRASE"],
        "sui-id backup verify --from ./sui-id.tar",
    ),
    ("upgrade preview", &[], "sui-id upgrade preview"),
    ("config sample", &[], "sui-id config sample > sui-id.toml"),
    (
        "config check",
        &[],
        "sui-id config check --config sui-id.toml",
    ),
    (
        "admin user unlock",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin user unlock --username alice",
    ),
    (
        "admin user reset-mfa",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin user reset-mfa --username alice --reason \"lost device\"",
    ),
    (
        "admin user recovery-link",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin user recovery-link --username alice --reason \"sole admin, lost MFA\"",
    ),
    (
        "admin master-key rotate",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin master-key rotate --generate-new-key --yes",
    ),
    (
        "admin metrics-token rotate",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin metrics-token rotate",
    ),
    (
        "admin registration-token issue",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin registration-token issue --max-uses 1",
    ),
    (
        "admin registration-token revoke",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin registration-token revoke --id 01jc6z9f0000000000000000r0",
    ),
    (
        "admin registration-token list",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin registration-token list",
    ),
    (
        "admin registration-token legacy-candidates",
        &["SUI_ID_MASTER_KEY"],
        "sui-id admin registration-token legacy-candidates",
    ),
    ("dev", &[], "sui-id dev --bind 127.0.0.1:8801"),
    ("help", &[], "sui-id help backup create"),
    ("version", &[], "sui-id version"),
];

pub(crate) fn print_version() {
    println!("sui-id {}", env!("CARGO_PKG_VERSION"));
}

pub(crate) fn print_overview() {
    print!("{}", render_overview());
}

/// `sui-id help [COMMAND...]` and the `--help` alias. Empty `path` is the
/// overview; a nonempty path that resolves prints that command's detail;
/// one that does not names the nearest match (clap's own `suggestions`
/// feature) and points back to `sui-id help`.
/// Returns whether `path` resolved — the caller (`run`) exits non-zero
/// when it did not (RFC 148 §5: "an unknown command... exits non-zero"),
/// the same convention clap's own unrecognized-subcommand error uses.
pub(crate) fn print_help(path: &[String]) -> bool {
    if path.is_empty() {
        print_overview();
        return true;
    }
    let root = command_tree();
    match resolve(&root, path) {
        Some(cmd) => {
            print!("{}", render_detail(cmd, path));
            true
        }
        None => {
            eprintln!("sui-id: unknown command `{}`.", path.join(" "));
            if let Some(suggestion) = nearest(&root, path) {
                eprintln!("  did you mean `sui-id help {suggestion}`?");
            }
            eprintln!("  run `sui-id help` for the full list.");
            false
        }
    }
}

fn resolve<'a>(root: &'a Command, path: &[String]) -> Option<&'a Command> {
    let mut cur = root;
    for word in path {
        cur = cur
            .get_subcommands()
            .find(|c| !c.is_hide_set() && c.get_name() == word.as_str())?;
    }
    Some(cur)
}

/// The nearest top-level command name to `path`'s first word, by edit
/// distance — the same crate (`strsim`, via clap's own `suggestions`
/// feature) clap itself uses for "did you mean".
fn nearest(root: &Command, path: &[String]) -> Option<String> {
    let first = path.first()?;
    root.get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(|c| c.get_name().to_string())
        .min_by_key(|name| strsim::levenshtein(name, first))
}

/// Every group's leaves, in display order, gathered once so the path
/// column can be aligned across every group (RFC 148 O2b §4/C1) rather
/// than restarting at each group's own widest path.
fn grouped_leaves() -> Vec<(&'static str, Vec<Leaf>)> {
    let root = command_tree();
    GROUPS
        .iter()
        .map(|(title, names)| {
            let mut leaves = Vec::new();
            for name in *names {
                // Proven unreachable by `help_groups_cover_every_top_level_command_exactly`
                // (`tests.rs`), which fails first if `GROUPS` ever names
                // something the registry does not.
                #[allow(clippy::expect_used)]
                let sub = root
                    .get_subcommands()
                    .find(|s| s.get_name() == *name)
                    .expect("GROUPS names only real top-level commands");
                collect_leaf_lines(sub, &[], &mut leaves);
            }
            (*title, leaves)
        })
        .collect()
}

/// RFC 148 O2b §4 (C1): one line per command -- its path, then a short
/// purpose -- in one column aligned across every group, no usage detail
/// (that belongs to `sui-id help COMMAND`), every line at most 80
/// columns. `overview_lines_are_at_most_80_columns_and_the_purpose_column_aligns`
/// (`tests.rs`) holds both properties as a test, not only as a
/// convention in this function.
fn render_overview() -> String {
    let root = command_tree();
    let about = root.get_about().map(|s| s.to_string()).unwrap_or_default();
    let mut out = format!("sui-id {} — {about}\n", env!("CARGO_PKG_VERSION"));

    let grouped = grouped_leaves();
    let width = grouped
        .iter()
        .flat_map(|(_, leaves)| leaves.iter())
        .map(|l| l.path.join(" ").len())
        .max()
        .unwrap_or(0);

    for (title, leaves) in &grouped {
        let _ = writeln!(out);
        let _ = writeln!(out, "{title}");
        for l in leaves {
            let path = l.path.join(" ");
            let _ = writeln!(out, "  {path:<width$}  {}", l.about);
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "Global options:");
    let _ = writeln!(
        out,
        "  --config PATH    path to the TOML configuration file (default: ./sui-id.toml)"
    );
    let _ = writeln!(
        out,
        "  --no-backup      skip the automatic pre-migration database snapshot"
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Run `sui-id help COMMAND` for a command's full options, environment"
    );
    let _ = writeln!(out, "variables, and an example.");
    out
}

/// One leaf command, gathered by [`collect_leaf_lines`]: its full word
/// path, clap's own generated usage for it, and its one-line `about`.
/// `usage` is read only by `reference.rs`'s test-only page renderer
/// (§6/C3: nothing in the operator's binary reads it any more).
pub(super) struct Leaf {
    pub(super) path: Vec<String>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) usage: String,
    pub(super) about: String,
}

/// Walks every *visible* descendant of `cmd`, appending one [`Leaf`] per
/// leaf (a command with no visible subcommand of its own) to `out`.
pub(super) fn collect_leaf_lines(cmd: &Command, path: &[&str], out: &mut Vec<Leaf>) {
    let mut path: Vec<&str> = path.to_vec();
    path.push(cmd.get_name());
    let children: Vec<&Command> = cmd.get_subcommands().filter(|c| !c.is_hide_set()).collect();
    if children.is_empty() {
        out.push(Leaf {
            path: path.iter().map(|s| s.to_string()).collect(),
            usage: leaf_usage(cmd, &path),
            about: about_line(cmd),
        });
    } else {
        for child in children {
            collect_leaf_lines(child, &path, out);
        }
    }
}

fn about_line(cmd: &Command) -> String {
    cmd.get_about().map(|s| s.to_string()).unwrap_or_default()
}

/// `sui-id <path words> <clap's own usage for this leaf, minus its own
/// repeated name>` — generated, so it cannot drift from what the parser
/// actually accepts (RFC 107's own defect).
fn leaf_usage(cmd: &Command, path: &[&str]) -> String {
    let full_name = path.join(" ");
    let mut rendered = cmd
        .clone()
        .bin_name(full_name.clone())
        .render_usage()
        .to_string();
    if let Some(rest) = rendered.strip_prefix("Usage: ") {
        rendered = rest.to_string();
    }
    let _ = full_name;
    rendered
}

fn render_detail(cmd: &Command, path: &[String]) -> String {
    let joined = path.join(" ");
    let full_name = format!("sui-id {joined}");
    let mut rendered = cmd.clone();
    rendered = rendered.bin_name(full_name.clone());
    let mut out = rendered.render_long_help().to_string();

    if let Some((_, env_vars, example)) = COMMAND_NOTES.iter().find(|(name, _, _)| *name == joined)
    {
        if !env_vars.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "Environment:");
            for var in *env_vars {
                let _ = writeln!(out, "  {var}");
            }
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "Example:");
        let _ = writeln!(out, "  {example}");
    }

    let _ = writeln!(out);
    let anchor = joined.replace(' ', "-");
    let _ = writeln!(
        out,
        "Reference: {}#{anchor}",
        sui_id_web::github_doc_url!("docs/src/reference/cli.md")
    );
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#[path = "help/tests.rs"]
mod tests;
