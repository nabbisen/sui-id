use super::*;

/// The `SUBCOMMANDS:` block only -- `OPTIONS:` ends it. Scoping to this
/// block, not the whole help text, is half of RFC 107 fix round item 1's
/// fix: a name mentioned in USAGE examples or OPTIONS prose (e.g.
/// `--admin-username` mentions "admin") must not be able to satisfy the
/// binding test on that mention alone.
fn subcommands_block(text: &str) -> &str {
    let start = text.find("SUBCOMMANDS:\n").expect("SUBCOMMANDS header");
    let rest = &text[start..];
    let end = rest
        .find("\nOPTIONS:\n")
        .expect("OPTIONS header ends the block");
    &rest[..end]
}

/// True only when `name` is the entry's own leading token: exactly four
/// spaces of indent, then `name`, then whitespace or end of line. The
/// other half of item 1's fix -- `text.contains(name)` passed for
/// `backup` on `verify-backup`'s own entry, for `restore` on prose
/// mentioning it, and for `admin` on nothing structural at all (luck,
/// since every real `admin <subaction>` entry's own leading token
/// happens to be `admin`). A line with more than four spaces of indent
/// is a wrapped description continuation, not a new entry, and is
/// skipped.
fn has_top_level_entry(block: &str, name: &str) -> bool {
    for line in block.lines() {
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        if rest.starts_with(' ') {
            continue;
        }
        if rest.split_whitespace().next() == Some(name) {
            return true;
        }
    }
    false
}

/// True only when `name` is named on its own `admin <name>` entry line
/// (which may wrap its description to the next line, as
/// `issue-recovery-link` and the two newest subactions do) -- not a
/// substring anywhere, which is what let `rotate-metrics-token`'s own
/// previous removal (the review's mutation 3) go undetected by `admin`
/// name checks that only ever used `rotate-metrics-token` itself, never
/// this helper.
fn has_admin_entry(block: &str, name: &str) -> bool {
    for line in block.lines() {
        let Some(rest) = line.strip_prefix("    admin ") else {
            continue;
        };
        if rest.split_whitespace().next() == Some(name) {
            return true;
        }
    }
    false
}

/// RFC 107's binding test, direction 1: every name in each const must
/// have its own entry in the help text's `SUBCOMMANDS:` block -- not
/// merely appear as a substring anywhere in the text, which the
/// original version of this test checked and which three of the five
/// top-level names (`backup`, `restore`, `admin`) satisfied by
/// accident, through unrelated text, even with their own entry deleted
/// (RFC 107 fix round 2026-10-19, item 1).
#[test]
fn cli_bindings_cover_help_and_dispatch() {
    let text = help_text();
    let block = subcommands_block(&text);
    for name in TOP_LEVEL_SUBCOMMANDS {
        assert!(
            has_top_level_entry(block, name),
            "{name} is in TOP_LEVEL_SUBCOMMANDS but has no entry of its own \
             in the SUBCOMMANDS block"
        );
    }
    for name in ADMIN_SUBACTIONS {
        assert!(
            has_admin_entry(block, name),
            "{name} is in ADMIN_SUBACTIONS but has no `admin {name}` entry \
             of its own in the SUBCOMMANDS block"
        );
    }
}

/// RFC 107's binding test, direction 2: every name in `ADMIN_SUBACTIONS`
/// must dispatch to a real arm in `run_admin_subcommand`, not fall
/// through to the `unreachable!()` guard that catches a const entry
/// with no matching arm (the "arm with no entry" case the dispatch
/// names) -- caught here as a panic, which fails the test, since
/// `run_admin_subcommand`'s own string-literal `match` cannot be made
/// to fail to compile against a runtime `&[&str]` without restructuring
/// it into a boxed-future dispatch table, a complexity tax this plain
/// CLI dispatcher does not carry for the five lines a test already
/// costs.
#[tokio::test]
async fn every_admin_subaction_dispatches() {
    for name in ADMIN_SUBACTIONS {
        let args: Vec<String> = vec!["sui-id".into(), "admin".into(), (*name).into()];
        let err = run_admin_subcommand(&args)
            .await
            .expect_err("every admin subaction is missing a required flag here");
        let msg = format!("{err:?}");
        assert!(
            !msg.contains("unknown admin subaction"),
            "{name} is in ADMIN_SUBACTIONS but the dispatcher does not \
             recognise it: {msg}"
        );
    }
}

/// RFC 107 fix round, item 2: closes the direction §2 of the returned
/// package disclosed as uncovered -- an arm added to
/// `run_admin_subcommand`'s own `match` without a matching
/// `ADMIN_SUBACTIONS` entry. The review's own precedent for doing this
/// textually rather than by restructuring the dispatcher is this
/// repository's G13 (`scripts/check-audit-matrix.sh`), which already
/// gates a different list against Rust source literals the same way,
/// in both directions. Textual, like G21's own checker states of
/// itself ("a textual scan, a proxy, not a proof"): reads this file's
/// own source via `include_str!`, isolates `run_admin_subcommand`'s
/// body by the next `pub(crate)` function after it, and asserts every
/// `Some("...")` literal pattern found there is in `ADMIN_SUBACTIONS`.
/// A stray `Some("...")` elsewhere in the body (there is none today)
/// would be a false positive; `every_admin_subaction_dispatches` above
/// is still the direction that matters more in practice, since this one
/// only fires if someone adds a working arm and simply forgets the
/// const line next to it.
#[test]
fn every_dispatched_admin_arm_is_in_the_const() {
    let src = include_str!("../cli.rs");
    let start = src
        .find("pub(crate) async fn run_admin_subcommand")
        .expect("run_admin_subcommand exists in cli.rs");
    let after = &src[start + 1..];
    let end = after
        .find("\npub(crate)")
        .expect("a later pub(crate) item ends this function's body");
    let body = &src[start..start + 1 + end];

    let mut rest = body;
    let mut found = Vec::new();
    while let Some(i) = rest.find("Some(\"") {
        let after_open = &rest[i + "Some(\"".len()..];
        let close = after_open
            .find('"')
            .expect("Some(\"...\" has a closing quote");
        found.push(&after_open[..close]);
        rest = &after_open[close..];
    }

    assert!(
        !found.is_empty(),
        "found no Some(\"...\") arms at all -- the scan's own boundaries are wrong"
    );
    for name in &found {
        assert!(
            ADMIN_SUBACTIONS.contains(name),
            "`Some(\"{name}\")` is a dispatch arm in run_admin_subcommand but \
             {name} is not in ADMIN_SUBACTIONS -- it will not appear in \
             --help or the unknown-subaction message"
        );
    }
    assert_eq!(
        found.len(),
        ADMIN_SUBACTIONS.len(),
        "found {} dispatch arms but ADMIN_SUBACTIONS has {} entries -- \
         counts must match for this to be a real bidirectional check, \
         not merely a one-way scan that happens to find a subset",
        found.len(),
        ADMIN_SUBACTIONS.len()
    );
}

/// Mutation check for the test above: a name genuinely absent from
/// both the const and the dispatcher must still produce the
/// "unknown admin subaction" error, confirming the assertion above
/// is discriminating rather than vacuous.
#[tokio::test]
async fn an_unlisted_admin_subaction_is_rejected() {
    let args: Vec<String> = vec![
        "sui-id".into(),
        "admin".into(),
        "not-a-real-subaction".into(),
    ];
    let err = run_admin_subcommand(&args).await.expect_err("unknown");
    assert!(format!("{err:?}").contains("unknown admin subaction"));
}
