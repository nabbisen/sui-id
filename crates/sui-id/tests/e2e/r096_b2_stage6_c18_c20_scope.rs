//! RFC 096-B2 stage 6 §A/§B: the same seam S4's own scope test
//! (`r096_b2_stage4_c17_scope.rs`) pins for `enable_federation_provider_
//! from_preflight`, repeated for C18's `delete_federation_provider` and
//! C20's `unlink_federation_provider` — one test, a list of names, in the
//! shape `r096_b1_stage10b_f03_scope.rs` already bundled two forbidden
//! names into one test for (two acknowledgments of the same rule are one
//! more than the record needs).
//!
//! Both names share the same two allowed files: their definition
//! (`sui-id-store/src/commands.rs`) and their one shared consumer
//! (`sui-id/src/http/federation_provider_admin.rs`, §0's own doc explains
//! why C20 gets a wrapper even though it needs no `sui-id`-only context).
//!
//! **Matches on a whole identifier, not a raw substring** — found
//! necessary by running this test against the real production source
//! this stage already wrote, not assumed up front: `delete_federation_
//! provider` (C18's own forbidden name) is also a substring of
//! `ConfirmDeleteFederationProviderData` and `render_confirm_delete_
//! federation_provider`, names the i18n/page layer needs for the confirm
//! page, a wholly different thing from the store command. `str::contains`
//! alone flagged both as violations; `contains_whole_identifier` does not.

use std::path::{Path, PathBuf};

const ALLOWED_FILES: &[&str] = &[
    "sui-id-store/src/commands.rs",
    "sui-id/src/http/federation_provider_admin.rs",
];

const FORBIDDEN_NAMES: &[&str] = &["delete_federation_provider", "unlink_federation_provider"];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("crates/ directory (one level up from crates/sui-id)")
}

fn is_test_file(path: &Path) -> bool {
    if path
        .components()
        .any(|c| c.as_os_str() == std::ffi::OsStr::new("tests"))
    {
        return true;
    }
    match path.file_stem().and_then(|s| s.to_str()) {
        Some(stem) => stem == "tests" || stem.starts_with("tests_"),
        None => false,
    }
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") && !is_test_file(&path) {
            out.push(path);
        }
    }
}

/// `true` if `name` occurs in `text` as a whole identifier -- neither
/// side of the match is an identifier character (`[A-Za-z0-9_]`).
/// Plain `str::contains` is not enough: `delete_federation_provider`
/// (C18's own forbidden name) is also a substring of this very module's
/// own `ConfirmDeleteFederationProviderData` and `render_confirm_delete_
/// federation_provider` -- names the i18n/page layer needs and that
/// name a wholly different thing (the confirm page, not the store
/// command). Found by running this test against real, already-written
/// production source, not assumed.
fn contains_whole_identifier(text: &str, name: &str) -> bool {
    let is_ident_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    let needle = name.as_bytes();
    let mut start = 0;
    while let Some(rel) = text[start..].find(name) {
        let at = start + rel;
        let before_ok = at == 0 || !is_ident_char(bytes[at - 1]);
        let after = at + needle.len();
        let after_ok = after >= bytes.len() || !is_ident_char(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = at + 1;
    }
    false
}

fn crate_relative(path: &Path, crates: &Path) -> String {
    path.strip_prefix(crates)
        .expect("path under crates/")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn all_production_rs_files() -> Vec<PathBuf> {
    let crates = crates_dir();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("read crates/ dir") {
        let entry = entry.expect("dir entry");
        let src = entry.path().join("src");
        if src.is_dir() {
            rs_files(&src, &mut files);
        }
    }
    files
}

#[test]
fn c18_and_c20_are_each_named_in_exactly_their_definition_and_one_shared_consumer() {
    let crates = crates_dir();
    let files = all_production_rs_files();
    assert!(
        !files.is_empty(),
        "expected to scan at least one crate's src/ under crates/"
    );

    let mut expected: Vec<String> = ALLOWED_FILES.iter().map(|s| s.to_string()).collect();
    expected.sort();

    for name in FORBIDDEN_NAMES {
        let mut hits: Vec<String> = Vec::new();
        for path in &files {
            let text = std::fs::read_to_string(path).expect("read .rs file");
            if contains_whole_identifier(&text, name) {
                hits.push(crate_relative(path, &crates));
            }
        }
        hits.sort();
        assert_eq!(
            hits, expected,
            "RFC 096-B2 S6: `{name}` must appear in production source in exactly its \
             definition and its one legitimate consumer ({ALLOWED_FILES:?}). A \
             capability-bypassing second caller, or a renamed/moved definition or \
             consumer, would show up here. Found: {hits:#?}"
        );
    }
}
