# `0.81.0` — the release preparation

> ## **DISPATCHED 2026-10-19.** Whose turn it is: **the implementation role's.**
> The hold is lifted. RFC 107's fix round was accepted and committed
> (`36c0a74`), so the working tree is free and `CHANGELOG.md` has no competing
> edit. This is the only open dispatch.
>
> **It does not produce a release.** The division recorded in the proposal's
> header block: the preparation is dispatchable, *the tag and the publish are
> not*. This dispatch stops before both.
>
> **`0.81.0` therefore includes RFC 107.** The proposal's §D listed it as
> joining the cycle only if its package returned in time. It did. §C's "fifteen
> entries" count is correspondingly out of date — three `### Fixed` entries were
> added by 107 — and **that is for the architect to reconcile, not you**; §3
> below tells you what to do if you spot others.

**Written and dispatched.** 2026-10-19 JST, by the architect.
**Authorization.** The cut is authorized; the proposal's header block carries
the wording and the date.
**Proposal.** [`release-0.81.0-proposal-2026-10-18.md`](release-0.81.0-proposal-2026-10-18.md)
— read §B, §C, §D and §G before starting. §G gates the tag, not this work.
**Baseline.** The tip at dispatch time. Read it with `git log -1` and confirm;
do not assume a hash written here.

## 0 — two things you must not do

**Do not touch any `### Security` entry in `CHANGELOG.md`** — not the wording,
not the severity, not the ordering. Five are in `[Unreleased]` and two of them
describe weaknesses that were live in shipped versions. Their text is owed a
ruling that has not been given — proposal §B. Promote the
section heading above them; leave every character below it alone.

**Do not write the `CHANGELOG` record for `0.80.0`.** It is settled in
direction — proposal §G, ruled 2026-10-19: publishing follows every tag, and
`0.80.0` is the one case with a good reason not to, because it carries the
**High**-severity flaw stage 12a fixed four days after its tag. It is recorded
as **superseded, not abandoned**, and the architect writes that record. §7
below asks you for the *mechanism* that lets it be written; the wording is not
yours. If
you find yourself wanting to add one, you have found §G and not a gap.

## 1 — measured at `ecf02aa`, so you start from fact

I measured each of these for this dispatch. Re-measure anything you rely on.

**The version appears in exactly three places that are edited by hand:**

| Where | What |
|---|---|
| `Cargo.toml:20` | `[workspace.package] version = "0.80.0"` |
| `Cargo.toml:33-37` | the five `[workspace.dependencies]` internal specs |
| `Cargo.lock` | six `version = "0.80.0"` entries — **refreshed by cargo, never edited by hand** |

**No test pins the release version.** `grep -rn "0\.80\.0" crates/ --include="*.rs"`
returns nothing. The `crates/sui-id/tests/e2e/r120_routes.rs` line that moved at
the `0.80.0` cut was the security-surfaces table a test mirrors, not a version
assertion. Do not go looking for one.

**`CHANGELOG.md` has no link-reference section.** No `[Unreleased]: https://…`
compare links to repoint; the file ends with the older-releases table. The
promotion is the heading and nothing else.

**`docs/src/contributing/release-process.md:152-171` requires the five internal
specs to be pinned to the current release.** `version = "0"` cost a real
debugging session at `0.77.0` — `sui-id-store` resolved back to v0.2.0 and the
native `sqlite3` link conflicted. Bump all five; do not loosen any.

## 2 — the bump

1. `Cargo.toml:20` → `0.81.0`.
2. `Cargo.toml:33-37`, all five → `version = "0.81.0"`.
3. Refresh `Cargo.lock` by running cargo (`cargo check --workspace` is enough),
   not by editing it. Confirm all six entries moved.

## 3 — `CHANGELOG.md`

Promote `## [Unreleased]` to `## [0.81.0] — 2026-MM-DD`, dated the day you do
it, and leave the body as it stands subject to §0. **Do not open a fresh empty
`[Unreleased]`** unless that is this file's existing convention — check how
`0.80.0` and `0.79.0` were promoted and follow whichever you find, saying which.

