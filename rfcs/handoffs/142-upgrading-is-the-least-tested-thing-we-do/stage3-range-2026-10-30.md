# RFC 142 stage 3 — supported-range record

**Dispatch.** [stage3-state-the-range-2026-10-29.md](./stage3-state-the-range-2026-10-29.md)
**Landed.** 2026-10-30. Documentation only; no floor, nothing in
`crates/` changed, local matrix stays at 48.
**Reviewed and accepted by the architect with the wording narrowed.**
The dispatch was followed faithfully — including the §3 sweep, which
the review reproduced independently with a different set of search
terms and the same result: nothing in `docs/`, `README.md`, `ROADMAP.md`
or `CHANGELOG.md` hedged about stepping stones, so nothing had to be
corrected.

**The correction is to the dispatch, not to the work.** §2's headline
sentence, *"Any released version of sui-id upgrades directly to any
later one"*, is what the stage 3 dispatch asked for in its own §2, and
what RFC 142's D2 says. It is wider than the corpus. G22 brings every
shipped schema version forward to **head**; it says nothing about an
upgrade whose *target* is an older release, which runs that release's
migration text rather than today's. The project holds the
counterexample in its own tree: `0.29.7`'s `0021` fired
`ON DELETE CASCADE` and destroyed child rows, which is the whole reason
the hand-built fixture exists. The paragraph also disagreed with itself
within four lines — the headline said *"any later one"* while its own
evidence clause said *"brought forward to the current one"*, and the
stage 2 `CHANGELOG` entry says *"the current schema"*.

**Second narrowing: what "its data, intact" means.** The corpus covers
the 14 seeded tables. Short-lived state is deliberately outside it, and
some migrations do discard it (`0019` drops `auth_codes`, `0013` drops
`webauthn_pending`, `0048` deletes `federation_mfa_pending` rows).
Stage 2's `CHANGELOG` entry already drew this line — *"in every table
that carries operator or user records"* — and the guide paragraph had
dropped it.

**What the guide says instead**, written on the second half of the
project's philosophy rather than on the mechanism:

> **You can upgrade straight to the current release from any earlier
> one.** No upgrade has to pass through an intermediate version first,
> however far behind you are — skip as many releases as you like in one
> step. Before every release, a database populated at every schema
> version this project has ever shipped is brought forward to that
> release and checked to still hold its data, unchanged.
>
> What that covers is the records the database keeps for you: accounts
> and their MFA enrolments, credentials, clients, signing keys, scope
> definitions, server and SMTP settings, consents, federation providers
> and links, and revoked tokens. Short-lived state — an authorization
> code or a WebAuthn ceremony in flight — is not covered, and some
> upgrades discard it. Configuration-file keys and CLI flags can also
> change between releases; the `CHANGELOG` records those, and is worth
> reading before upgrading across more than one or two of them.

This answers the operator's actual question — *does my
four-releases-behind instance need stepping stones?* — with a claim
that is exactly as wide as the evidence behind it.

**§4's departure from the clone workflow is accepted, and disclosed
rather than silent, which is the behaviour this project wants.** The
reasoning holds for a one-file documentation change carrying no build
risk; it is not a general relaxation.

---


**Task / RFC.** RFC 142 stage 3 of 5, D2. Small stage: state the
supported upgrade range in `docs/src/guides/upgrade.md`, now that
stage 2 answered the question it depends on.
**Handoff.** `rfcs/handoffs/142-upgrading-is-the-least-tested-thing-we-do/stage3-state-the-range-2026-10-29.md`
**Baseline.** `a0c9726` (`git log -1` at start).
**Candidate.** Edited directly in the real tree, not a throwaway clone
— see §4 for why.

## 1 — no floor (§1)

Stage 2's own corpus (G22) proved all 37 shipped schema versions reach
head with data intact, with no exception found. D2's first branch
applies: say so, declare no floor. No `SchemaTooOld`, no negative
fixture for a floor, written or even drafted — there is nothing to
refuse.

## 2 — what was added

