# Cycle E — proposed for authorization

> ## Whose turn it is: **not the architect's.**
> This proposal and the two decisions in §C await authorization.
> Nothing is dispatched on it. The three tidies are the one open dispatch and
> they are maintenance, outside any cycle.

**Proposed.** 2026-10-20 JST, by the architect.
**Previous cycle.** D — `0.81.0`, released and published 2026-10-20.

## A — this proposal is overdue, and by my own terms

`ROADMAP.md`'s decision section says, in the architect's own words: *"decide D
and E at the end of cycle C, when M2a's real cost is known, rather than
trading two months of the critical path against an estimate today."*

**Cycle C closed 2026-10-03 and cycle D has now shipped.** The condition I set
was met seventeen days ago and I did not come back to it. That is the same
failure as the three tidies, which sat held behind a schedule line that had
stopped naming them: a deferral with a trigger nobody checks is a deferral
that expires silently.

## B — a defect in the schedule of record, found while writing this

The two decisions below are stated in `ROADMAP.md` against **RFC numbers that
were reassigned**:

| The decision text says | What that number actually is now |
|---|---|
| "RFC 120 (the store boundary)" | `120-consent-and-setup-prove-the-caller` — **Implemented, v0.79.0** |
| "RFC 121 is a feature" | `121-a-verification-failure-is-not-a-pass` — **Implemented, v0.79.0** |
| "RFC 119 ships no code at all" | correct — `119-embedding-contract`, still Proposed |

Two of the three RFCs the decision paragraph discusses shipped as security
fixes with nothing to do with either subject. **RFC 119 `:90` repeats the same
error** — it calls the store boundary "RFC 120 … not yet written".

**The store boundary has no RFC and is unwritten.** The next free number is
140; I have not reserved one, because whether the work is scheduled at all is
decision 1 and is not mine.

Corrected in both documents in this proposal's own commit. Had it gone
uncorrected, a decision would have been taken against a table that cannot be
checked.

## C — the two decisions, restated accurately

Unchanged in substance from `ROADMAP.md`; only the names are repaired.

1. **Is the embeddable work an authorized exception to the feature freeze?**
   The freeze admits urgent security fixes, remediation-enabling refactors,
   dependency and security maintenance, and repairs keeping approved gates
   executable. The store boundary has a real claim to the second category;
   the embeddable module itself does not; RFC 119 ships no code.
2. **If it proceeds — does the store boundary come before or after M2b?**
   The trade is unchanged and still cuts both ways: M2b converts settings,
   pending settings, federation configuration and client metadata onto the
   Class-A seam, all store code, so **boundary-first means M2b is written
   against the new boundary, and boundary-second means refactoring M2b's own
   work.** The technical argument favours before; the programme argument
   favours after.

## D — the recommendation: cycle E is RFC 094 M2b, and the freeze holds

> **SUPERSEDED 2026-10-20** by
> [`cycle-e-and-0.83-revised-2026-10-20.md`](cycle-e-and-0.83-revised-2026-10-20.md).
> This section named M3 as a milestone not blocked by M2b and then did not
> weigh it as a candidate. M3 is Accepted, its entry gate has been open since
> 2026-10-05, two of its stages have landed and **two remain** — it fits a
> release, which M2b's 77 planned rows do not. The revision recommends M3 for
> `0.83.0` and M2b as the programme after it. The critical-path argument
> below is unchanged and still correct; only the order is.

**M2b is the only remaining work that unblocks anything.** The chain, each
link read from the documents rather than recalled:

```
M2b ──> 096-B2 ──> M4-B
         (RFC 096 :20 requires M2b)
     └─> RFC 097 / M5  (blocked on "096-B completing")
          └─> M6 ──> M7
```

Everything from M4-B to M7 sits behind it. M2c and M3 are the only remaining
milestones that do not.

**Measured scope, today, not recalled** — G17 on `18f205f`:

```
check-write-commands: all conditions satisfied
  (102 rows: 25 sealed, 77 planned; 98 implemented)
```

M2b's own exit criteria from the milestone table: convert settings, pending
settings, federation configuration and client metadata; land the `syn` AST
boundary gate; make the structural gate blocking; correct the audit-chain
claims.

**A visible symptom of how long this has been deferred.** `docs/threat-model.md`
is accurate as of **v0.26.0**. I updated its banner to say v0.81.0 during this
release — the fifty-fifth minor version it has not been checked against. It is
RFC 097's artifact, RFC 097 is blocked on 096-B, 096-B2 is blocked on M2b. The
threat model does not get written until this moves.

**So: hold the freeze, and spend cycle E on M2b.** That answers decision 1
"no, for now" and makes decision 2 moot rather than resolved — which is worth
saying plainly, because a decision made moot can be reopened and one recorded
as settled usually is not.

**What I am not proposing, and why.** I am not proposing M2b as a single
cycle-sized lump. 77 planned rows is not a cycle, it is a programme. If this
is authorized, the next thing I bring is a staged M2b plan with a first
tranche and its gate, in the shape 096-B1's stage plan took — which worked:
twelve stages, each independently reviewable, each with its own evidence.

## E — what is still owed, none of it blocking

- **§B of the `0.81.0` proposal** — the wording of the two released-version
  `### Security` entries, the advisory question (recommendation: no), and
  **the rule change**, which is the only one of the three that prevents a
  repeat of the disclosure gate being routed around.
- The held CLI registration-token severity and timeline.
- The M3 veto on the restored FAQ sentence.
- Stage 14's upstream and credentials — a `:23` closure item for RFC 096,
  not a release gate, and now the last thing between 096-B1 and its closure
  record.

## F — what I will do without further authorization

Finish the three tidies' round and commit it. Keep the schedule current. Bring
the staged M2b plan **if** cycle E is authorized as M2b. Dispatch nothing from
sections C or D before that.