Proposal §C reconciled the body already (`2e0e63d`): one entry claimed the
verifier had no caller, which the same section's own 12b entry contradicts. If
you find another entry that was true at its stage and is false at the cut,
**that is a finding, not a licence to edit** — report it and leave it, because
the architect cannot tell your correction from a `### Security` edit otherwise.

## 4 — the release rule, which is not optional

`.git-exclude/rules/project-instructions-general-common.md:27-31` requires two
things of every release, and the `0.80.0` cut did both visibly:

**(a) Audit for security flaws; update the threat model if the changes involve
new data flows, external integrations or auth logic, otherwise verify existing
controls remain valid.** `0.81.0` involves all three. `docs/threat-model.md` is
RFC 097's artifact and is blocked on RFC 096-B completing, so — exactly as the
`0.80.0` release commit recorded — **verify the controls that exist rather than
updating a document that does not.** Return that verification as prose: which
controls you checked and against what. Do not rewrite `docs/threat-model.md`.

**(b) Verify all docs match the changed requirements, design and specs.** The
`0.80.0` cut moved three documents under this rule. Do the same pass over
`0.81.0`'s changed behaviour — the federated second factor completing through
the verifier, the `auth.federation.mfa_required` event replacing
`auth.login.password_ok_mfa_required` on the federated path, and migrations
`0047`/`0048`. **Name every document you checked, including the ones you found
correct.** A pass that lists only what moved is indistinguishable from a pass
that stopped early.

**Two staleness banners name a workspace version that the bump makes wrong:**

- `docs/development-specification.md:8-21` — *"Stale as of v0.80.0
  (2026-10-13)… the workspace is three minor versions past it"*. After the
  bump it is four, and `contracts/doc-authority.toml`'s tolerance is 2.
- `docs/threat-model.md:3-4` — *"accurate as of **v0.26.0**; the workspace is
  now **v0.77.0**"*. Already three releases stale; no gate catches it.

**G15 passes either way** — I checked: check (C) exempts any document carrying
the banner regex, however far behind it is, and both carry it. So this is not
a gate failure, it is prose that misleads a reader about how stale a document
is, which is the half of this project's design philosophy about people not
being misled. **Re-state the workspace figure in both banners and nothing
else.** Do not re-date the syntheses and do not touch the known-divergence
lists; re-synthesis is scheduled work and is not this.

## 5 — the pre-publish checklist, with evidence

`docs/src/contributing/release-process.md:99-126`. Run these and **return the
output of each**, not a claim that it passed:

1. **`python3.14 scripts/check-published-versions.py --root .`** — RFC 131 D7's
   required release step. **Return its stdout and its exit code.** It exits
   **1** today, on `0.80.0`, and will keep exiting 1 until the architect lands
   the superseded record (proposal §G). **After your 1b change below it must
   still exit 1 for the right reason** — the record does not exist yet — so do
   not treat making it exit 0 as the goal. Return it anyway — the evidence is the
   point, and D7 exists because nobody was looking. *In zsh — which is this machine's shell,
   whatever the tooling reports — `$status` is `$?`, so after a pipeline it is
   the last command's. Capture the exit code without a pipe, or you will
   report `0` for a check that returned 1.*
**1b. Teach the detector a third outcome: *superseded*.** This is the only code
change in the round, and it exists because the vocabulary shaped a wrong
recommendation — the architect's, recorded in proposal §G.

`scripts/check-published-versions.py:70-77` recognises exactly two states: a
version is published, or `CHANGELOG.md` records it with the literal word
*abandoned*, in one of two narrow shapes. Reality has a third: **a tag
deliberately not published because a later release supersedes it for cause**,
which is `0.80.0`'s case and is not the same thing as abandoning a release.

