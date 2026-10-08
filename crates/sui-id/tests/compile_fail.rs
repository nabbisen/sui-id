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

/// RFC 096 :648 (stage 4a): see the fixture's own comment.
#[test]
fn compile_fail_verified_id_token_claims_cannot_be_constructed_directly() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/verified_id_token_claims_cannot_be_constructed_directly.rs");
}

/// RFC 096 :873-896 (stage 4c): see the fixture's own comment.
#[test]
fn compile_fail_retained_cache_entry_cannot_be_constructed_directly() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/retained_cache_entry_cannot_be_constructed_directly.rs");
}

/// RFC 096 :656-659 (stage 6a): see the fixture's own comment.
#[test]
fn compile_fail_required_identity_claims_cannot_be_constructed_directly() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/required_identity_claims_cannot_be_constructed_directly.rs");
}
