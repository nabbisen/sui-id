//! RFC 147 §1 and §2, and RFC 144 (Refresh, §3): source scans over
//! `crates/sui-id-web/src/pages`'s top-level files (not its test
//! subdirectories, e.g. `clients/tests.rs`, which hold assertions,
//! not markup), that fail the moment a future page regresses a
//! repair — not just the sites each RFC measured.

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

// ── RFC 144 (Refresh, §3): no placeholder carries guidance ────────────────

/// No page sets a `placeholder=` attribute. RFC 144's refresh: a
/// placeholder reads as text already typed, and disappears the
/// moment typing begins, which is when an uncertain reader most
/// wants the guidance. Every field's guidance now lives in visible
/// `field__hint` text instead.
#[test]
fn no_page_sets_a_placeholder_attribute() {
    for (name, src) in page_sources() {
        let lines: Vec<usize> = src
            .match_indices("placeholder=")
            .map(|(pos, _)| src[..pos].matches('\n').count() + 1)
            .collect();
        assert!(
            lines.is_empty(),
            "{name}: placeholder= attribute at line(s) {lines:?} — RFC 144 moves \
             every field's guidance to visible field__hint text instead"
        );
    }
}

/// The stylesheet carries no `::placeholder` rule either — RFC 144 N2a's
/// interim one (`--fg-muted` on a vanishing placeholder) is deleted along
/// with the attribute it styled.
#[test]
fn components_css_has_no_placeholder_rule() {
    assert!(
        !crate::components::components_css().contains("::placeholder"),
        "the stylesheet still defines a ::placeholder rule, but no page sets \
         the attribute it would style"
    );
}

// ── An audit table's header heads every cell its rows render ───────────────

/// The user detail page once headed the audit page's six-cell rows with
/// three columns, so a UUID sat under *Action* and the action under
/// *Outcome* (found in 0.92.0's N2b walk). Both tables now take their header
/// from `audit_table_head`; this checks that it and a rendered row each have
/// `AUDIT_COLUMNS` cells.
#[test]
fn the_audit_header_heads_every_cell_of_an_audit_row() {
    use super::audit::{AUDIT_COLUMNS, audit_row_view, audit_table_head};
    let t = sui_id_i18n::Locale::En.strings();
    let entry = sui_id_shared::api::AuditLogEntryDto {
        at: chrono::Utc::now(),
        actor: None,
        action: "user.create".into(),
        target: Some("00000000-0000-0000-0000-000000000000".into()),
        result: "ok".into(),
        note: None,
    };
    let head = super::common::render(|| audit_table_head(t));
    let row = super::common::render(|| audit_row_view(t, entry));
    // `<th ` or `<th>`, not `<thead`.
    let cells = |html: &str, tag: &str| {
        html.matches(&format!("<{tag} ")).count() + html.matches(&format!("<{tag}>")).count()
    };
    assert_eq!(cells(&head, "th"), AUDIT_COLUMNS, "header cells: {head}");
    assert_eq!(cells(&row, "td"), AUDIT_COLUMNS, "row cells: {row}");
}

// ── A key-value row is a table row, so it sits inside a table ──────────────

/// `kv_row` and its three wrappers render a `<tr>`. Outside a `<tbody>` (the
/// overview and MFA pages once put them in a `<dl>`, or in nothing) the
/// browser drops the cells, and the label and value run together into one
/// line: "Passkeys0Active sessions1" (found in 0.92.0's N2b walk). Every
/// call in a page must follow an open `<tbody>`.
#[test]
fn every_key_value_row_is_inside_a_table_body() {
    const CALLS: [&str; 4] = ["kv_row(", "kv_text(", "kv_code(", "kv_bool_badge("];
    for (name, src) in page_sources() {
        // The helpers' own definitions, and their calls to each other.
        if name == "common.rs" {
            continue;
        }
        let mut bad = vec![];
        for call in CALLS {
            let mut from = 0;
            while let Some(rel) = src[from..].find(call) {
                let pos = from + rel;
                from = pos + call.len();
                // `kv_row(` also matches inside `fn kv_row(` or `use … kv_row`.
                let before = &src[..pos];
                if before.ends_with("fn ")
                    || !before.ends_with(|c: char| !c.is_alphanumeric() && c != '_')
                {
                    continue;
                }
                let open = before.rfind("<tbody>");
                let close = before.rfind("</tbody>");
                if !matches!((open, close), (Some(o), c) if c.is_none_or(|c| o > c)) {
                    bad.push(before.matches('\n').count() + 1);
                }
            }
        }
        bad.sort_unstable();
        assert!(
            bad.is_empty(),
            "{name}: a key-value row outside a <tbody>, at line(s) {bad:?}"
        );
    }
}

// ── RFC 144 N2e §2: a password match you can see ──────────────────────────

/// The three forms with a confirmation field (`pages/setup/admin.rs`'s
/// one, `pages/auth.rs`'s two — change-password and password-reset)
/// each load the script, name their confirmation field's
/// `data-confirms`, and render the shared status element exactly once
/// per form.
#[test]
fn every_confirmation_field_is_wired_to_the_match_script() {
    let sources: std::collections::HashMap<String, String> = page_sources().into_iter().collect();
    // The file stem, not the full source-file name: G13 (`check-audit-matrix.sh`)
    // greps every quoted auth-namespaced literal for an audit-event name,
    // and the full name quoted together with its extension matched that
    // shape (found in source, no matrix row) — a false positive from this
    // test's own fixture, not a real audit event.
    const EXPECTATIONS: &[(&str, usize)] = &[("setup/admin", 1), ("auth", 2)];
    for (stem, expected_forms) in EXPECTATIONS {
        let file = format!("{stem}.rs");
        assert!(
            sources.contains_key(file.as_str()),
            "{file}: not found under src/pages"
        );
        let src = &sources[file.as_str()];
        for (needle, label) in [
            ("/static/password-match.js", "script tag"),
            ("data-confirms=", "confirmation field"),
            ("password_match_status(", "status-element call"),
        ] {
            let found = src.matches(needle).count();
            assert_eq!(
                found, *expected_forms,
                "{file}: expected {expected_forms} {label}(s) ({needle}), found {found}"
            );
        }
    }
}
