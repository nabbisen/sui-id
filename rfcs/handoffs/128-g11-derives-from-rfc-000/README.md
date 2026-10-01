# G11's conditions derive from RFC 000, not from a retired RFC

**RFC.** [RFC 128](../../accepted/128-g11-derives-from-rfc-000.md), **Proposed** — nothing dispatched.
**Author.** High-capability model, requirements-architect role.

## The trace, so it is not re-derived

- `scripts/check-rfc-integrity.py:658-665` is condition 9: when `Security
  review` is Required, an Accepted RFC must carry `Independent design review`
  with a durable repository-relative reference.
- `contracts/rfc-policy.toml` — G11's own policy data — **never mentions
  independence at all.** The rule is hard-coded in the script.
- `rfcs/archive/018-rfc-lifecycle-policy.md:216` is the sentence it implements,
  nearly verbatim, including the `N/A` prohibition; `:220` defines independence
  as "the reviewer did not author".
- `git log -S'Independent design review' -- scripts/check-rfc-integrity.py`
  gives `3f2eace`, **RFC 093's** G11 checker. RFC 093 and RFC 018 were written
  in the same month by the same architect.
- RFC 000 has the substance and neither the `N/A` prohibition nor a definition
  of independence.

## The audit criterion, sharpened by `@nabbisen` on 2026-10-01

**Ask of each statement: does it presuppose a skilled architect external to this
team?** *"No such a person out of our team."* That is the test — not tone, not
strictness.

**Four RFCs, not ninety-eight.** Measured: 75 RFCs were first added in June 2026
and **none** is classified `Security review: Required`; 23 in July 2026, of which
**three** are — RFCs **094, 095, 096**, all Accepted and all load-bearing in the
live programme. Add **RFC 093**, which owns every gate. Auditing the other 94
would be cost without return, which is the same failure in the opposite
direction.

**And the answer is already partly on the record.** `ROADMAP.md` §S1 states that
`codex-project-architect`, `codex-developer` and
`codex-independent-architecture-security-reviewer` are **agent identities from one
vendor**, and that *"a change authored, implemented and approved by one party has
had no review at all."* So the July-era "independent" reviews were same-vendor
agents. **Do not re-derive this; start from it.** What the audit adds is which
*statements* in those four RFCs only make sense if an external architect exists.

## What D2's audit must produce

One row per G11 condition — there are fourteen live ones (1–7, 9–15) — each
with: what it checks, the live document that requires it, and the `file:line`
where that document says so. **A condition whose only source is RFC 018, or
which has no source at all, is named as such.** Two are already known to need
looking at:

- **Condition 7** (`RFC-MI-*` identifiers): the token `RFC-MI-` appears in
  **neither** RFC 000 nor RFC 018. It may be legitimate — the closed historical
  list lives in `contracts/rfc-policy.toml`, which is data an RFC may own — but
  the owning document has not been identified.
- **Condition 9**: D1's subject.

Conditions 12 and 13 cite RFC 000 in the script itself and are expected to pass
the audit; cite them anyway, because the output of this audit is the citation.

## Stage 0 — audit and understand the current status, before anything is changed

`@nabbisen`, 2026-10-01: *"To audit and understand the current status should be
first."* This stage produces no code and no corrections. It answers, with
`file:line` evidence:

1. **The fifteen mislabelled reviews.** For each of RFCs 094, 095, 096, 102, 103,
   110, 112, 115, 116, 117, 118, 121, 122, 123 and 126: what the review package
   actually contained, classified as **measurement**, **enumeration**,
   **feasibility**, or **a judgement about whether the design was right**. The
   last category is the one that matters, and the architect already knows of
   three — RFC 121's D3 settlement, RFC 123's two-bucket design, RFC 126's
   caller-queues behaviour — all three adopted by the architect, which is how
   they became decisions. **Find the rest. Do not re-argue them.**
2. **Statements presupposing an external skilled architect** in RFCs 093, 094,
   095 and 096 — `@nabbisen`'s criterion, and the reason the audit exists.
   `ROADMAP.md` §S1 is the starting point, not something to re-derive.
3. **Which of those statements are load-bearing now** — a milestone exit gate, a
   closure prerequisite, a gate condition — and which are historical prose. The
   first group blocks the programme; the second costs a sentence.

**Nothing in this stage changes a file under `rfcs/`.** The corrections are D1–D5
and they wait on the audit, because the audit may change what they should say.

## Order

1. The audit (D2), as a package, **before any code changes**. It may change what
   D1 should say.
2. D1 and D3's changes to the checker and the policy.
3. D4's pass over the twenty RFCs carrying the field — **only after
   `@nabbisen` has settled whether an author may review their own RFC** (D3
   leaves it to him deliberately). Until he does, the correct entry for RFC 124
   is the honest one it already has.