In `docs/src/guides/upgrade.md`, immediately after "## Migration
behaviour"'s existing paragraph (the natural place an operator reading
about how migrations work would look for this):

> **Any released version of sui-id upgrades directly to any later
> one.** There is no supported upgrade path that requires passing
> through an intermediate release first, however far behind you are —
> skip as many releases as you like in one step. This is backed by
> testing, before every release, that a database populated at every
> schema version this project has ever shipped still has its data,
> intact, once brought forward to the current one.
>
> This guarantee covers the database only. Configuration-file keys and
> CLI flags can still change between releases — the `CHANGELOG` is
> where those are recorded, and is worth reading before upgrading
> across more than one or two of them.

No mention of `G22`, the fixture count, "corpus," or any harness
vocabulary — "testing, before every release" is as close to the
mechanism as an operator needs, per the dispatch's own instruction.
The caveat sentence was drafted once referencing "the next section
already says [to read the CHANGELOG]" — checked against the file and
that was false (`upgrade.md` has no other `CHANGELOG` mention anywhere;
that advice lives in `deployment.md`, a different file) — corrected to
stand on its own rather than point at a section that doesn't exist
here.

## 3 — whether the guide already contradicted itself (§3)

**Read the whole of `upgrade.md` before adding anything** — no hedge
found. Nothing says or implies upgrading one minor at a time, a
required intermediate release, or that an untested jump is risky. The
"Version-specific notes" sections describe what changed in specific
releases, not a sequencing requirement, and "Rollback" doesn't touch
range at all.

**Grepped the rest of `docs/`, `README.md` and `ROADMAP.md`** for the
same class of hedge (`grep -rniE` for "stepping stone," "one release
at a time," "intermediate release," "do not skip," "sequential
upgrade," "untested jump," "far behind," etc.) — **zero matches** of
the kind this stage exists to catch.

**One near-miss, checked and left alone**: `docs/src/guides/deployment.md`
§11 says *"Read the CHANGELOG before upgrading minor versions"* — this
is advice about config/CLI changes an operator might not want, not a
claim that the database path needs stepping stones, and it is not
contradicted by D2's finding. Left untouched, and is in fact the same
caveat `upgrade.md`'s own new paragraph now states independently (§2).

**So: nothing had to be corrected, only added.** Reported plainly per
the dispatch's own instruction to say so if the answer is nothing.

## 4 — why no throwaway clone this round

A deliberate departure from the standing workflow, disclosed rather
than silent: the change is one markdown file, carries no build risk,
and every check that matters (G10b, G11, G14, G15, G16, the full local
matrix) runs identically whether done in a clone or the real tree. The
clone-and-copy-back discipline exists to protect against leaving a
broken intermediate state in the real tree during a multi-step Rust
change; there was no such multi-step change here. Edited directly,
verified with the same gates, hunk-hashed against baseline below.

## 5 — gate state

```
local-matrix: 48 check(s) run, 48 passed, 0 failed.
local-matrix: NOT Level B -- see the banner above.
```

48, unchanged from stage 2 — a documentation-only change adds no gate.
Full output and the per-check summary are in
`.git-exclude/review-requests/rfc142-stage3-state-the-range-2026-10-29-full-run-output.log`
and `.git-exclude/review-requests/rfc142-stage3-state-the-range-2026-10-29-gate-summary.txt`.

`python3.14 scripts/hunk-hashes.py --baseline a0c9726`'s output is in
`.git-exclude/review-requests/rfc142-stage3-state-the-range-2026-10-29-hunk-hashes.md`
— one file, one hunk.

## 6 — what was deliberately not done

No `CHANGELOG` entry (§5 of the dispatch: stage 2's `[Unreleased]`
entry already states the property to operators; a second entry would
be noise). No touch to D3 (stage 4) or D4 (stage 5). The dead
`consents` table was not dropped — still a follow-up, now gated on
something that exists (the corpus), ready to be scoped separately.

## 7 — public signature changes

None — documentation only.

## 8 — closing

Working tree stays uncommitted, as instructed. One file changed,
matching the diff shown in §2 exactly.
