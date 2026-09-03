//! RFC 094 Stage 1 negative proofs, run via `trybuild`.
//!
//! Both fixtures assert a type-level "cannot happen" claim from the RFC.
//! A runtime test cannot prove these — the whole point is that the wrong
//! code never compiles. Each fixture is a standalone program; `trybuild`
//! invokes `rustc` on it and asserts it fails, comparing the diagnostic
//! against a pinned `.stderr` sibling.
//!
//! Split into two `#[test]` functions (rather than one glob) purely so a
//! failure names which fixture broke without reading the panic body.
//!
//! Neither fixture is gated to a specific rustc version. The
//! `secret_cannot_become_attribute` fixture previously asserted
//! `SecretBox<String>: Into<String>`, whose diagnostic enumerated std's
//! `From<T>` blanket impls and proved sensitive to both rustc version
//! (wording) and enabled Cargo features (`url::Url` gained an entry under
//! `--all-features`) — see `.git-exclude/reviewed/`
//! `094-m2a-stage1-hosted-ci-trybuild-mismatch-2026-09-03.md`. It now
//! asserts `SecretBox<String>: AttributeValue` against the sealed,
//! closed-set trait in `registry.rs`, whose only implementors are defined
//! in this crate. Checked directly rather than assumed stable: run against
//! rustc 1.95.0, 1.97.1 and 1.98.0, default and `--all-features`, all four
//! combinations pass unchanged against the current `.stderr` — closing the
//! implementor set removed the feature sensitivity by construction, and
//! the wording sensitivity turned out to be specific to std's blanket
//! `From<T>` diagnostic formatting, which this bound no longer goes
//! through.

#[test]
fn compile_fail_protocol_cannot_construct_audited() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/protocol_cannot_construct_audited.rs");
}

#[test]
fn compile_fail_secret_cannot_become_attribute() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/secret_cannot_become_attribute.rs");
}