4. D5's amendment to RFC 093, in the same commit as the checker change.

## What is not yours to decide

Whether an author may review their own RFC. RFC 128 D3 says the gate stops
supplying a definition RFC 000 does not give; it does not supply a different
one. If the audit turns up an argument either way, **report it — do not settle
it.**

## D2 and D1/D3 dispatched 2026-10-01 — the condition trace, then the checker

Stage 0 is complete: Part 1 found the twelve load-bearing statements (now
re-scoped by RFC 129), Part 2 classified the fifteen packages. **Neither did D2's
trace of G11's own conditions**, which is this dispatch.

**An ordering correction by the architect.** He said he would do D4 — the fifteen
headers — before dispatching this. That was backwards: D4 corrects what those
headers *say*, and D1 changes the shape the field may take, so doing D4 first
means doing it twice. D4 follows this.

### D2 — trace all fourteen live conditions

One row per condition (1–7, 9–15): what it checks, the **live** document that
requires it, and the `file:line` where that document says so. This is
enumeration, not judgement — do not argue whether a condition is good, only
where it comes from.

**A condition whose only source is `rfcs/archive/018-rfc-lifecycle-policy.md`, or
which has no source at all, is named as such.** Two are already known to need
looking at:

- **Condition 7** (`RFC-MI-*` identifiers): the token `RFC-MI-` appears in
  **neither** RFC 000 nor RFC 018. The closed historical list lives in
  `contracts/rfc-policy.toml`, which is data an RFC may legitimately own — but no
  owning document has been identified. Find it or report that none exists.
- **Condition 9**: D1's subject, already traced in the RFC.

Conditions 12 and 13 cite RFC 000 inside the script; **cite them anyway**, because
the citation is the output.

### D1 and D3 — re-derive condition 9

Record **who reviewed** and a **durable repository-relative reference**. That is
all RFC 000 asks. Remove, because their only source is a retired RFC:

- the prohibition on `N/A`;
- **any definition of independence.** RFC 000 requires "a named independent design
  reviewer" and never says what independence is. **The gate must not supply a
  definition RFC 000 withholds** — that is a gate legislating, which is RFC 110's
  fault one layer down. `@nabbisen` has not ruled on whether an author may review
  their own RFC, and the checker must not decide it for him.

**What the field must then accept**, because all three now exist in the tree: a
review by the owner (RFC 129), a review by the architect who authored the RFC
(RFCs 120, 124, 125, 128), and a record that no review occurred (RFC 105). If
your change rejects any of those, it is still legislating.

### D5 — the RFC 093 amendment

In the same commit as the checker change. Dated, attributed to `@nabbisen`'s
approval of 2026-10-01, and **not** to the architect or to this RFC's author.

### Two things from today

Run the gates **through `scripts/ci-gate.sh`**, not by hand. And **hand over a
working tree** — `4f58066` was committed and pushed unreviewed, which is recorded
in RFC 124's handoff; a change to `check-rfc-integrity.py` is the gate that checks
every RFC, so it is the last file that should reach `origin/main` unverified.

## D2 and D1/D3/D5 reviewed 2026-10-01 — accepted, three corrections

Both packages are **accepted**. D1/D3/D5 landed as `9381347`. D2 is a findings
package with no tree change.

**The central finding is verified and it corrects this RFC, which is to say it
corrects me.** `check_accepted_metadata` has never enforced an `N/A` prohibition
or any definition of independence, in any version since `3f2eace` — confirmed
three ways: `git log -S"N/A"` returns only `9381347` itself, the logic at
`3f2eace` is byte-identical to today's, and all twelve `author` hits in the file
are prose. RFC 018's residue was never in the gate's *behaviour*, only in how
RFC 093 and this RFC *described* it. That is a smaller and more specific defect
than the one I suspected, and D2's trace closes the rest: **all fourteen live
conditions trace to a live document** (RFC 000, RFC 093 or RFC 110), so my
"unlikely to be the only inheritance" was speculation and it was wrong.
Condition 7's open question is closed too — RFC 093 introduces the RFC-MI
scheme, so it owns the historical list.

### D7 — D2's RFC 093 citations point into a superseded file

Every citation into `rfcs/done/093-build-toolchain-release-gates.md` in D2's
table resolves against `9381347^` and **none resolves against HEAD**, because
D1/D3's own amendment to that file shifted them (+13 after `@@ -11`, +16 after
`@@ -256`). The two packages shipped together, so the trace points into a version
the same delivery replaced: condition 7's `:254-256` now lands on
`### RFC integrity contract` and a command line. Correct rows 4, 5, 6, 7 and 11
to `:262`, `:263`, `:264-266`, `:267-269` and `:275-276`, or state the baseline
commit in the Method section. Either is acceptable; silence is not — line numbers
are the entire value of a trace.

