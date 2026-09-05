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

/// RFC 094 Stage 2: `AuthorizedCommandContext` gating per command. See
/// `registry.rs`'s `SystemPrincipalPermitted`.
///
/// Unlike the two fixtures above, this one *is* gated — checked directly,
/// not assumed: E0599's wording changed between rustc 1.95 and 1.96 (`` the
/// associated function or constant `for_system_actor` exists `` vs `` the
/// function or associated item `for_system_actor` exists ``, and similarly
/// in the "cannot be called" clause). 1.96 already carries the new
/// wording, and every later toolchain tested (1.97.1, 1.98.0) matches it,
/// so `since(1.96)` isn't chasing a moving target — it's the one real
/// boundary. Below it, this test is `#[ignore]`d entirely (trybuild has no
/// "check failure, skip the message" mode), so the MSRV lanes don't
/// re-verify this specific gate — but the gate itself is an ordinary
/// trait-bound restriction on an `impl` block, whose *existence* doesn't
/// depend on rustc version, only the wording of the error naming it does.
#[rustversion::attr(before(1.96), ignore)]
#[test]
fn compile_fail_system_principal_forbidden_cannot_use_system_actor() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/system_principal_forbidden_cannot_use_system_actor.rs");
}

/// RFC 094 Stage 2 item 3: "use an event enum for the wrong `C`". Ungated
/// — E0117 (orphan rules) is the same across every toolchain tested
/// (1.95.0, 1.96.1, 1.97.1, 1.98.0), unlike either E0599 fixture below.
#[test]
fn compile_fail_event_cannot_bind_to_wrong_command() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/event_cannot_bind_to_wrong_command.rs");
}

/// RFC 094 Stage 2 item 3: "provide an arbitrary event kind".
///
/// Gated the same way and for the same reason as the `for_system_actor`
/// fixture above: E0599's wording for a missing enum variant also changed
/// between rustc 1.95 and 1.96 (`` no variant or associated item named ``
/// vs `` no variant, associated function, or constant named ``) — checked
/// directly on 1.95/1.96/1.97.1/1.98.0, not assumed from the other E0599
/// fixture's drift.
#[rustversion::attr(before(1.96), ignore)]
#[test]
fn compile_fail_arbitrary_event_kind_does_not_exist() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/arbitrary_event_kind_does_not_exist.rs");
}
