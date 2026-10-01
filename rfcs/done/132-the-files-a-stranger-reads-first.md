# RFC 132 — The files a stranger reads first

**Status.** Implemented
**Closure reviewed on.** 2026-10-02
**Closure approved by.** `@nabbisen` (accountable owner), 2026-10-02: "Batch 1 is approved." The closure review was performed by **the architect, which wrote this RFC**, and is therefore **not** independent of it — `@nabbisen` is the approver, which is what RFC 000 requires when no independent role exists. The implementation role verified the review's measurements separately; that corroboration is recorded beside it and is not approval.
**Closure evidence.** [Closure review batch 1, 2026-10-02](../handoffs/110-rfc-header-governance-guard/closure-review-batch-1-2026-10-02.md), with [independent verification](../handoffs/110-rfc-header-governance-guard/closure-verification-batch-1-2026-10-02.md)
**Accepted on.** 2026-10-01
**Approved by.** `@nabbisen`, 2026-10-01: "Accepted."
**Security review.** Required
**Independent design review.** [Security review 2026-10-01](../handoffs/132-the-files-a-stranger-reads-first/security-review-2026-10-01.md) — **by the architect, who authored this RFC, and therefore not independent.** `@nabbisen` requested the audit this RFC acts on; he did not ask for a design review of the RFC itself and did not state that he performed one. Carried under `ROADMAP.md` R1's residual. **The field's name overstates the document**, as it does on RFCs 124, 128 and 130.
**Amended on.** 2026-10-01 — D6 added, resolving the proptest question D4(4) deliberately left open, on `@nabbisen` asking for the reasoning rather than a verdict.
**Design prerequisites.** None. Acts on `ROADMAP.md` S1d (disclosure, ruled 2026-10-01) and on the governance-files audit of the same day.
**Implementation prerequisites.** None.
**Closure prerequisites.** Someone who has found an authentication bypass is told where to report it **at the moment they choose to open an issue**, not only in a file they had no reason to open. No template invites a secret or a third party's personal data without saying not to. No file in `.github/` promises something the project does not do. No contact route anywhere is an email address. And `CONTRIBUTING.md` describes **this** project's process, including that a behaviour change needs an RFC and that a release includes publishing six crates.
**Tracks.** Project surface. Audit requested by `@nabbisen`, 2026-10-01.
**Touches.** `.github/ISSUE_TEMPLATE/config.yml`, `.github/ISSUE_TEMPLATE/bug_report.yml`, `.github/SECURITY.md`, `.github/CONTRIBUTING.md`, `.github/CODE_OF_CONDUCT.md`, and a new `.github/pull_request_template.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

## Summary

These six files are the only ones a stranger reads before anything else, and they
are the project's interface to people it has never met. Audited 2026-10-01: the
issue templates are sound, and **the prose files describe a different project**
than the one that exists. One gap actively defeats the security policy.

## The audit

**What is already right, and should not be disturbed.** `SECURITY.md` routes
reports to GitHub's private advisory form rather than to an address; its in-scope
list is specific and real (*"breaking the audit log's append-only property"* is
exactly what RFC 125 found); and its out-of-scope list correctly excludes
root-on-host and documented-dangerous configuration. The three issue templates
ask for the right fields in the right order.

**No privacy exposure exists today.** No email address appears in `.github/`,
`README.md` or `CONTRIBUTING.md`. That is the state to preserve, and D0 makes it a
rule rather than a coincidence.

**The defects, worst first.**

| # | Finding |
|---|---|
| 1 | `config.yml` is one line, `blank_issues_enabled: true`, with **no `contact_links`**. Someone who has found an authentication bypass opens "New issue" and sees three templates and a blank option, none mentioning security. `SECURITY.md`'s first rule — *"Do not file a public GitHub issue"* — depends on them having already read a file they had no reason to open |
| 2 | `bug_report.yml` asks for logs with `render: shell` and an environment block, with **no instruction to redact**. For an identity provider a paste can carry session cookies, bearer tokens, client secrets, the master-key path, and the email addresses of the reporter's *own users*. `Version` is also optional, though version decides whether a flaw applies |
| 3 | `SECURITY.md` says to report the version via *"`sui-id --version` once that flag exists; for now the git SHA"*. **The flag exists** — it prints `sui-id 0.79.0` |
| 4 | `SECURITY.md` promises a reporter *"credit in the changelog and security advisory"*. Under S1d the project issues no advisory by default, so half the promise is one it does not keep |
| 5 | `CONTRIBUTING.md` **never mentions RFCs** — zero occurrences. The project runs RFC 000's lifecycle, where design is owner-approved before implementation; the contributor-facing file describes "open an issue, then send a PR" |
| 6 | `CONTRIBUTING.md`'s Releases section says releases are *"`CHANGELOG.md` and a Git tag"* and **never mentions publishing**. That is precisely the omission that left four tags unpublished (0.76.10–12, 0.78.0) |
| 7 | `CONTRIBUTING.md` states `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` — none carrying `+stable` or `--locked`. A contributor following it passes locally and fails CI, which is how a G08 failure reached `71bca90` |
| 8 | `CONTRIBUTING.md` promises wide proptest coverage *"as a periodic CI job"*. **`PROPTEST_CASES` appears in no workflow** |
| 9 | `CODE_OF_CONDUCT.md` says to *"contact the maintainers privately"* and names **no channel**. Privacy-safe, and unactionable |
| 10 | **No PR template**, although `CONTRIBUTING.md` lists firm PR expectations (a failing-then-passing test, a `CHANGELOG` entry, a `docs/` update) |

## Decisions

### D0 — No contact route is ever an email address

`@nabbisen`, 2026-10-01: *"our privacy should be protected so any email address
etc. must not be exposed."* Every route these files offer is a GitHub mechanism —
the private advisory form, an issue, a discussion. This is already true and
becomes a rule so it stays true. It applies to a maintainer's address and to a
reporter's: no template asks for one.

### D1 — The issue chooser carries the security route

`config.yml` gains `contact_links` whose first entry sends a suspected
vulnerability to
`https://github.com/nabbisen/sui-id/security/advisories/new`, and
`blank_issues_enabled` becomes `false`.

