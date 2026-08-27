# RFC 093 — closure review

**Date:** 2026-08-27 (Asia/Tokyo)
**Reviewer of record:** requirements-architect role, **on the owner's explicit
direction of 2026-08-27**, and **not independent of checks 1–3** — see §1.
**Approver:** `@nabbisen` (accountable owner)
**Evidence run:** `31571776585` — `success`, head `0fcb423`, **17/17 jobs**
**Outcome:** RFC 093 closes. M1a and M1b are complete and their closure
prerequisites are satisfied.

## 1. The independence position, stated first because it qualifies everything below

RFC 093's closure prerequisite is that **an independent review** confirms the
legacy audit diagnostic is not represented as structural assurance.

**That is not what happened, and this record must not be read as if it were.**

Checks 1–3 examine `docs/src/reference/audit-coverage-matrix.md`, whose
correction (B-093-1) I authored. I also reviewed every M1a and M1b implementation
step and amended RFC 093 three times. For checks 1–3 I am the author of the work
under review, and check 3 — whether any *other* document repeats the overclaim —
is precisely where an author is least able to see their own omission.

The owner was offered both routes on 2026-08-12 and again on 2026-08-27: confirm
the checks himself, or route them to the implementation role, which authored none
of this. **He directed that they stand on my run.** That is his decision to make
as accountable approver, and it is recorded here rather than smoothed over.

What that buys and does not buy: the checks are mechanical and their results are
reproducible from the commands below by anyone. What is missing is a second
reader's judgment on **completeness** — whether the sweep in check 3 looked
everywhere it should have. I have run it twice, three weeks apart, with the same
result. Twice by the same party.

Under RFC 018 as amended, independence is role independence and this review does
not have it. Recorded as an unreviewed judgment carried by the owner, not as a
completed independent review.

## 2. The five checks

### Check 1 — does the matrix still claim CI proves coverage? **No.**

- Requirement and enforcement claim are separated (lines 3–11).
- The retracted sentence *"…without updating the matrix is a CI failure"* is
  **absent**.
- The vacuous-pass statement is **present**: *"a privileged operation that emits
  **no** audit event — there is no literal to compare, so the gate passes."*

*Method note.* My first pass today reported the third bullet as soft, on a
single-line grep. The sentence wraps across two lines. Confirmed by
whitespace-normalising the document rather than pattern-matching a line — the
same defect class as the `#backups` miscount earlier in this programme, caught
here before it reached the record.

### Check 2 — RFC 093's mandated wording, verbatim? **Yes.**

Extracted the mandated sentence from RFC 093 itself and compared it to the matrix
document after stripping blockquote and bold markers:

> Diagnostic only: string parity proves neither emission completeness nor
> mutation/audit atomicity. RFC 094 owns the authoritative structural gate.

Present, verbatim. A plain `grep` would have matched "Diagnostic only" without
proving the rest of the sentence is intact; this compares the whole mandated
string.

### Check 3 — does any other document repeat the overclaim? **No.**

Seven tracked files mention `check-audit-matrix`. Each was read, not counted:

| Location | Verdict |
|---|---|
| `docs/…/audit-coverage-matrix.md` | The corrected document |
| `rfcs/done/088` | States the gate "is not directly relevant here" |
| `rfcs/done/085` | "greps event-name constants and asserts (a)…(b)" — accurate |
| `CHANGELOG.md` | Historical release records, describing string parity accurately |
| `rfcs/accepted/093` | This RFC mandating the diagnostic wording — the fix itself |
| `rfcs/reviews/093-closure-review-request-2026-08-03`, `093-closure-review-2026-08-12` | Dated review records |

**This is the check whose completeness I cannot vouch for** — see §1.

### Check 4 — is the evidence real? **Yes.**

`gh run view 31571776585` → `conclusion=success`, `headSha=0fcb423`,
`jobs={"success":17}`. Exactly as recorded at M1b closure.

### Check 5 — are the non-blocking items still non-blocking? **Yes, and two are now fixed.**

| # | Item | State 2026-08-27 |
|---|---|---|
| a | README external links outside G10b's scope | Open — now **44**, not the 26 recorded on 2026-08-12. G10b checks local resolution only |
| b | In-book links resolving on disk but not in the rendered book | Open; `check-markdown-links` passes over `docs`, G10a builds the book and passes |
| c | `rfcs/README.md` template omitted required fields | **FIXED** — `954efc3`, verified by a negative control |
| d | RFC 000 and RFC 018 shared a title | **FIXED** — `584cefe`; RFC 018 is now the sui-id profile with precedence stated |
| e | G12 has no negative self-test | Open by design — a `[gate_matrix_exceptions]` entry, not a dispatched lane |

The three that remain were assessed non-blocking on 2026-08-12 and the owner
confirmed that assessment on 2026-08-27. None affects a claim RFC 093 makes about
itself.

## 3. What closure does not assert

- **Not** that the audit coverage matrix is mechanically enforced. It is not, and
  RFC 093's own required wording says so. RFC 094 owns the authoritative gate.
- **Not** that the diagnostic's absence from CI was reviewed here.
  `scripts/check-audit-matrix.sh` is referenced by no workflow; RFC 093's
  requirement is conditional on it being "retained in CI", so nothing is
  violated — but RFC 085 in `done/` still describes it as a CI step, which is a
  stale statement in a different RFC and outside this closure's scope.
- **Not** that checks 1–3 had independent scrutiny. They did not.

## 4. Durability

Nothing holds checks 1–3. The correction they confirm could be edited tomorrow
with no gate noticing. Two greps would hold most of it — assert the mandated
sentence present, assert the retracted sentence absent anywhere — and the owner
ruled on 2026-08-27 that they land with **R10**'s resolution, because RFC 093
owns every gate definition and no other RFC can extend a check until that lands.

Recorded here so the gap is visible in the closure rather than only in the
register.

---

`rfcs/reviews/093-closure-review-2026-08-27.md`
