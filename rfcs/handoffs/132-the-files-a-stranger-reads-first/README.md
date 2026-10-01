# RFC 132 — handoff

**RFC.** [`../../accepted/132-the-files-a-stranger-reads-first.md`](../../accepted/132-the-files-a-stranger-reads-first.md)
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
