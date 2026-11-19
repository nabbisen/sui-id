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