The policy must not depend on the reporter having read the policy. A chooser is
where the decision is actually made.

**The trade, stated:** `false` means every issue passes a template, so someone
with a thought that fits none of the three is pushed into the closest one. For a
project whose worst-case issue is a public zero-day, steering everyone through a
surface that *can* carry a warning is worth that friction. The `question.yml`
template already absorbs the freeform case.

### D2 — A template that asks for logs says what not to paste

`bug_report.yml` gains a short, unmissable instruction immediately above the logs
field: redact session cookies, bearer and refresh tokens, client secrets,
signing-key and master-key material, and any address or identifier belonging to a
real user. It names them, because "redact sensitive data" leaves the reader to
guess and an identity provider's logs are full of things they may not recognise as
sensitive.

`Version` becomes required, and its description names `sui-id --version`.

### D3 — `SECURITY.md` promises only what the project does

Two corrections, both small and both the same fault:

1. The `--version` parenthetical goes; the flag exists.
2. The credit promise becomes credit **in `CHANGELOG.md`**, with an advisory named
   only as something that may follow if one is issued — per S1d, which rules that
   none is by default and records why.

A reporter must be able to rely on what this file says. A promise of an advisory
the project will not publish is worse than no promise.

### D4 — `CONTRIBUTING.md` describes this project

Four changes:

1. **The RFC lifecycle is named.** A typo or a bug fix is a PR. A change to
   behaviour, a contract or a security property goes through an RFC under
   `rfcs/`, owner-approved before implementation (RFC 000). Saying so costs a
   paragraph and saves a contributor from writing something that cannot merge —
   which is the stated purpose of the file's own "Before you start" section.
2. **Releases include publishing.** Six crates, bottom-up, and a cut that is not
   published is not a release (RFC 131 D7). The current wording is the omission
   that produced four unpublished tags.
3. **The verification commands stop being restated.** The file points at
   `scripts/ci-gate.sh <GATE_ID>` and `contracts/gate-inputs.toml` instead of
   paraphrasing G07, G08 and the test lanes. A restated command is a command that
   drifts; this is the same principle as RFC 116's one-source rule and RFC 131 D4,
   and the reason RFC 131 D4's scope is recorded there as too narrow.
