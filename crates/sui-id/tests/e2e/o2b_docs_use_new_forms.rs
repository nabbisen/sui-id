//! RFC 148 cycle O2b §5 (C2): the operator-facing guides teach the new
//! CLI forms. A reader following `docs/src/guides/` or `README.md`
//! should meet no notice. This scans every fenced code block in both for
//! the old forms RFC 148 §2 aliased — prose that merely *names* a
//! command to identify a past action (an audit event's cause, a
//! historical note) is not in scope, which is why this looks only
//! inside code blocks, not the whole page.
//!
//! The allowed list is empty: every hit here must be fixed in the
//! guide, not added to this list. If an old form is ever genuinely
//! needed verbatim in prose (not as an instruction to type), move it
//! out of a code block rather than widening the list.

use std::path::{Path, PathBuf};

/// The exact old forms RFC 148 cycle O2b §5 names. Deliberately narrower
/// than the full pre-148 alias set (bare `backup`/`restore`, `--dev`,
/// `--version`, `--help` are not here): those words alone are also
/// substrings of their own new forms (`backup create`, `backup
/// restore`) or are rarely written as a bare instruction to type, so
/// scanning them risks false positives this dispatch does not ask this
/// test to resolve.
const OLD_FORMS: &[&str] = &[
    "--print-sample-config",
    "verify-backup",
    "preview-upgrade",
    "admin unlock-user",
    "admin reset-mfa",
    "admin issue-recovery-link",
    "admin rotate-key",
    "admin rotate-metrics-token",
    "admin issue-registration-token",
    "admin revoke-registration-token",
    "admin list-registration-tokens",
    "admin legacy-dynamic-candidates",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR has at least two ancestors")
        .to_path_buf()
}

/// Every fenced code block's own content (the backtick fence lines
/// themselves excluded), in order.
/// A fenced block's own opening/closing line, with an optional leading
/// `>` blockquote marker (one level) stripped first -- a code block
/// quoted inside a `>` aside (operators.md's own "keep a copy outside
/// the host" example) is still a real, typed instruction, and the first
/// version of this scan missed exactly that case.
fn fence_marker(line: &str) -> Option<&str> {
    let unquoted = line
        .trim_start()
        .strip_prefix('>')
        .unwrap_or(line)
        .trim_start();
    unquoted.starts_with("```").then_some(unquoted)
}

fn code_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut lines = markdown.lines();
    while let Some(line) = lines.next() {
        if fence_marker(line).is_none() {
            continue;
        }
        let mut block = String::new();
        for inner in lines.by_ref() {
            if fence_marker(inner).is_some() {
                break;
            }
            block.push_str(inner);
            block.push('\n');
        }
        blocks.push(block);
    }
    blocks
}

fn markdown_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            markdown_files_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

#[test]
fn operator_guides_and_readme_teach_the_new_cli_forms() {
    let root = repo_root();
    let mut files = vec![root.join("README.md")];
    markdown_files_under(&root.join("docs/src/guides"), &mut files);
    assert!(files.len() > 3, "sanity: found too few files: {files:?}");

    let mut violations: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for block in code_blocks(&text) {
            for old in OLD_FORMS {
                if block.contains(old) {
                    violations.push(format!("{}: {old:?}", path.display()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "old CLI forms still typed in a guide's own code block (fix the guide, \
         do not widen this test's allowed list -- it is empty by design):\n{}",
        violations.join("\n")
    );
}
