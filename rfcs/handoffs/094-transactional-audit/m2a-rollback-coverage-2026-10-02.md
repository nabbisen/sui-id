# RFC 094 M2a — rollback coverage and the structural gate

**RFC status: Accepted** (`rfcs/accepted/094-transactional-audit.md`), so
implementation is permitted under RFC 000.

**Everything in this document is dispatched.** There is no exploratory or
"when convenient" section; if a paragraph describes work, build it.

## Where M2a actually stands

M2a's closure prerequisite has five clauses. Measured on `b35adc0`:

| Clause | State |
|---|---|
| C15 is atomic | **met** — validate-first, rollback proven by injected failure |
| raw database access confined to `sui-id-store`, gate-asserted | **met** — `rusqlite` depended on by no other crate, asserted by `crates/sui-id/tests/e2e/r094_m2a_db_access_scope.rs` |
| every converted path uses the approved transaction seam | **met in fact, not in enforcement** — all 24 declared commands reach `db.class_a(…)` with their own `ClassATx<'_, X>`; nothing proves the 25th will |
| injected append failures roll back mutation for every converted row | **13 of 24** |
| the structural gate passes over converted commands | **not landed** |

**How I measured, so you can dispute it.** 24 `command <ID> = "<ID>"` entries in
`crates/sui-id-store/src/commands.rs`. For the seam, each ID appears at a
`db.class_a(context, move |tx: &mut ClassATx<'_, ID>|` site (26 sites; U07 and
U37 have two each). For rollback, I counted **test functions that call
`db.fault_injector().fail_before_next_append()`** and took the command ID from
the function's own name:

- `runner/user_admin.rs` — U01, U02, U04
- `runner/mfa.rs` — U07, U14, U15
- `runner/passwords.rs` — U09, U10
- `runner/credential_lockout.rs` — U09, U10
- `runner/key_rotation.rs` — K01
- `runner/lockout.rs` — U08
- `runner/recovery.rs` — U37
- `runner/refresh.rs` — T04
- `runner/dynamic_registration.rs` — C15

**Eleven have no injected-failure test: U03, U05, U12, U22, L01, L02, L03, L04,
L05, L06, L07.** If any of those is covered by a test whose name does not carry
its ID, my count is wrong and the gate in step 1 will say so — report that rather
than quietly adjusting the list.

## Step 1 — the structural gate, first

Build the gate **before** the eleven tests, not after. Its value is that it
closes the door behind the work: a 25th command added without a rollback test
must fail, and that protection should exist while the eleven are being written,
not only once they are done.

The gate enumerates the declared commands from the registry — not from a
hand-maintained list — and for each asserts both halves of clauses 1 and 4:

1. the command reaches the Class-A seam, and
2. a test injects an append failure for it.

For (2), a name-matching heuristic over test functions is too weak to be a gate;
an ID appearing in a comment would satisfy it. Make the *test* declare its
subject, so the assertion reads a fact rather than a spelling — e.g. a small
attribute or helper that registers `(command_id, test_name)` at test time, with
the gate asserting the registered set covers the declared set. **The mechanism is
yours to choose; the requirement is that the gate cannot be satisfied by naming.**

**The eleven are an explicit exemption list in the gate, with a comment naming
this document.** Each new test deletes one line. The list must only ever shrink,
and the gate fails if an ID not on it lacks coverage.

**This is my call, not `@nabbisen`'s, and here is the reasoning in case he
overrules it:** the alternative is to land the gate last, which keeps `main`
honest (no gate passing while coverage is incomplete) but leaves the regression
door open for the whole of the eleven. An exemption list that can only shrink
records the debt in the enforcing mechanism itself, where it cannot be forgotten,
and that is the stronger position. The cost is that the gate is green while M2a
is incomplete — acceptable because M2a's *closure* is gated on the list being
empty, which is a separate check from the gate passing.

## Step 2 — the eleven tests

One test per command, following the shape RFC 094 §"Two lessons" requires and
`c15_injected_append_failure_leaves_the_token_unspent_and_no_client_row`
demonstrates:

- the command must have **genuinely mutated** before the injected failure;
- snapshot the relevant domain rows **and** the audit tail before and after;
- assert the mutation is absent **and** no audit row was appended;
- never assert merely that the function returned `Err`.

RFC 094 records why: the one pre-existing rollback test passed for an unrelated
reason, because its scenario used a duplicate-key conflict that SQLite's `ABORT`
resolution fails at statement level — so "nothing new persisted" held whether or
not the transaction rolled back. **A test that would still pass if the rollback
were removed is worse than no test.** For each of the eleven, delete the rollback
and confirm the new test fails; report the failure output, as you did for the
consent-policy e2e.

The seven `L0x` commands are a cluster — if their rollback shape is genuinely
identical, say so and parameterize rather than copying seven near-duplicates.
That is a judgement I want you to make and state, not one I am prescribing.

## Not in scope

M2b (the remaining Class-A conversion, the AST boundary gate, the authority
switch). The two open design questions from the consent-policy work — whether
`first_time_only` is right for dynamic clients, and whether enabling a dynamic
client should surface its consent policy — are **`@nabbisen`'s, still open**, and
no code should anticipate either answer.

## Return

The usual: per-hunk SHA-256 over unified-diff text against a stated baseline, the
gate's own output, and the revert-and-rerun evidence for each of the eleven.
