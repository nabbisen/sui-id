//! RFC 134 D1 negative proof, run via `trybuild`. See
//! `sui-id-store/tests/compile_fail.rs` for the project's convention: a
//! fixture is a standalone program `trybuild` invokes `rustc` on, asserting
//! it fails and comparing the diagnostic against a pinned `.stderr`.

#[test]
fn compile_fail_cookies_feature_must_stay_off() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/cookies_feature_must_stay_off.rs");
}

/// RFC 134 D3: see the fixture's own comment.
#[test]
fn compile_fail_validated_discovery_cannot_be_constructed_directly() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/validated_discovery_cannot_be_constructed_directly.rs");
}
