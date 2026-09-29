# The audit chain must be verified as a chain

**RFC.** RFC 125, **Proposed**. Urgent integrity fix; it precedes RFCs 121–124.
**Implementer.** Mid-capability model — the role that found and proved it.
**Baseline.** `5dba3df` or later.

## Why this handoff was staged outside git until now

The same reason as RFC 120's: the repository is **public**, and a tracked
document saying the audit chain does not detect tampering tells an attacker who
already holds the database that they need not be careful. It moved here in the
same commit as the fix, which is when the description became safe.

## Stage 1 — landed, and one residual found in the fix itself

Stage 1 is in the same commit as this file. Verified by the reviewer: 13 of 13
hunks match, nothing unclaimed; 1020 tests, every gate and `mdbook` re-run.
Rewrite, deletion, truncation and the window edge are all caught, each by a test
that fails on `5dba3df`.

**Stage 2 exists because the reviewer found a path through the new code**, and
demonstrated it rather than asserting it — a temporary probe in this checkout,
run and then removed, with `audit.rs` restored and its SHA-256 shown to match:

```
ChainVerifyReport { checked: 2, broken_at_seq: None, legacy_unhashed: 1 }
```

**A row whose `hash` column is empty is skipped as a pre-v0.17.0 legacy row,
before its own linkage is checked.** So blanking a hashed row's `hash` and
relinking its successor takes that row out of verification in **two** writes, and
its content is then unverified. The only signal is `legacy_unhashed` rising, and
no surface shows it.

**Stage 2, dispatched: legacy rows form a contiguous prefix.** Once a hashed row
has been seen in the walk, an empty `hash` is a break. That is the whole rule. It
is two lines, and it closes the path completely — demoting a row would then
require blanking every row before it, which destroys the prefix it was hiding in.

Also in stage 2:

- **A test for exactly the probe above**, failing before the rule and passing
  after. The probe's shape is in this file; write it properly.
- **A mutation** removing the rule, naming the test that catches it.
- **`legacy_unhashed` is not a free-floating number.** Decide and state whether
  verification should report it as part of the result an operator reads, given
  that it is now the signal for a specific attack shape. RFC 121 owns the
  surfaces; say what you would have it show, and it becomes an input to that RFC
  rather than a second design decision there.
- **The three documents I corrected on your behalf are narrowed further** once
  the rule holds: `docs/threat-model.md`'s residual bullet, the module doc's
  "one further gap", and `ROADMAP.md` §S2's residual paragraph each become the
  stronger, true statement. **Do not delete them — rewrite them.** A removed
  caveat reads as though the limit never existed.

## What to build

RFC 125 D1–D7 are the specification. The measurement is already yours; this
handoff adds only what the architect found on top of it:

**Deletion and truncation, not just rewrite.** Your finding was the single-row
rewrite. The architect confirmed by reading that `prev_hash` is compared to
nothing anywhere, and that `seq` gaps are never checked, so **deleting a row and
truncating the log are equally undetected**. D2 exists for that. Please confirm
both by execution as you did the rewrite, since the same worktree device applies.

**The window edge (D3) is the part most likely to be got subtly wrong.** Rows
arrive newest-first. The oldest row in the window has no predecessor in it, and
treating that edge as verified is how a "verified intact" that means less than
it says gets reintroduced by the fix meant to remove it. Say which of the two
options in D3 you chose and why.

**Do not widen scope.** Not an external anchor, not a signature, not a new
column. If the honest fix needs a schema change, stop and say so before building
it — RFC 112 has just made an older binary refuse a newer database, so a
migration inside this fix costs more than it looks.

## Evidence

- **Three tests that fail at the baseline and pass after**: a rewrite whose own
  hash was recomputed; a deleted row; a truncated head. Show both results, as
  RFC 120 did.
- A test for the genesis row and for a later row with an empty `prev_hash` (D4).
- A test for the window edge, asserting whichever D3 behaviour you chose.
- The corrected existing test (D6), with its comment saying what it actually
  proves.
- Mutations: remove the linkage comparison; remove the sequence check; treat the
  window edge as verified. Name the test that catches each.
- fmt, both clippy scopes, the workspace count before and after, MSRV, and every
  doc gate. **G13 and G15 both read the audit documentation you are correcting.**

## The documents (D7)

`repos/audit.rs:4-10`, `migrations/0009_audit_hash_chain.sql:5-7`, `ROADMAP.md`
§S2, and any page under `docs/` that repeats the claim — find them; do not trust
this list to be complete. Each becomes what is true after the fix. Where the
honest sentence is narrower than the old one, ship the narrower sentence.

## What to return

The usual package, in `.git-exclude/` — **it describes the defect, so it stays
there**. Include the before/after for all three attacks, your D3 choice with its
reasoning, and the full list of documents you found making the claim.

**Nothing you write that will be committed should describe the attack**, only
the invariant and what the tests assert. The RFC 120 package's §11 scan is the
model.