4. **The proptest claim is resolved by D6**, not by deleting it.

### D5 — A reachable private channel, and a PR template

`CODE_OF_CONDUCT.md` names the route it currently only gestures at — a GitHub
private channel, per D0, never an address.

`.github/pull_request_template.md` is added, carrying the expectations
`CONTRIBUTING.md` already states, so they are met at submission rather than
discovered in review.

### D6 — Wide proptest runs are wanted, before a release, inside `fuzz.yml`

`@nabbisen`, 2026-10-01, asked the right question rather than for a verdict:
*"Whichever yes or no, the reasoning is important. What is the purpose ? When will
it be run ?"* Both answers are measurable, and were measured.

**The purpose is discovery that feeds the fast suite.** The narrow caps exist for
speed — seven properties at 256 or 512 cases, the CIDR matcher at 512, Argon2 at
4. A wide run reaches inputs the narrow run cannot. When it finds a
counterexample, proptest writes a regression file, that file is committed, and
**the counterexample becomes a permanent case in the fast suite forever**. So the
wide run is not a parallel test suite; it is a generator of narrow-suite cases.

**It has already worked here.**
`crates/sui-id-store/proptest-regressions/tests_state_machine/auth_codes.txt`
holds a shrunk counterexample — five issues, a purge, then consuming index 0 —
found by the auth-code state-machine property and replayed on every run since.
That is the mechanism paying for itself once already.

**The cost is small for the properties that matter.** Measured on the CIDR
property: about 0.0127 ms per case above a 111 ms fixed cost, so 512 → 65,536
cases costs that property under a second. The seven cheap properties can be
widened for seconds, not minutes.

**Argon2 stays at 4 cases, deliberately.** Each case is a 64 MiB hash, so widening
is expensive; and the property is a hash/verify round-trip, which is not
input-space-sensitive — a thousand more passwords explore nothing a handful
doesn't. Widening it would buy cost and no coverage.

**`PROPTEST_CASES` does work**, contrary to the architect's first suspicion that an
explicit `cases:` would override the environment. Verified by timing the same
property at three settings — 118 ms, 319 ms, 942 ms for the default, 16,384 and
65,536 — which fits a linear model through the 512-case baseline. So
`CONTRIBUTING.md`'s documented command is correct and stays; only the job it
promises is missing.

**When it runs: before a release, manually dispatched — and in `fuzz.yml`.**

- **Not per-push.** It is slow, and it is *nondeterministic by design*: a lane
  that can fail on a newly generated input fails for reasons unrelated to the
  change under test. That is the opposite of what `[gates]` is for, which is why
  this is a release-time step and not a gate — the same category as RFC 131 D7's
  publish check.
- **Not on a schedule.** `.github/workflows/fuzz.yml:3-5` records why: a weekly
  schedule *"failed eight consecutive weeks unnoticed — a cron in a solo repo has
  no subscriber for a red run. Run before a release instead."*
- **So: inside `fuzz.yml`**, as a second job behind the existing
  `workflow_dispatch` with its own case-count input. It is the same kind of thing
  as fuzzing — randomised, slow, nondeterministic, valuable, needing a human who
  is already looking — and that workflow already has the right trigger, the right
  input knob and the reasoning recorded. **No new workflow**, per the complexity
  caution.

`CONTRIBUTING.md`'s sentence then becomes true as written, and names the dispatch.

## What this is not

It is not a rewrite. `SECURITY.md`'s scope lists, the three templates' fields and
`CONTRIBUTING.md`'s code-style and property-test guidance are good and stay.
Eight of the ten findings are a sentence or a few lines each.

It also does not widen RFC 131 D4, which is recorded there as a scope change
awaiting `@nabbisen`. D4 here changes one document's text; whether a **gate**
enforces that across all three drifted documents is that RFC's question, not this
one's.

## Risk

**The one real risk is D1's `blank_issues_enabled: false`**, which is the only
decision here that takes something away from a stranger. If it proves to turn
people away rather than steer them, it is one line to revert, and the
`contact_links` half — which is the part that actually closes the security gap —
is independent of it and should stay either way.

Everything else is a correction of a false or absent statement, where the risk of
acting is lower than the risk of leaving it.