### D8 — D2's condition 10 citation is wrong under every baseline

`:274-275` is not a shifted reference; it is wrong in both versions. The text it
means — *"Done security-sensitive RFCs created from 093 onward have dated
independent closure metadata"* — is pre-amendment `:260-261`, HEAD `:276-277`.
**The substance is correct and I verified it:** `check_closure_metadata` matches
on neither `independent`, nor author identity, nor `N/A`, so condition 10's
description carries the same imprecision condition 9's did.

Fix the pointer. Do **not** fix condition 10's wording — flagging it while
declining to act, because the dispatch named condition 9, was correct scope
discipline, and that adjacent imprecision is now mine to schedule.

### D9 — D1/D3's "15 RFCs" sentence, and the test comment's missing hedge

Two edits to `9381347`'s own text.

**The enumeration is wrong three ways.** *"all 15 currently-accepted
security-sensitive RFCs (094, …, 102, 103, …)"* lists **19** numbers, includes
**102 and 103** (both in `rfcs/done/`, not `accepted/`), and omits **105 and
120** — which are precisely the two the package's own new docstring cites as the
live examples of a recorded absence of review and of self-review. So the prose
contradicts the comment added in the same commit. The true set is 19: 094 095 096
105 110 112 115 116 117 118 120 121 122 123 124 125 126 128 129. The underlying
claim is true — the logic is unchanged and G11 passes — but the evidence offered
for it is not.

**The test comment drops the docstring's qualifier.** The docstring correctly
ends *"must stay accepted unless `@nabbisen` rules otherwise — that ruling is not
this gate's to make."* The comment above the three new tests in
`scripts/tests/test_rfc_integrity.py` states flatly that *"the gate must accept
all three."* Those tests are what would block implementing such a ruling: if an
author may not review their own RFC, then
`test_independent_design_review_by_the_rfc_author_is_accepted` must go, and a
reader of only that comment would read its deletion as a regression. Carry the
qualifier across, so the pin reads as descriptive of today rather than normative
forever. While there: condition 9's docstring quotes RFC 000's *closure evidence*
clause, which is condition 10's subject — RFC 000 bundles both at `:37-41`, so it
is traceable, but tighten it to the design-review half.

### Two things done right, recorded deliberately

The first draft of the RFC 093 amendment cited archived RFC 018 in a header, G11
caught it, and the package **reported that instead of quietly fixing it** — *"the
gate working correctly against my own draft is a good sign, not friction to route
around."* That is the disposition RFC 110 exists to produce. And G16's third
flagged line was correctly diagnosed as a false positive from a shifted scan
window, *"confirmed by diff, not assumed."* That phrase is the standard; keep
working to it.

### D4 stays where it is

The fifteen headers still wait on `@nabbisen`. Nothing in these two packages
decides anything about them, and nothing in D7–D9 should either. **I first wrote
here that they wait on his ruling on whether an author may review their own RFC;
the section below measures that and corrects it.**

### Correction to D4's blocker, measured 2026-10-01 — not the architect's to settle

I wrote above that D4 waits on `@nabbisen`'s ruling on self-review. **That is
wrong, and the census says so.** Of the 24 RFCs carrying an
`Independent design review` field, the reviewer recorded is:

| Recorded reviewer | Count | RFCs |
|---|---|---|
| the implementation role | **15** | 094, 095, 096, 110, 112, 115, 116, 117, 118, 120, 121, 122, 123, 125, 126 |
| the architect, self, and **said so in the header** | 2 | 124, 128 |
| `@nabbisen` | 2 | 098, 129 |
| none performed, recorded as a gap | 1 | 105 |
| pre-threshold / other shape | 4 | 018, 093, 102, 103 |

**No RFC in D4's set is a self-review.** The self-review question touches only
124 and 128 — both of which already state in the header that the field's name
overstates the document — and the three new acceptance tests. So the ruling I
said D4 waits on does not gate D4 at all.

**What does gate it is the re-review `@nabbisen` deferred.** Those fifteen are
*the same fifteen* that stage 0 part 2 classified, and part 2's finding was that
**every one of the fifteen contains at least one passage that is a judgment about
whether a design was right** — the category he has since ruled is not the
implementation role's to carry. So rewriting one of those fifteen headers is not a
wording choice: it asserts what that review was worth, and that assertion is the
deferred re-review's output, not this dispatch's.

**D4 therefore stays undispatched**, and for a better-stated reason than the one I
gave. The architect's recommendation on how to proceed is `@nabbisen`'s to accept
or reject and is **not recorded here as a decision**; it is in the review result
for these packages. Nothing in D4 is the dev team's to start.
