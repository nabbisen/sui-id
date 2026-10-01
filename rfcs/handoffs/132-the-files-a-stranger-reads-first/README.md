# RFC 132 — handoff

**RFC.** [`../../done/132-the-files-a-stranger-reads-first.md`](../../done/132-the-files-a-stranger-reads-first.md)
**Status of the RFC.** **Accepted** 2026-10-01 by `@nabbisen` ("Accepted."), amended the same day to add D6.

## Dispatch order

This is the smallest of the three open RFCs and blocks nothing, but **D1 goes
first and alone if anything is split**: it is the one finding that actively
defeats the security policy today.

## What to build

- **D0** — no contact route is an email address, anywhere, and no template asks a
  reporter for one. This is already true; keep it true.
- **D1** — `.github/ISSUE_TEMPLATE/config.yml` gains `contact_links` whose first
  entry points a suspected vulnerability at
  `https://github.com/nabbisen/sui-id/security/advisories/new`;
  `blank_issues_enabled` becomes `false`.
- **D2** — `bug_report.yml` gains a redaction instruction immediately above the
  logs field, naming session cookies, bearer and refresh tokens, client secrets,
  signing-key and master-key material, and any real user's address or identifier.
  `Version` becomes required and names `sui-id --version`.
- **D3** — `SECURITY.md`: drop the `--version` parenthetical (the flag exists —
  it prints `sui-id 0.79.0`); change the credit promise to `CHANGELOG.md`, with an
  advisory named only as something that may follow.
- **D4** — `CONTRIBUTING.md`: name the RFC lifecycle; make Releases include
  publishing six crates bottom-up; replace the restated `cargo` commands with a
  pointer to `scripts/ci-gate.sh <GATE_ID>` and `contracts/gate-inputs.toml`; and
  make the proptest sentence name the dispatch D6 adds.
- **D5** — `CODE_OF_CONDUCT.md` names a real private GitHub channel; add
  `.github/pull_request_template.md` carrying the expectations `CONTRIBUTING.md`
  already states.
- **D6** — a second job in **`.github/workflows/fuzz.yml`** (not a new workflow)
  behind the existing `workflow_dispatch`, with its own case-count input, running
  `PROPTEST_CASES=<n> cargo test --workspace`. Leave Argon2's `cases: 4` alone.

## Three things the security review requires of the wording

These are not restatements of the decisions; they are the ways a correct decision
can be implemented badly.

1. **D2 must say *redact*, not *omit*.** Wording that reads as "do not paste logs"
   would make bug reports worse while looking more careful. State that a redacted
   log is wanted.
2. **D3 must not read as withdrawing credit.** The replacement says credit **is**
   given, in `CHANGELOG.md`. What is being removed is a promise the project would
   not keep, not the recognition.
3. **D1's `blank_issues_enabled: false` is the one revertible decision.** If it
   turns people away rather than steering them, revert that line only — the
   `contact_links` half closes the security gap independently and stays.

## What is not yours

Whether a **gate** enforces D4's no-restated-commands rule across every document
is RFC 131 D4, widened on `@nabbisen`'s approval 2026-10-01. This RFC changes the
text of two documents; the gate that holds them there is that one's work. Do not
build the checker here.
## Dispatched 2026-10-01 — stages 1 and 2

**Start here.** This is the first work in the three-RFC sequence. Stage 1 is small
and goes alone; stage 2 follows it.

**The sequence across all three RFCs**, so you can see where this sits:
stage 1 and 2 here → stage 3 in
[`../130-gates-declare-their-input-scope/README.md`](../130-gates-declare-their-input-scope/README.md)
→ stage 4 in
[`../131-two-gate-levels-and-one-rule/README.md`](../131-two-gate-levels-and-one-rule/README.md).
**D4 of this RFC is deliberately not in either stage below** — it is entangled
with RFC 131's D4 and lands in stage 4. Do not do it here.

### Stage 1 — D1 only, as its own change

`.github/ISSUE_TEMPLATE/config.yml`. Add `contact_links` whose **first** entry
routes a suspected vulnerability to
`https://github.com/nabbisen/sui-id/security/advisories/new`, and set
`blank_issues_enabled: false`.

It is a handful of lines, and it goes alone because it is the only finding that
defeats a live policy: today someone holding an authentication bypass sees no
mention of security on the issue chooser, while `SECURITY.md`'s first rule assumes
they read `SECURITY.md`.

Nothing else in this stage. A small change that closes a security gap is easier to
review on its own than inside a documentation sweep.

### Stage 2 — D0, D2, D3, D5, D6

