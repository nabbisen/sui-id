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

