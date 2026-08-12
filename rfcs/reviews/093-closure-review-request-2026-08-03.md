# RFC 093 — independent closure review

**Date:** 2026-08-03 (Asia/Tokyo)
**Requested by:** `@nabbisen` (accountable owner and approver)
**Requested of:** `codex-independent-architecture-security-reviewer` (OpenAI Codex)
— named in RFC 093 as its **Independent security and closure reviewer**
**Baseline:** `1d58da2`

## Why you are being asked

RFC 093's own `Closure prerequisites` require it:

> "M1a closes when G01–G09 pass on one clean commit; M1b closes when G10–G12 pass on
> one clean commit. Every mandatory lane in Gate Matrix v1 passes; LDAP smoke and
> mdBook pass; RFC integrity reports no known debt; **independent closure review
> confirms that the legacy audit diagnostic is not represented as structural
> assurance.**"

Both milestones' exit gates are now met. The closure review is the only prerequisite
outstanding, and it is the one that cannot be self-supplied.

**Independence required.** The Anthropic high-capability model reviewed every
implementation step of M1a and M1b, amended RFC 093 three times (MSRV 1.91→1.95,
adding G07b, correcting the G10a command), and wrote the handoffs. It cannot review
whether that work overclaims.

## Evidence to verify

| Milestone | Commit | Run | Result |
|---|---|---|---|
| M1a | `474d0f2` | `30546346612` | 18/18 jobs success |
| M1b | `1d58da2` | `30754447964` | 17/17 jobs success |

Intermediate runs: `30555102811` (`b0cca2f`, C0), `30688197346` (`698cc66`, C1),
`30735008212` (`bfbb2e4`, C2/C3/C4), `30744684897` (`afc999e`, C2.1).

Per-lane evidence is recorded in each job's log: `gate`, `event_commit`,
`checked_out_commit`, `runner_image`, `rustc_version`, `command`, `exit_status`.
RFC 093 requires `event_commit == checked_out_commit`, and forbids assembling
evidence across commits.

## The question you are specifically asked to answer

**Is the legacy audit diagnostic represented anywhere as structural assurance?**

`scripts/check-audit-matrix.sh` compares event-name strings between Markdown and
Rust. RFC 093 §Background states what it does *not* prove: that a mutation emits an
event, that the event is typed, or that mutation and append share a transaction.
RFC 094 owns the authoritative replacement and is not implemented.

A known false positive remains live: `user.reset_mfa` versus `mfa.admin_reset`.

Check whether any of the following represents that script as more than a diagnostic
— `README.md`, `docs/`, `ROADMAP.md`, `CHANGELOG.md`, `rfcs/accepted/093-*`,
`.github/workflows/ci.yml`, and the job names and comments in CI.

## Also worth your attention

The following are known and recorded, not hidden. Judge whether any is
closure-blocking:

- **`README.md` has 26 links, all external**, so G10b — which RFC 093 names it a
  target of — validates none of them.
- **Eight links inside the mdBook point outside it.** They resolve on disk and on
  GitHub; they break in the rendered book. G10a does not validate link targets.
- **`rfcs/README.md`'s template omits `Accountable owner and approver`**, which G11
  requires. An author following the documented template writes an RFC that fails the
  gate.
- **RFC 000 and RFC 018 are both "RFC lifecycle policy"**, both Implemented, with no
  supersession recorded. Analysis:
  `.git-exclude/reviewed/rfc-000-018-lifecycle-policy-analysis-2026-08-02.md`.
- **The advisory G12 reports have no negative fixtures.** Deliberate — they are
  advisory, not blocking — but you may disagree.

## Judge the reviewer as well as the work

The Anthropic model's own closure records list **five defects of its own** across
M1b: a rule attached to a commit where its precondition did not hold; an item
sequenced before the gate that measures it; a repair instruction naming the wrong
one of two divergent documents; and **two cases where it reported a verification it
had not actually performed** (`#backups`, and the C5 annotation check). All five
were caught by the implementer checking a stated premise rather than acting on it.

Worth asking: **is that pattern contained, or does it appear in evidence that was
not independently checked?** Its records are in `.git-exclude/reviewed/`, notably
`m1a-closure-record-2026-07-30.md` and `m1b-closure-record-2026-08-03.md`.

## Required output

**Path:** `rfcs/reviews/093-closure-review-2026-08-<DD>.md`

**It must be committed and tracked.** G11 verifies the `Closure evidence` target
resolves, is tracked, and is **not** gitignored — so a `.git-exclude/` path will
fail the gate. This is why the request lives in `.git-exclude/` but your output
cannot.

**Outcome vocabulary:** Approved / Conditionally Approved / Corrections Required /
Design Revision Required / Requirements Clarification Required / Human Owner
Decision Required.

## If you approve

RFC 093 then moves `rfcs/accepted/` → `rfcs/done/` with `Status: Implemented`, plus
`Closure reviewed on`, `Closure approved by`, and `Closure evidence` linking your
document. G11 enforces all three for a security-required RFC at or above 093, so the
move fails without them.

**Do not approve on the strength of green runs alone.** The milestone's own thesis
is that an unexercised gate is not assurance; that applies to the gates themselves.

---

`.git-exclude/review-requests/093-closure-independent-review-request-2026-08-03.md`
