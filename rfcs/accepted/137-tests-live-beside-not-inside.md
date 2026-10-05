# RFC 137 — Tests live beside the module, not inside it

**Status.** Accepted
**Accepted on.** 2026-10-06
**Approved by.** `@nabbisen`, 2026-10-06: "RFC 137 is accepted." — and, on the one open question, "Your recommendation on direction is accepted", settling that **G21 is scoped to `crates/*/src/` only**; `crates/*/tests/` is measured separately afterwards rather than widening a 77-file change mid-flight.
**Security review.** Not required — no application behaviour changes and no crate's public surface moves. This RFC relocates test code and adds a gate over file layout. Reason subject to acceptance.

**Design prerequisites.** None. The rule already exists in
`.git-exclude/rules/project-instructions-rust.md`; this RFC enforces it and
pays down the accumulated breach.
**Implementation prerequisites.** This RFC Accepted.
**Closure prerequisites.** Every `crates/*/src/**` file's test module lives in a
sibling file; a gate fails on an inline one; the gate's exemption list is empty;
and every crate's test count is **identical** before and after.
**Tracks.** Convention conformance. Neighbour of `ROADMAP.md` R3 (source-size
debt) but distinct: R3 is about file length, this is about file role.
**Touches.** `crates/*/src/**`, `scripts/`, `contracts/gate-inputs.toml`, CI.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

`project-instructions-rust.md` requires ⭕ `src/some_mod.rs` +
`src/some_mod/tests.rs` and forbids ❌ `#[cfg(test)] mod tests` inside the
implementation file. **Measured 2026-10-06: 77 files under `crates/*/src/`
break it, holding 10,587 lines of test code.** The correct pattern is also
present — `authz/tests.rs`, `mail/tests.rs`, `password/tests.rs` — so this is a
rule applied unevenly, not an unknown one.

**`@nabbisen` ruled on 2026-10-06 that all of them are to be fixed.**

> **The count he was given was 70; the precise count is 77.** My first sweep
> matched only `mod tests`, missing modules named otherwise —
> `redirect_uri_matches_tests` and its kind. His ruling was "all", so the
> instruction is unchanged, but the number he approved against was mine and it
> was low by seven.

## Why it is worth doing, beyond "the rule says so"

**A reader opening a module should see the module.** Ten thousand lines of test
code interleaved with implementation is ten thousand lines a reviewer scrolls
past to find the thing under review, and the largest offenders are the
load-bearing ones — `commands.rs` and `registry.rs` among them.

**And it is the architect's own recent failure.** Five of the six modules I
approved this week are inline: `dynamic_registration_validation.rs`,
`resolver.rs`, `response_bounds.rs`, `discovery.rs`, `authorize.rs`. I reviewed
them against the dispatch and the design and never checked the project's Rust
conventions. **A rule nobody enforces is one the reviewer stops seeing.**

## Decisions

### D1 — The gate, built first

**G21** fails when any file under `crates/*/src/` declares a test module
inline — `#[cfg(test)] mod …` with a body — rather than as `mod …;` pointing at
a sibling file.

**Built before the migration, not after.** The 77 become an explicit
**shrink-only exemption list**, and each migrated file deletes one line. The
discipline G20 established applies: **an exempted file that no longer needs the
exemption is an error**, so a stale entry fails the build rather than sitting
unnoticed.

**Why a gate at all:** without one, 77 fixes decay back. Every cleanup in this
project that stuck — G16, G19, G20 — shipped with the gate that holds it.

### D2 — The migration is staged by crate, smallest first

| Stage | Crate | Files |
|---|---|---|
| 1 | `sui-id-web` | 1 |
| 1 | `sui-id-shared` | 4 |
| 1 | `sui-id-i18n` | 5 |
| 2 | `sui-id` | 17 |
| 3 | `sui-id-core` | 24 |
| 4 | `sui-id-store` | 26 |

**Stage 1 is three small crates together** — enough to prove the method and the
gate without a large diff. Stages 2–4 are one crate each.

### D3 — The proof is the test count, per crate, unchanged

**A move that silently drops a test module compiles and passes.** `cargo test`
reports fewer tests and nothing fails. So each package states **the exact test
count for its crate before and after, and they must be equal.**

Nothing else is acceptable evidence. Not "the suite passes" — it will.

## Why the move is mechanically safe

`src/foo.rs` with `#[cfg(test)] mod tests { … }` becomes `src/foo.rs` with
`#[cfg(test)] mod tests;` plus `src/foo/tests.rs`. **The child module keeps
access to the parent's private items through `super::`**, which is why this
pattern works at all. Rust 2018+ allows `foo.rs` and `foo/` to coexist with no
`mod.rs`, and the project's own rules say so.

## Non-goals

- **No test is rewritten, renamed, added or deleted.** Pure relocation.
- **No implementation line changes**, beyond the `mod tests;` declaration.
- **Not R3.** This does not target file length, and a module whose tests move
  out may still exceed 500 lines. R3 is separate and stays open.
- **`tests/` directory files are out of scope** for this RFC, though the same
  splitting logic applies to them under the project rules.

## Open questions

**None. Settled at acceptance.**

*Should G21 also cover `crates/*/tests/`?* — **No. Scoped to `crates/*/src/`**,
approved 2026-10-06. `crates/*/tests/` is measured separately once this is
finished; the directory has **not** been measured and no count for it is
claimed.