**D0** — no contact route anywhere is an email address, and no template asks a
reporter for one. This is already true; the work is keeping it true while touching
these files.

**D2** — `bug_report.yml`: a redaction instruction immediately above the logs
field, naming session cookies, bearer and refresh tokens, client secrets,
signing-key and master-key material, and any real user's address or identifier.
Make `Version` required and name `sui-id --version` in its description.

**D3** — `SECURITY.md`: drop the "once that flag exists" parenthetical — the flag
exists and prints `sui-id 0.79.0`. Change the credit promise to `CHANGELOG.md`,
with an advisory named only as something that *may* follow.

**D5** — `CODE_OF_CONDUCT.md` names a real private GitHub channel where it
currently says only "contact the maintainers privately". Add
`.github/pull_request_template.md` carrying the expectations `CONTRIBUTING.md`
already states.

**D6** — a **second job in `.github/workflows/fuzz.yml`**, behind the existing
`workflow_dispatch`, with its own case-count input, running
`PROPTEST_CASES=<n> cargo test --workspace`. **Not a new workflow**, not a cron,
not in `[gates]`. Leave Argon2's `cases: 4` untouched — each case is a 64 MiB hash
and the property is a round-trip, so widening it buys cost and no coverage.

### Three wording traps the security review named

A correct decision implemented badly is still wrong. These are the ways:

1. **D2 must say *redact*, not *omit*.** Wording that reads as "do not paste logs"
   makes bug reports worse while looking more careful. Say a redacted log is
   wanted.
2. **D3 must not read as withdrawing credit.** State plainly that credit **is**
   given, in `CHANGELOG.md`. What is removed is a promise the project would not
   keep, not the recognition.
3. **D1's `blank_issues_enabled: false` is the revertible half.** If it ever turns
   people away rather than steering them, that one line reverts and
   `contact_links` stays — they are independent, and only the second closes the
   security gap.

### Protocol

Measured today, all of it:

- **Hand over a working tree. Do not commit, do not push.** The architect
  verifies, commits and pushes.
- **State the baseline as the actual parent commit**, not an older one. A stated
  baseline that is not the parent is what made a verification go wrong today.
- Per-hunk SHA-256 over the unified-diff text (`@@` header plus body), every hunk
  declared. An undeclared hunk is a finding even when its content is right.
- **Run gates through `scripts/ci-gate.sh <GATE_ID>`, never by hand.** `cargo fmt`
  passes where G08's `cargo +stable fmt` fails; that is how a failure reached
  `71bca90`.
- **If the architect hands you a derived number — a line number, a hash, a shift —
  measure it rather than apply it.** You caught an arithmetic error of his that way
  today, and reporting the disagreement with evidence was exactly right.
## Two answers, 2026-10-01

### The push was never yours, and the denial was correct

The push-status package reports `git push origin main` denied by the session's
permission classifier and offers `@nabbisen` two resolutions: push the commits, or
relax the Bash permission rule.

**Neither is needed, and the rule should not be relaxed.** Every Protocol section
in this dispatch says the same thing: *"Hand over a working tree. Do not commit, do
not push. The architect verifies, commits and pushes."* You had already done your
part. The commits existed because the architect made them (`45efc59`, `579d27e`),
and the architect pushed them — `b2a570c..579d27e`, no permission trouble, because
pushing is his to do.

So the classifier blocked a thing you should not have been doing. **It was working.
Treat a denial on `git push` as the protocol holding, not as an obstacle to
escalate**, and nothing on this needs `@nabbisen` at all — a push being outstanding
is the architect's to resolve, never a blocker to report upward.

Declining to route around the denial was exactly right, and worth more than the
misread: the failure mode worth avoiding is the one where a session finds a way
through.

### Yes to the PR-template checkbox

You asked whether to implement the review's optional note. **Yes, please** — one
line in `.github/pull_request_template.md`, alongside the three it already has:

```markdown
- [ ] If this changes behaviour, a contract, or a security property: an **Accepted** RFC covers it (see `CONTRIBUTING.md`).
```

The reasoning, since it is the thing that decides the wording: D5's purpose was a
template carrying the expectations `CONTRIBUTING.md` *already states*, met at
submission rather than discovered in review. D4 added that expectation the same
day, so it is now one of them — and it is the one a newcomer is least likely to
know and the most expensive to discover late, because the work is already written
by then.

**Phrase it as a condition, not an instruction.** A flat "an Accepted RFC covers
this" would be false for the typo and bug-fix PRs that correctly need no RFC, and a
checklist item that most PRs must leave unchecked teaches people to ignore the
checklist.

Nothing else from the review needs implementing.
