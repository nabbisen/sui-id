use super::*;

fn visible_top_level_names() -> std::collections::BTreeSet<String> {
    command_tree()
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(|c| c.get_name().to_string())
        .collect()
}

fn visible_leaf_paths() -> Vec<String> {
    let root = command_tree();
    let mut leaves: Vec<Leaf> = Vec::new();
    for (_, names) in GROUPS {
        for name in *names {
            let sub = root
                .get_subcommands()
                .find(|s| s.get_name() == *name)
                .expect("group name is a real top-level command");
            collect_leaf_lines(sub, &[], &mut leaves);
        }
    }
    leaves.iter().map(|l| l.path.join(" ")).collect()
}

/// RFC 148 §3's own "cannot drift" requirement: a command added to the
/// registry and forgotten in `GROUPS` would otherwise just not appear in
/// `sui-id help` -- silently, which is RFC 107's own defect reborn one
/// layer up. This asserts the two sets are exactly equal, both ways.
#[test]
fn help_groups_cover_every_top_level_command_exactly() {
    let registry: std::collections::BTreeSet<String> = visible_top_level_names();
    let grouped: std::collections::BTreeSet<String> = GROUPS
        .iter()
        .flat_map(|(_, names)| names.iter())
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        registry, grouped,
        "GROUPS must name exactly the registry's own visible top-level commands"
    );
}

/// Same property, one level down: every leaf command (a word with no
/// further visible subcommand) must have its own entry in
/// `COMMAND_NOTES`, so a newly added leaf with no example/env-var note
/// fails this test instead of silently missing one in `help COMMAND` and
/// the generated reference page.
#[test]
fn every_leaf_has_notes() {
    let leaves: std::collections::BTreeSet<String> = visible_leaf_paths().into_iter().collect();
    let noted: std::collections::BTreeSet<String> = COMMAND_NOTES
        .iter()
        .map(|(name, _, _)| name.to_string())
        .collect();
    assert_eq!(
        leaves, noted,
        "COMMAND_NOTES must name exactly every leaf the registry produces"
    );
}

#[test]
fn overview_fits_one_screen_and_names_every_leaf() {
    let text = render_overview();
    let line_count = text.lines().count();
    assert!(
        line_count <= 60,
        "the overview should fit comfortably on one screen; got {line_count} lines:\n{text}"
    );
    for leaf in visible_leaf_paths() {
        assert!(
            text.contains(&format!("sui-id {leaf}")) || text.contains(&leaf),
            "the overview must mention every leaf; missing {leaf:?}:\n{text}"
        );
    }
}

/// RFC 148 O2b §4 (C1): no overview line wraps an 80-column terminal, and
/// every command's purpose starts at the same column, in every group --
/// not just within one, which is what O2a actually shipped (measured on
/// `ff1a9f5`: aligned per group, not across them). **Mutation:** lengthen
/// one `about` past the shared budget; confirmed red (a later line's
/// purpose column moves, or the 80-column bound is crossed), then
/// reverted.
#[test]
fn overview_lines_are_at_most_80_columns_and_the_purpose_column_aligns() {
    let text = render_overview();
    let mut purpose_columns: Vec<(usize, &str)> = Vec::new();
    for line in text.lines() {
        assert!(
            line.chars().count() <= 80,
            "overview line exceeds 80 columns ({} chars): {line:?}",
            line.chars().count()
        );
        // A command line is "  <path>  <purpose>": two-space indent, then a
        // path (never starting with '-', unlike the "Global options:"
        // flag lines below it), then the gap before the purpose column.
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') || rest.starts_with('-') || rest.is_empty() {
            continue;
        }
        let Some(gap_start) = rest.find("  ") else {
            continue;
        };
        let purpose = rest[gap_start..].trim_start_matches(' ');
        let purpose_col = 2 + rest.len() - rest[gap_start..].trim_start_matches(' ').len();
        let _ = purpose; // used only in the assertion message below
        purpose_columns.push((purpose_col, line));
    }
    assert!(
        purpose_columns.len() >= 15,
        "expected at least fifteen command lines; found {}:\n{text}",
        purpose_columns.len()
    );
    let first = purpose_columns[0].0;
    for (col, line) in &purpose_columns {
        assert_eq!(
            *col, first,
            "this command line's purpose starts at column {col}, not {first} \
             like the others: {line:?}\n{text}"
        );
    }
}

/// `true` for "RFC " followed by 1-3 digits not themselves followed by
/// a further digit -- this project's own RFCs (currently up to 149),
/// never a public standard an operator is expected to know, such as
/// "RFC 3339" or "RFC 6749" (always 4 digits in anything cited here).
fn names_an_internal_rfc(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(pos) = text[i..].find("RFC ") {
        let start = i + pos + 4;
        let digits = bytes[start..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if (1..=3).contains(&digits) {
            return true;
        }
        i = start;
    }
    false
}

/// RFC 100 `0.94.0` P7 §2: an operator reading `--help` does not know
/// what an RFC number means. This renders every leaf's full `--help`
/// text, exactly as `sui-id help <command>` and `<command> --help`
/// would, and fails on the literal jargon rather than on any specific
/// wording -- a new flag or command that names one is caught here
/// without this test needing to change. Code comments keep their RFC
/// references; this only ever renders help text.
#[test]
fn no_leafs_help_text_names_an_internal_rfc() {
    let root = command_tree();
    for path in visible_leaf_paths() {
        let path: Vec<String> = path.split(' ').map(str::to_owned).collect();
        let cmd = resolve(&root, &path).expect("a leaf path from the registry itself resolves");
        let detail = render_detail(cmd, &path);
        assert!(
            !names_an_internal_rfc(&detail),
            "`{}`'s help names an internal RFC -- say what is meant instead:\n{detail}",
            path.join(" ")
        );
    }
}

#[test]
fn help_for_a_known_leaf_resolves_and_names_its_own_options() {
    let root = command_tree();
    let path = vec!["backup".to_string(), "create".to_string()];
    let cmd = resolve(&root, &path).expect("backup create resolves");
    let detail = render_detail(cmd, &path);
    assert!(detail.contains("--to"), "{detail}");
    assert!(detail.contains("--encrypt"), "{detail}");
    assert!(detail.contains("Example:"), "{detail}");
    assert!(detail.contains("Reference:"), "{detail}");
}

#[test]
fn an_unknown_command_resolves_to_nothing_and_suggests_the_nearest() {
    let root = command_tree();
    assert!(resolve(&root, &["bakup".to_string()]).is_none());
    let suggestion = nearest(&root, &["bakup".to_string()]).expect("a suggestion");
    assert_eq!(suggestion, "backup");
}

#[test]
fn hidden_aliases_never_resolve_through_help() {
    let root = command_tree();
    assert!(
        resolve(&root, &["restore".to_string()]).is_none(),
        "a hidden pre-148 alias must not be reachable through `sui-id help`"
    );
}
