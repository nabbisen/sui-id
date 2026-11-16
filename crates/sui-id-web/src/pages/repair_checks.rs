//! RFC 147 §1 and §2: two source scans over `crates/sui-id-web/src/pages`'s
//! top-level files (not its test subdirectories, e.g. `clients/tests.rs`,
//! which hold assertions, not markup), that fail the moment a future page
//! regresses either repair — not just the three/two sites this RFC
//! measured.

use std::fs;
use std::path::Path;

fn pages_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pages")
}

/// Every page source under `src/pages`, **including its subdirectories**
/// (`settings/`, `me_security/`, `setup/` hold 18 pages), skipping test
/// files: a file named `tests.rs`, anything under a `tests/` directory,
/// and this module's own source, which quotes the very literals it scans
/// for. The name is the path relative to `src/pages`.
fn page_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir)
            .expect("read pages dir")
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "tests") {
                    continue;
                }
                walk(&path, root, out);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path
                    .file_name()
                    .is_some_and(|n| n != "tests.rs" && n != "repair_checks.rs")
            {
                let name = path
                    .strip_prefix(root)
                    .expect("under pages")
                    .to_string_lossy()
                    .into_owned();
                let src = fs::read_to_string(&path).expect("read page source");
                out.push((name, src));
            }
        }
    }
    let root = pages_dir();
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out.sort();
    out
}

// ── §1: every primary action has a visible label ──────────────────────────

/// Scans forward from each `button__icon` (an icon inside a button) to the
/// nearest closing `</a>` or `</button>`, whichever comes first, and
/// requires a `{t.` interpolation in between: the visible label. An
/// `aria-label=t.…` attribute (no braces) does not count, so an icon with
/// only an accessible name is exactly what this rejects.
fn icons_without_a_label(src: &str) -> Vec<usize> {
    let mut bad = vec![];
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find("button__icon") {
        let pos = search_from + rel;
        let close_a = src[pos..].find("</a>").map(|i| pos + i);
        let close_button = src[pos..].find("</button>").map(|i| pos + i);
        let close = match (close_a, close_button) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => src.len(),
        };
        if !src[pos..close].contains("{t.") {
            bad.push(src[..pos].matches('\n').count() + 1);
        }
        search_from = pos + "button__icon".len();
    }
    bad
}

#[test]
fn every_button_icon_has_a_visible_label() {
    for (name, src) in page_sources() {
        let bad = icons_without_a_label(&src);
        assert!(
            bad.is_empty(),
            "{name}: button__icon with no visible {{t.…}} label, at line(s) {bad:?}"
        );
    }
}

/// The icon-only button is retired, not only labelled: `.button--icon` was
/// a fixed 32px circle, and the label N1 put inside it wrapped one
/// character per line (found in 0.92.0's N2a walk). Neither a page nor the
/// stylesheet may use the class again.
#[test]
fn the_icon_only_button_class_is_retired() {
    let retired = ["button", "--icon"].concat();
    for (name, src) in page_sources() {
        assert!(
            !src.contains(&retired),
            "{name}: uses the retired `{retired}` class"
        );
    }
    assert!(
        !crate::components::components_css().contains(&retired),
        "the stylesheet defines the retired `{retired}` class"
    );
}

// ── §2: every `empty_state(…, None)` call is on the named, reasoned list ──

/// The balanced-paren argument text of the call whose `(` sits at
/// `open_paren`.
fn call_args(src: &str, open_paren: usize) -> &str {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut i = open_paren;
    loop {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open_paren + 1..i];
                }
            }
            _ => {}
        }
        i += 1;
    }
}

/// Splits `args` on its top-level commas (depth 0 relative to `args`
/// itself), so `Some(("/path", t.label))` is one argument, not three.
fn top_level_args(args: &str) -> Vec<&str> {
    let mut parts = vec![];
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, c) in args.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(args[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(args[start..].trim());
    parts
}

/// Every `empty_state(message, None)` call's line, for a bare literal
/// `None` second argument — not a conditional that merely evaluates to
/// `None` sometimes, which several pages already pass deliberately
/// (confirmed below).
fn bare_none_empty_state_calls(src: &str) -> Vec<usize> {
    let mut found = vec![];
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find("empty_state(") {
        let name_pos = search_from + rel;
        let open_paren = name_pos + "empty_state".len();
        let args = call_args(src, open_paren);
        let parts = top_level_args(args);
        if parts.len() >= 2 && parts[1] == "None" {
            found.push(src[..name_pos].matches('\n').count() + 1);
        }
        search_from = open_paren + 1;
    }
    found
}

#[test]
fn every_bare_none_empty_state_is_on_the_named_list() {
    // The only reasoned exception RFC 147 §2 names: an empty audit log
    // has no next step, since nothing a user does produces one.
    const ALLOWED: &[&str] = &["audit.rs"];

    for (name, src) in page_sources() {
        let lines = bare_none_empty_state_calls(&src);
        if lines.is_empty() {
            continue;
        }
        assert!(
            ALLOWED.contains(&name.as_str()),
            "{name}: a new bare `empty_state(.., None)` at line(s) {lines:?} needs a reason \
             added to this test's ALLOWED list, same as audit.rs's"
        );
    }
}

/// RFC 147 §2's own confirmation: these four already pass a real action
/// (not a bare `None`), so the fix needed was in `federation_providers.rs`
/// and `signing_keys.rs` alone.
#[test]
fn pages_the_rfc_says_already_pass_an_action_do() {
    for name in ["clients.rs", "users.rs", "dashboard.rs"] {
        let src = fs::read_to_string(pages_dir().join(name)).expect("read page source");
        assert!(
            src.contains("empty_state("),
            "{name}: expected an empty_state( call to confirm against"
        );
        assert!(
            bare_none_empty_state_calls(&src).is_empty(),
            "{name}: expected no bare `None` call, since the RFC says this page already \
             passes an action"
        );
    }
    let passkey_src = fs::read_to_string(pages_dir().join("me_security").join("passkey.rs"))
        .expect("read me_security/passkey.rs");
    assert!(passkey_src.contains("empty_state("));
    assert!(bare_none_empty_state_calls(&passkey_src).is_empty());
}
