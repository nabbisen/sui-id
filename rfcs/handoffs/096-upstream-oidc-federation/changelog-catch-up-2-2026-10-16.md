# 096-B1 — the second CHANGELOG catch-up, and why there should not be a third

> ## Whose turn it is: **the implementation role's.**
> Nothing waits on `@nabbisen` or the architect. This is small and goes out
> ahead of stage 12 so stage 12 is not delayed behind it.

**Dispatched.** 2026-10-16 JST, by the architect.
**Scope.** `CHANGELOG.md` only. No code, no RFC, no other document.
**Baseline.** Read the tip with `git log -1` and name the full SHA.

## Why there is a second one

The first catch-up (2026-10-15, landed `b33f5d5`) cleared stages 7 through
10a. Four stages have landed since with no entries: **10b, 11, 11b and 6d**.

That is the same backlog rebuilding two days later, and the cause is mine: I
treated the catch-up as a one-off instead of asking each stage's own package
for its entry. Every dispatch asks for hashes, gates and counts and none asks
for the changelog, so the gap reopens on every stage. **From stage 12 onward
each dispatch carries its own entry in its own *What to return* list**, which
is the half of this fix that stops a third catch-up. This dispatch is the
other half.

## What landed, and what each one is

Verify the second column yourself before writing from it — the first catch-up
found my own table wrong in one place, and that is the habit worth keeping.

| commit | stage | what it is |
|---|---|---|
| `f3e6925` | 10b — F03 | The four guarded outcomes. **Reachable by nobody**: still no caller. |
| `749571b` | 11 — F06 and the WebAuthn branch | The third method, end to end. Also no caller. |
| `63e0e09` | 11b | A restart no longer grants a fresh five-attempt budget. **Migration 0048**, which dedups and then constrains. |
| `a6524e0` | 6d — F01 into L04 | Attempt completion, provider and link authority now commit with the session. **Fixes a shipped bug.** |

## What the entries must say

**Keep the register the file already uses.** v0.80.0's own entry says "The
work is not finished — the legacy mapping path still decides the outcome and
is removed in a later release." Groundwork is groundwork; say so.

**10b and 11 are still groundwork.** They complete the federated MFA machinery
and nothing can reach it. The existing `Unreleased` groundwork bullet already
says a verifier exists with no caller — extend it rather than adding a second
bullet that says the same thing about more code.

**11b is a security entry.** `### Security`, with severity argued from what
exploiting it would take, the way the absent-email entry already does. The
honest framing: the five-attempt cap on a federated second factor could be
reset by restarting the upstream sign-in, **and the path has no caller**, so no
released version was exploitable — say both halves. It also carries migration
`0048`, which **deletes rows** on upgrade (duplicate active continuations,
keeping the most restrictive). An operator deserves to read that before it
happens.

**6d is the one with a user-visible fix in it.** Two things, and they are
different: federated sign-ins through an existing link with no second factor
never marked their login attempt complete, so those rows accumulated until the
24-hour purge; and the provider-enabled and link-exists checks now happen in
the same transaction as the session rather than minutes earlier. Say plainly
that the first was bookkeeping and not an authentication weakness —
one-time-ness has been enforced by the attempt's own claim since stage 6c, and
an operator who reads "attempts were never completed" will otherwise assume
the worse thing.

## What this must not do

**No version heading.** Everything goes under `## [Unreleased]`. Cycle D's
version is assigned when it is cut, which is gated on stage 12.

**No new claim about readiness**, and nothing outside `CHANGELOG.md`.

## What to return

1. The entries, and **what you measured** for the second column above —
   including anything that contradicts it.
2. **Per-hunk SHA-256** against the tip you named, regenerated after your last
   commit.
3. Gate evidence. Docs-only, so CI path-filters to Level A; run the matrix
   locally anyway. **G10b and G14** are the ones a CHANGELOG edit moves.
4. Anything wrong with this dispatch, including the framing of any of the four.
