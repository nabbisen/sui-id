//! RFC 094 Stage 1 negative proofs, run via `trybuild`.
//!
//! Both fixtures assert a type-level "cannot happen" claim from the RFC.
//! A runtime test cannot prove these — the whole point is that the wrong
//! code never compiles. Each fixture is a standalone program; `trybuild`
//! invokes `rustc` on it and asserts it fails.

#[test]
fn compile_fail_fixtures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
