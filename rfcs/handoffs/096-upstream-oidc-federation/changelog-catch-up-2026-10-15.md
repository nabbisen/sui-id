# 096-B1 — the CHANGELOG catch-up, written while the stages are fresh

**Dispatched.** 2026-10-15 JST, by the architect.
**Why now and not at release time.** Cycle D is gated on stage 12, so the next
release is some way off. Four feature commits have landed since v0.80.0 and the
`Unreleased` section is **empty**. Reconstructing four stages at release time is
how "what it does not claim" gets written badly — and three of these four stages
claim very little, which is exactly the hard part to write honestly later.
**Baseline.** Read the tip with `git log -1` and name the full SHA.
**Scope.** `CHANGELOG.md` only. No code, no RFC, no other document.

## What landed, and what each one actually delivers

Measured, not inferred — this is the material, and the second column is the
part that matters:

| commit | stage | what an operator gets |
|---|---|---|
| `e9d5001` | 7 — F04 | **Real.** First federated provisioning happens in one transaction. |
| `d7d5523` | 8 — migration 0047 | Two tables. No behaviour. |
| `f5b4035` | 9 — F02 | A continuation row written on every federated MFA sign-in, three cookies set, and `auth.federation.mfa_required` emitted in place of `auth.login.password_ok_mfa_required` **on the federated path only**. Nothing reads the row or the cookies. |
| `6582bd7` | 10a — the verifier | **Zero callers.** |

Verify the second column yourself before writing it — `grep` for callers of
`federated_mfa::verify` in `crates/sui-id/src/`, and for who touches
`federation_mfa_pending` outside `sui-id-store`. If you find a reader I missed,
that is a finding and the entry changes.

## What the entries must say

**One `### Added` / `### Changed` grouping under `## [Unreleased]`**, in the
Keep-a-Changelog shape the file already uses.

**Stage 7 is the only one written as a benefit.** Say what is now atomic and
what that prevents.

**Stages 8, 9 and 10a are written as what they are: groundwork.** The house
style for this is already in the file — v0.80.0's own entry says "The work is
not finished — the legacy mapping path still decides the outcome and is removed
in a later release." Match that register. Do not dress inert machinery as a
feature, and do not hide it either: an operator who sees a new table filling up
deserves to find it in the CHANGELOG.

**The audit-event change is the one entry that must be unmissable**, because it
is the only one that can break something an operator already relies on:
`auth.federation.mfa_required` now replaces `auth.login.password_ok_mfa_required`
on the federated path. The local path is unchanged — `session.rs:240` still
emits the old event — and that distinction belongs in the entry, because
"the event was renamed" without it would be false. Anyone alerting on the old
name loses the federated signal and keeps the local one.

**Two more behaviour changes owed a note**, both from earlier stages and neither
yet in the file. Confirm each at source before writing it, and say in the
package what you confirmed:

- the absent-`email` denial under `provision_on_first_login`;
- `try_login_with_cascade` no longer attempting a directory bind for a
  federated username.

## What this must not do

**No version heading.** The entries go under `## [Unreleased]`. Cycle D's
version is assigned when it is cut, not now.

**No claim about readiness** beyond what the entries describe.

**Nothing outside `CHANGELOG.md`.**

## What to return

1. The entries, and **the measurement behind the second column above** — say
   what you grepped and what you found, including anything that contradicts the
   table.
2. Confirmation at source of the two owed notes.
3. **Per-hunk SHA-256** against the tip you named.
4. Gate evidence. Docs-only, so CI path-filters to Level A; run the matrix
   locally anyway. **G10b and G14** are the ones a CHANGELOG edit moves.
5. Anything you think is wrong with this dispatch — including the second column.
