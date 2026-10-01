# The finding this RFC answers

**Source.** The independent design review of [RFC 121](../../done/121-a-verification-failure-is-not-a-pass.md),
2026-09-29, by the implementation role. It was asked to review a *display*
defect and found that the control whose display was being corrected did not
implement its own stated property.
**Proved, not argued.** The reviewer demonstrated it by execution in a
disposable worktree, disclosing that exception to its read-only scope before
using it. The architect confirmed it independently by reading, and added the
deletion and truncation consequences.
**Status.** **Closed** — `fc356bb` (stage 1) and `37931e4` (stage 2), 2026-09-30.
This record exists because RFC 000 requires an Accepted RFC's independent design
review to be durable, repository-relative evidence rather than a claim.

**Why this file is an extract.** The same review carried findings that belong to
RFC 121 and are still open. What is here is the part RFC 125 answers, and that
part is fixed.

## The finding

`verify_chain_tail` recomputed each row's hash from **that row's own `prev_hash`
column** and compared it to that row's own `hash` column. It never compared row
N's `hash` against row N+1's `prev_hash`, and never checked `seq` for gaps. The
linkage that makes the log a chain was written by `append` and read by nothing.

Against the attacker the module itself names — raw SQL access, no application
code — the consequences were:

| Attack | Before |
|---|---|
| Rewrite a row, recomputing that row's own hash from its own unchanged `prev_hash` | undetected, one `UPDATE` |
| Delete a row | undetected |
| Truncate the log | undetected |

Measured: `ChainVerifyReport { checked: 3, broken_at_seq: None, legacy_unhashed: 0 }`
after a single-row rewrite. What the check did catch was a modification that
**failed** to recompute the row's hash — a careless attacker, and nothing else.

## Why it survived review for so long

The existing test, `tampering_with_a_row_makes_chain_verification_fail`, tampered
a row's `action` **without** updating its `hash`, so it was caught by the
single-row formula check — the only check there was. Its own comment reasoned
about chain linkage the code never performed. To anyone reading the suite, it
read as coverage of the property. It is renamed and its comment corrected by
this RFC (D6), because a test that misdescribes what it proves is how a gap
hides.

## Three statements in the tree were false

The module doc (*"to rewrite or delete row N you must recompute every subsequent
row's hash"*), the migration comment (*"breaks the chain at the next row, which
is detectable"*), and `ROADMAP.md` §S2's record of the chain as tamper-evident
within its trust boundary. It was not tamper-evident within that boundary
either. All are corrected — which is as much the point of this RFC as the code
is, and is the [RFC 098](../../done/098-documentation-authority-reconciliation.md)
shape applied to a doc comment instead of a document.

## A second residual, found in the fix

Stage 1's fix skipped a row whose `hash` column was empty, reading it as a
pre-v0.17.0 legacy row, **before** checking its linkage. The architect
demonstrated that blanking a hashed row's `hash` and relinking its successor
therefore removed that row from verification in two writes, raising only
`legacy_unhashed`, which no surface displayed:
`ChainVerifyReport { checked: 2, broken_at_seq: None, legacy_unhashed: 1 }`.

Stage 2 closed it: hashing turned on once and never off, so a legacy row can
never legitimately follow a hashed one. The rule needed one line the dispatch
did not name — `seen_hashed` is seeded from the boundary row one older than the
window — without which a demoted row at the window's oldest edge would still
have been accepted. Re-running the architect's probe against stage 2 reports
`broken_at_seq: Some(2)`.