- Accept *superseded* alongside *abandoned* in **both** patterns
  (`SECTION_ABANDONED_RE`, `LIST_ABANDONED_RE`).
- **Keep both patterns as narrow as they are.** Their docstring records why:
  a loose "the paragraph mentions the word somewhere" scan over-matched
  `0.79.0`, which was merely *named* in `0.78.0`'s note as where the content
  shipped. A widening that reintroduces that is worse than no widening.
- Rename what the widening makes inaccurate — `abandoned_versions`, the two
  constants, and the message at `:152-155` — to whatever states the real
  property. **Say what you chose and why**; the architect has no preference
  beyond that the name must not keep claiming "abandoned".
- `scripts/tests/` must cover **both** words in **both** shapes, and must
  still reject the `0.79.0` over-match. Mutate each new branch away and
  confirm the test fails; a test over an alternation is exactly the shape that
  passes while testing nothing.

**This does not change RFC 131 D7**, whose text describes two outcomes. Amending
it is a separate proposal and is not yours.

2. **`cargo package -p sui-id-shared --allow-dirty`** — must produce a package
   and the verify build must succeed. **Only this one is verifiable.** The
   other five resolve their internal dependencies from the index and cannot be
   dry-run-packaged until the ones below them are published; `sui-id-i18n`
   happens to work too, having no internal dependencies. **If you run all six,
   four will fail on `sui-id-shared = "^0.81.0"` not existing. That is the
   index reporting proposal §G's fact, not a defect** — do not report it as
   one, and do not try to fix it.
3. The working tree is clean for the real publish — **not your step.** You
   leave the tree uncommitted as usual; the architect commits.

## 6 — gates

This touches `crates/` (the bump) **and `scripts/` plus `scripts/tests/`**
(item 1b), so both scopes are in play. **Run the full local gate set** —
including A3.4 and the gate-matrix fixtures, which a loop over `[gates]` alone
misses — and report it.

**Level B is not yours to obtain here and I am not asking for it.** RFC 131 D2
requires Level B on *the exact commit being tagged*, and that commit does not
exist until the architect makes it. The architect dispatches the CI workflow on
the pushed release commit and confirms 27 jobs, 0 skipped, all 24
`[gate_owners]` entries matched both ways. Report your local result and say
plainly that it is local.

*`scripts/ci-gate.sh <G>` refuses a dirty tree, so sequence your runs against
that; it is a known friction, not a problem to solve in this round.*

## 7 — what to return

The usual package, plus:

- **Which `[Unreleased]` convention you followed** at the promotion, and which
  prior cut you took it from.
- **The release-rule output from §4** — the control verification as prose, and
  the full list of documents checked with their verdicts.
- **The §5 evidence**, verbatim, including the exit code that is 1.
- **For 1b: the name you chose** for what was `abandoned_versions`, and why;
  and the `scripts/tests` count before and after, since the baseline is 349
  plus 29 subtests and your new cases move it.
- **Anything in `CHANGELOG.md` that is true at its stage and false at the cut**
  — reported, not edited.
- `python3.14 scripts/hunk-hashes.py --baseline <sha>` against the **last**
  commit of the round. If the architect lands anything between the dispatch and
  your return, baseline against that commit — the 12b fix round hit exactly
  this.

**No `CHANGELOG` entry for this round, including for 1b.** The standing
per-stage rule does not apply twice over: this round *is* the changelog's
promotion, so an entry would be an entry about itself; and 1b changes a
maintainer-only release detector with no user-visible behaviour. If you think
1b is user-visible, say so rather than adding one.

Leave the working tree uncommitted. One open dispatch at a time.

## 8 — what this does not do

It does not tag, publish, or resolve `0.80.0`. After it returns and is
reviewed, the architect commits `release: 0.81.0`, pushes, obtains Level B on
that commit, and then — **and only once §G and §B's wording are
settled** — tags `0.81.0`, no `v` prefix. The publish is a separate
outward-facing act, outside the architect's standing authorization, and is
asked for separately.
