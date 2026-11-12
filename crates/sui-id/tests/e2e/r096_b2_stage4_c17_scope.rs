//! RFC 096-B2 stage 4 §C (the approved design's items 3 and 4): the store
//! command [`sui_id_store::commands::enable_federation_provider_from_preflight`]
//! takes a `PreflightEvidence` whose fields are distinct newtypes so a
//! caller cannot swap two values and still compile — but nothing in the
//! type system stops a *second* caller from building that struct out of
//! whole cloth and invoking the command directly, skipping
//! `preflight_provider` entirely. `check_and_enable`
//! (`crates/sui-id/src/http/federation_preflight.rs`) is supposed to be
//! the one legitimate caller; this test is what makes that more than a
//! comment, in the shape of `r096_b1_stage10b_f03_scope.rs`, widened per
//! the dispatch: every crate's production source, not just one crate's.
//!
//! **Scope.** `crates/*/src`, every crate, not only `sui-id` — unlike
//! the F03/F06 test, this command's own *definition* lives in
//! `sui-id-store`'s production source too, so the count this test
//! expects is **two** files, not zero: the definition itself, and the
//! one consumer. Test files are excluded the same way
//! `r096_b1_stage10b_f03_scope.rs` excludes `crates/sui-id/tests/`: a
//! directory literally named `tests` anywhere under a crate's `src/`
//! (`sui-id-store/src/commands/tests/...`, this command's own runner
//! tests) is not production source, and neither is a sibling test file
//! (`tests.rs`, or a `tests_*`-prefixed file — this crate's own
//! convention for a test module living beside the code it tests).

use std::path::{Path, PathBuf};

const FORBIDDEN_NAME: &str = "enable_federation_provider_from_preflight";

/// Exactly these two files, in this order; anything else naming the
/// command is the violation this test exists to catch.
const ALLOWED_FILES: &[&str] = &[
    "sui-id-store/src/commands.rs",
    "sui-id/src/http/federation_preflight.rs",
];

fn crates_dir() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is always this crate's own root
    // (`crates/sui-id`), the Cargo.toml the `e2e` test binary belongs
    // to, regardless of where under `tests/` this file lives.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("crates/ directory (one level up from crates/sui-id)")
}

/// `true` for a path this test does not treat as production source: a
/// `tests` directory component anywhere under `src/`, or a sibling test
/// file named `tests.rs` / `tests_*.rs`.
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

/// `path`, relative to `crates/`, with OS-native separators normalized
/// to `/` so the comparison against [`ALLOWED_FILES`] is portable.
fn crate_relative(path: &Path, crates: &Path) -> String {
    path.strip_prefix(crates)
        .expect("path under crates/")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn enable_from_preflight_is_named_in_exactly_its_definition_and_one_consumer() {
    let crates = crates_dir();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("read crates/ dir") {
        let entry = entry.expect("dir entry");
        let src = entry.path().join("src");
        if src.is_dir() {
            rs_files(&src, &mut files);
        }
    }
    assert!(
        !files.is_empty(),
        "expected to scan at least one crate's src/ under crates/"
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("read .rs file");
        if text.contains(FORBIDDEN_NAME) {
            hits.push(crate_relative(path, &crates));
        }
    }
    hits.sort();

    let mut expected: Vec<String> = ALLOWED_FILES.iter().map(|s| s.to_string()).collect();
    expected.sort();

    assert_eq!(
        hits, expected,
        "RFC 096-B2 S4 §C: `{FORBIDDEN_NAME}` must appear in production source in \
         exactly its definition and its one legitimate consumer \
         ({ALLOWED_FILES:?}). A capability-bypassing second caller, or a renamed/\
         moved definition or consumer, would show up here. Found: {hits:#?}"
    );
}
