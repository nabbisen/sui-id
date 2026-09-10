# Handoff — test-file organization

**Owner of this handoff.** High-capability model (architect).
**Implementer.** Mid-capability model.
**Governing rule.** `.git-exclude/rules/project-instructions-rust.md`,
§Testing Guidelines. Under `src/`, test modules are separate files:
`src/some_mod.rs` + `src/some_mod/tests.rs`, and when `tests.rs` grows
too large, submodules under `src/some_mod/tests/`. A `#[test]` module
placed inside `src/some_mod.rs` is the named anti-pattern.

**Not an RFC companion.** This directory follows the existing local
practice for non-RFC handoffs (`fuzz-corpus-persistence`,
`stable-clippy-drift-1.98`, `prep-federation-module-split`,
`rfc-template-reconciliation`). That practice is itself a deviation from
RFC 000's `NNN-slug/` convention, recorded in
`.git-exclude/reviewed/project-manners-recap-2026-09-10.md` §5.5 and
awaiting `@nabbisen`'s ruling. If the ruling goes the other way, this
directory moves with the rest.

## Status

| Item | State |
|---|---|
| A. `commands.rs` split (3,295 → 1,332 + 264 + 1,687) | **Done**, approved 2026-09-10 |
| B. `commands/tests/runner.rs` sub-split | **Ready to start** |
| C. The remaining 42 inline-test files | Queued; see §C |

## A. Completed

`crates/sui-id-store/src/commands.rs` had 1,965 lines of inline
`#[cfg(test)] mod tests`. Split into `commands.rs` (production only,
`mod tests;`), `commands/tests.rs` (registry-level consistency tests),
`commands/tests/runner.rs` (per-command `Database`-backed tests).
Verified as a content-preserving move, not merely a green test run —
see `.git-exclude/reviewed/project-manners-recap-2026-09-10.md` §2.

## B. Split `commands/tests/runner.rs` — do this before Wave C's remainder

**Why now.** U12, `mfa.disable` and `mfa.recovery_codes_regenerate` will
add roughly 300 lines to this file. Splitting first means those tests are
written into their final home; splitting after means moving more code.

**Target layout.** `crates/sui-id-store/src/commands/tests/`:

| File | Contents (current `runner.rs` line ranges) | ≈ lines |
|---|---|---|
| `runner.rs` | shared fixtures + `mod` declarations | 80 |
| `runner/key_rotation.rs` | K01 and its three injected-failure proofs (76–216) | 140 |
| `runner/lockout.rs` | U22 (217–304) and U08 (1074–1133) | 150 |
| `runner/chain_integrity.rs` | `concurrent_class_a_commands_maintain_one_unbroken_chain` (305–356) | 55 |
| `runner/user_admin.rs` | U01–U05 (357–794) | 440 |
| `runner/passwords.rs` | U06 (795–893), U09 (1134–1240), U10 (1283–1423) | 350 |
| `runner/mfa.rs` | U07 (894–1073) | 180 |
| `runner/refresh.rs` | T04/T09 (1424–end) | 265 |

**Grouping rationale — domain, not wave.** RFC 094's waves are a delivery
artifact and will not survive the project; command domains will. Grouping
by domain gives every command still to come an obvious destination:
U12 / `mfa.disable` / `mfa.recovery_codes_regenerate` go to `runner/mfa.rs`,
U11 to `runner/passwords.rs` or a new `runner/email.rs` once RFC 101 lands.

`lockout.rs` deliberately holds U22 and U08 together: U22 sets the lockout
and U08 clears it, and the pair is more readable adjacent than split by
which actor triggers it.

**Fixture placement — decided; do not re-derive.** Checked against every
call site:

| Fixture | Where used | Home |
|---|---|---|
| `fresh_db`, `a_user`, `a_client`, `an_admin`, `latest_audit_action` | throughout | `runner.rs` |
| `seed_active_session` | U02–U05 **and** U06, U09, U10 | `runner.rs` |
| `seed_passkey` | U07 only | `runner/mfa.rs` |
| `seed_reset_token` | U10 only | `runner/passwords.rs` |
| `seed_family`, `a_successor` | T04/T09 only | `runner/refresh.rs` |

No `common.rs` module. Shared fixtures live in the parent (`runner.rs`)
and submodules reach them with `use super::*;` — the same shape
`commands/tests.rs` already uses for `runner`. `seed_active_session`
looks group-local from its position in the file but is not; it has call
sites in U06, U09 and U10. It stays in `runner.rs`.

**Constraints.**

1. **Move only.** No test renamed, reordered, added, removed, or edited.
   Rustfmt may rejoin lines that fit at the shallower indent — that is
   expected and fine; nothing else may change.
2. **Verify the glob chain by compiling.** `use super::*;` becomes four
   levels deep (`commands` → `tests` → `runner` → `runner::mfa`). A
   private glob import is visible to descendant modules, so this should
   work — but confirm it with `cargo check --tests`, do not assume.
3. **Keep the `#[allow(...)]` scope.** `runner`'s
   `#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]`
   must still cover every moved test; an inner `#![allow(...)]` at the
   top of each new file is the equivalent form.

**Evidence required in the review request.** A green test run is not
sufficient evidence for a move-only change — a silently dropped test
still leaves the suite green. Prove content identity:

```
# strip all whitespace from old and new, normalise rustfmt's trailing
# commas, compare hashes
tr -d '[:space:]' < old | sed -e 's/,)/)/g; s/,]/]/g; s/,}/}/g' | sha256sum
```

Report the two hashes and that they match, alongside the usual gates
(`fmt --check`, both clippy scopes, `cargo test` default and
`--all-features` with the 160/164 counts, MSRV 1.95).

## C. The remaining 42 files

42 files under `crates/*/src` still carry an inline `#[cfg(test)] mod
tests`; 15 now follow the convention. Largest first:

```
1081  crates/sui-id-store/src/registry.rs
 908  crates/sui-id-i18n/src/locale/en.rs
 892  crates/sui-id-i18n/src/locale/ja.rs
 891  crates/sui-id-i18n/src/locale/zh_hans.rs
 795  crates/sui-id/src/http/handlers/federation.rs
 769  crates/sui-id-core/src/authn/step_up.rs
 691  crates/sui-id-store/src/repos/audit.rs
 652  crates/sui-id/src/runtime/dev_mode.rs
 511  crates/sui-id-core/src/authn/webauthn.rs
 492  crates/sui-id-store/src/metrics.rs
```

**Do not sweep all 42.** A 42-file mechanical churn across every crate is
a large diff with no functional content, and it would collide with every
in-flight branch.

**Order of work:**

1. **`registry.rs` next**, after §B. It is RFC 094's own seam, it will
   keep growing through M2a, and it is the largest remaining file. Same
   move-only discipline and same identity evidence.
2. **Opportunistically thereafter.** When a wave touches a file in the
   list and adds tests to it, split that file in the same candidate and
   say so in the review request. This retires the debt along the paths
   that are actually moving, without a standalone churn commit.
3. The three `sui-id-i18n` locale files are ~900 lines each and nearly
   all table data. Raise them separately — they may want a different
   treatment from the rest, and that is a design question, not a move.

**Do not** open item C work without a dispatch. It is queued, not active.
