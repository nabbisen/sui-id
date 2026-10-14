# RFC 096-B1 stage 10a fix round — a row the verifier can actually read

**Dispatched.** 2026-10-14 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Reviews.** First round
[`stage10a-the-federated-mfa-verifier-and-its-candidates-2026-10-14.md`](stage10a-the-federated-mfa-verifier-and-its-candidates-2026-10-14.md);
result in `.git-exclude/reviewed/rfc-096-b1-stage10a-the-verifier-reads-a-row-it-cannot-read-2026-10-14.md`.
**Baseline.** Unchanged — `194ee20270705d92d772f7c5c41633d0843e0bed`. Keep
working in the same tree; it is still uncommitted and that is correct.

The package is accepted in substance: the placement decision, the candidate
family, the two binding properties, the four fixtures and the self-found
`Malformed` fix all stand. Seven items below, three of them blocking. Two of
the first round's items were my errors and are listed in §C of the result,
not here.

## 1 — (blocking) make the row type able to hold a terminal row

Migration 0047's CHECK forces `continuation_sealed IS NULL` once `status`
leaves `'pending'`, and `FederationMfaPendingRow.continuation_sealed` is
`Vec<u8>`. I measured what that costs:

```
PROBE find_by_id(terminal) = Err(Db(InvalidColumnType(5, "continuation_sealed", Null)))
```

The terminal `UPDATE` is accepted by the table; the row is then unreadable. So
`verify`'s `row.status != Pending` guard cannot fire, the `status` field you
added cannot hold the value it was added to observe, and stage 10b's F03
terminalizes rows nothing can read afterwards.

Make `continuation_sealed` an `Option<Vec<u8>>`, matching the column and the
CHECK the table already enforces. `insert_within_tx` still writes it present —
that is the only state it constructs. Then prove the guard: a test that
terminalizes a row and asserts `verify` returns `BindingFailure` **through the
status branch**, not merely that it returns that value. With item 3 done, the
branch is distinguishable from the error path and the assertion means
something; without item 3 it would not, which is why these two travel
together.

## 2 — (blocking) say what happened to stage 9's R3, and leave no false doc

`insert_within_tx` now names `status` and `failure_count`. Stage 9's fix round
added `a_valid_row_defaults_status_failure_count_and_primary_method` because
it did not, and that test's doc still says the columns are "deliberately
omitted" and that it "is what would fail if a later table rebuild dropped
one". For two of the three it now reads back what the test itself supplied.

Naming them is defensible and I am not asking you to revert it. I am asking
for the two things that go with reversing an accepted decision: rename the
test and correct its doc so it claims only the column it still protects
(`primary_method`), and state the reversal in the package — one paragraph on
why explicit is better here than the `DEFAULT`s stage 9 was accepted on. A
package that changes a prior stage's reviewed decision has to say so; `grep`
over this one for `R3`, `DEFAULT` or `insert_within_tx` finds nothing.

## 3 — (blocking) stop laundering every store error into `BindingFailure`

Seven `map_err(|_| BindingFailure)`. A database outage, a decrypt failure and
a corrupt recovery blob are currently the same value as a wrong cookie.

The local path you otherwise copy faithfully does not do this — `mfa::verify_pending`
(`mfa.rs:215-227`) propagates with `?` and uses its generic refusal only for
absence. RFC 096 `:754-755` asks for an unbound generic error on *binding
failure*; an internal error is not one. Fail-closed does not require
discarding the distinction: the attacker-visible response is identical because
the handler maps both to the same page.

Two cases. `BindingFailure` unchanged — still one value for all six binding
refusals, still proven by `every_binding_failure_is_the_same_value`. Plus an
internal case the caller maps to 500 and logs. Item 1 is the argument: that
defect was invisible for exactly as long as the error had no shape of its own.

## 4 — the `Expired` doc argues from a principle the module contradicts

Its reasoning — "by the time expiry is checked the row's identity has already
resolved … nothing left to hide" — would equally exempt disabled user,
disabled provider and non-pending status, all checked after the digests pass.
Those are `BindingFailure`, correctly: `:752-753` puts the user and the
provider inside the binding gate.

The outcome is right; delete the generalization and keep the justification the
same doc already gives — `:740-741` makes expiry `Invalidated`, `Invalidated`
is a write, 10a writes nothing, so it is handed on.

## 5 — fix the fixture citation

`federated_mfa.rs:53` cites
`federated_mfa_candidate_cannot_be_constructed_directly.rs`; the file is
`…_valid_totp_cannot_be_constructed_directly.rs`.

## 6 — prove the third derive

The module doc calls `:757-758` "the absence of those three derives" and
proves two. Add a `Serialize` fixture — `serde_json::to_string(&v)` on a
`ValidTotp` parameter, same shape as the `Debug` one.

## 7 — drop `PartialEq for FederatedMfaOutcome`

It makes two distinct candidates compare equal, and it exists only so
`assert_eq!` works on the whole `Result`. `a.unwrap_err() == b.unwrap_err()`
needs only the hand-written `Debug` you already have. Rewrite the assertions
in `every_binding_failure_is_the_same_value` and remove the impl: a type
family defined by the capabilities it lacks should not gain one for a test.

## What to return

The same package shape. Specifically:

1. The four code changes (items 1, 3, 4, 5–7) with **per-hunk SHA-256 against
   `194ee20`** — the same baseline, since nothing has landed in between.
2. **Item 1's new test**, and say in one line how you made the status branch
   distinguishable from the error path so the assertion is not vacuous.
3. **Item 2's paragraph**, plus the renamed test and its corrected doc.
4. The full matrix again from a throwaway workspace. `sui-id-core --lib` is
   **184** on the returned tree, not 183 — your own `(173 + 11)` is right and
   the total was the slip; reconcile from 184.
5. Anything you think is wrong with this round, same as last time. You were
   right against my §3 instruction on the harness location; that rate is the
   useful number.

## Not in this round

**Stage 10b stays blocked**, and not only on these items. The placement
decision has a consequence the package found half of: F03 cannot take a
candidate, so it will be a `pub` store function taking plain primitives, and
handlers already call `sui_id_store::commands::*` directly in eight places. A
handler could call F03 and never pass the verifier. That is a question about
the design, not about your code, and it is mine and the owner's to settle
before 10b is dispatched. Do not try to solve it in this round.
