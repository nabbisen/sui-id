# RFC 132 — The files a stranger reads first

**Status.** Proposed
**Security review.** Required
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
4. **The unimplemented proptest job claim goes**, or the job is created. The RFC
   does not choose: either is honest, and which one is a question about whether
   wide proptest runs are wanted, not about documentation.

### D5 — A reachable private channel, and a PR template

`CODE_OF_CONDUCT.md` names the route it currently only gestures at — a GitHub
private channel, per D0, never an address.

`.github/pull_request_template.md` is added, carrying the expectations
`CONTRIBUTING.md` already states, so they are met at submission rather than
discovered in review.

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
