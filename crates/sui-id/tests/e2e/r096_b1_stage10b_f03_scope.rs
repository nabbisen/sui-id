//! RFC 096-B1 stage 10b, dispatch §0: the fourth part of the F03-seam
//! decision (RFC 096 `:784-789`, A-plus-four). The type system cannot
//! hold "handlers cannot supply a row-touching reason" across the
//! `sui-id-store` boundary — `sui-id-store` cannot name a
//! `sui-id-core` type, so F03 (`sui_id_store::commands::complete_federated_mfa`)
//! is an ordinary `pub` function taking plain primitives, reachable
//! from any crate that depends on `sui-id-store`, `sui-id` among them.
//! This test is the whole of the enforcement: a source scan, in the
//! shape of `r094_m2a_db_access_scope.rs`, with a failure message that
//! names the rule and the clause it comes from rather than only
//! refusing — the rule is a source-scope test precisely because the
//! compiler cannot state it.
//!
//! **Why `complete_federated_mfa` must never appear in `crates/sui-id/src/`**:
//! the only legitimate caller is stage 10b's own promotion path
//! (`sui_id_core::federated_mfa::promote`/`promote_webauthn`), which
//! runs the verifier first and converts its sealed candidate into
//! F03's plain primitives — the one place that conversion can happen,
//! since only that module can see the candidate types at all (stage
//! 10a's own sealing). A handler calling F03 directly could supply a
//! `step`, a sealed blob pair, or a rejection reason of its own
//! choosing, with no binding gate in front of it at all — exactly what
//! `:758-762`/`:784-789` says a handler must not be able to do.
//!
//! **RFC 096-B1 stage 11 extends this to F06**
//! (`sui_id_store::commands::begin_federation_webauthn_ceremony`),
//! rather than writing a second test: it is the same `sui-id-store`-
//! cannot-name-`sui-id-core` seam, for the same reason, with the same
//! one legitimate caller
//! (`sui_id_core::federated_mfa::begin_webauthn_ceremony`) — a second,
//! near-identical scope test would be the violation this comment
//! warns against on the dispatch's own terms (two acknowledgments of
//! the same rule are one more than the record needs), not a second
//! rule.
//!
//! **Scope, matching `r094_m2a_db_access_scope.rs`'s own choice**:
//! production source only (`crates/sui-id/src/`), not
//! `crates/sui-id/tests/` — a test file naming a command to prove
//! this very test's own mutation sensitivity (below) is not the
//! violation this test exists to catch.

use std::path::{Path, PathBuf};

const FORBIDDEN_NAMES: &[&str] = &[
    "complete_federated_mfa",
    "begin_federation_webauthn_ceremony",
];
const HANDLER_CRATE_SRC: &str = "sui-id/src";

fn handler_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .canonicalize()
        .expect("crates/sui-id/src directory")
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_handler_source_names_f03_or_f06_directly() {
    let dir = handler_src_dir();
    let mut files = Vec::new();
    rs_files(&dir, &mut files);
    assert!(
        !files.is_empty(),
        "expected to scan at least one .rs file under {HANDLER_CRATE_SRC}"
    );

    let mut offenders = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("read .rs file");
        for name in FORBIDDEN_NAMES {
            if text.contains(name) {
                offenders.push(format!("{} (names `{name}`)", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "RFC 096 `:758-762`/`:784-789` (the F03/F06 seam, A-plus-four): only the \
         federated-MFA promotion path (`sui_id_core::federated_mfa::promote`/\
         `promote_webauthn`/`begin_webauthn_ceremony`) may call {FORBIDDEN_NAMES:?} — \
         these are the one place each that converts the verifier's own sealed \
         candidate, or issues a new ceremony, after the binding gate has already \
         run. Calling either directly from a handler bypasses that gate entirely. \
         Found: {offenders:#?}"
    );
}
