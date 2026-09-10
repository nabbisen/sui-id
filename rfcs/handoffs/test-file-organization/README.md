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
| B. `commands/tests/runner.rs` sub-split | **Done**, approved 2026-09-10 (`520ee79`) |
| B-2. Correct two misfiled tests in `runner/refresh.rs` | **Done**, approved 2026-09-10 (`74ef5c2`) |
| C. The remaining 42 inline-test files | Queued; see §C |

## A. Completed

`crates/sui-id-store/src/commands.rs` had 1,965 lines of inline
`#[cfg(test)] mod tests`. Split into `commands.rs` (production only,
`mod tests;`), `commands/tests.rs` (registry-level consistency tests),
`commands/tests/runner.rs` (per-command `Database`-backed tests).
Verified as a content-preserving move, not merely a green test run —
see `.git-exclude/reviewed/project-manners-recap-2026-09-10.md` §2.

## B. Split `commands/tests/runner.rs` — delivered `520ee79`

Delivered as specified. Two errors in this section's own tables were
found during the split and are corrected in place below, both marked
**[corrected 2026-09-10]**. Neither changed a destination except the
one that §B-2 now fixes.

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
| `runner/passwords.rs` | U06 (795–893), U09 (1134–1282), U10 (1283–1423) | 350 |
| `runner/mfa.rs` | U07 (894–1073) | 180 |
| `runner/refresh.rs` | T04/T09 (1424–1633) | 210 |
| `runner/policy_markers.rs` | U30, O01 (1634–end) — see §B-2 | 55 |

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

**[corrected 2026-09-10] Two errors in the tables above.** Found by the
implementation role during the split and corrected here rather than left
to be rediscovered:

1. U09's range read 1134–1240, leaving 1241–1282 unaccounted for. That
   gap is `u09_injected_failure_before_append_rolls_back_credential_and_revocations`,
   a real U09 test. The destination was unaffected — all of U09 belongs in
   `passwords.rs` — but the range was wrong.
2. `refresh.rs` read "T04/T09 (1424–end)", which swept in two tests that
   are not refresh-token tests. See §B-2.

**A handoff table is an instruction, not a fact.** Where one contradicts
the code, the code wins: prefer the `// ──` section banners over any line
range given here, and report the discrepancy rather than absorbing it.
That is how both errors above surfaced.

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
sufficient evidence for a move-only change — a silently dropped test still
leaves the suite green. Prove content identity three ways; each closes a
different silent failure.

**[corrected 2026-09-10]** This section previously specified a
whole-stream hash after whitespace and trailing-comma normalisation. That
works only for a 1:1 or 1:2 move: any regrouping adds `use super::*;` and
`mod` lines and reorders items, so the hashes cannot match for reasons
unrelated to content. A sorted-character multiset is **not** the fix — it
is invariant under every permutation, so `assert_eq!(expected, actual)`
and `assert_eq!(actual, expected)` compare equal. Compare per item
instead, so reordering is irrelevant by construction rather than by
invariance:

1. **Per-item bodies.** Parse both sides into `fn name -> brace-matched
   body`, normalise whitespace and rustfmt's trailing commas, compare the
   maps. Report the function count, the names missing/added (expect none),
   and the bodies differing (expect none).
2. **Per-item attributes.** Compare the contiguous attribute lines
   preceding each `fn`. A dropped `#[tokio::test]` removes a test from the
   run while leaving every character of its body in place — the suite
   stays green because the test is no longer a test. Report the
   `#[tokio::test]` count on both sides.
3. **Module declarations.** Every new `.rs` file must be named by a `mod`
   declaration, and every declaration must have a file. An undeclared file
   is not compiled and raises no error. Report the exact set match.

Alongside the usual gates: `fmt --check`, both clippy scopes, `cargo test`
default and `--all-features` with the 160/164 counts, and MSRV 1.95 —
which is what actually exercises the deep glob chain on the floor
toolchain.

## B-2. Correct two misfiled tests in `runner/refresh.rs` — delivered `74ef5c2`

`runner/refresh.rs` currently holds, besides T04/T09:

- `u30_protocol_inserts_session_with_no_audit_row`
- `o01_operational_enqueues_email_with_no_audit_row`

Neither is a refresh-token test. They are the `WriteTx<Protocol>` and
`WriteTx<Operational>` policy-marker proofs — "a command of this class
mutates without an audit row" — which use a session insert and an outbox
enqueue only as vehicles. They landed in `refresh.rs` because §B's table
said "T04/T09 (1424–end)" and the pre-split file has no section banner
between the T04/T09 block and these two. The handoff was wrong; the
execution followed it correctly.

**Move both into a new `crates/sui-id-store/src/commands/tests/runner/policy_markers.rs`**,
declared from `runner.rs` alongside the other seven.

`t09_protocol_issues_initial_token_with_no_audit_row` **stays** in
`refresh.rs`. It is Protocol-classed, but it is genuinely a refresh-token
command, and the domain is the grouping axis — not the policy marker.

**Why this is worth a commit of its own.** The argument for grouping by
domain rather than by wave was that a domain grouping keeps answering
where new tests go. A file named `refresh.rs` holding two policy-marker
proofs stops answering it, and the next `WriteTx<Operational>` or
`WriteTx<Bootstrap>` proof has nowhere obvious to land. `policy_markers.rs`
is that home.

**Same constraints and same evidence as §B**, including the per-item and
attribute checks — two functions is not a reason to relax either.

### Final layout of `commands/tests/runner/`

`runner.rs` holds the shared fixtures and eight `mod` declarations.

| File | Commands | Lines |
|---|---|---|
| `key_rotation.rs` | K01 | 142 |
| `lockout.rs` | U22, U08 | 149 |
| `chain_integrity.rs` | Class-A chain invariant | 53 |
| `user_admin.rs` | U01–U05 | 420 |
| `passwords.rs` | U06, U09, U10 | 390 |
| `mfa.rs` | U07 | 181 |
| `refresh.rs` | T04, T09 | 210 |
| `policy_markers.rs` | U30, O01 | 57 |

Where the commands still to come belong: U12, `mfa.disable` and
`mfa.recovery_codes_regenerate` to `mfa.rs`; the next
`WriteTx<Operational>` or `WriteTx<Bootstrap>` proof to
`policy_markers.rs`; U11 to `passwords.rs`, or to a new `email.rs` if
RFC 101 gives email its own command family.

**Compare the whole module subtree, not the touched files.** A per-file
identity comparison passes even when a function moves between two files
that both changed, or is dropped from one and duplicated into another.
Collect every `.rs` in the subtree on both sides, abort on a duplicate
function name, and compare the union. That is what §B-2's review ran, and
it is what §C should run for `registry.rs`.

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
