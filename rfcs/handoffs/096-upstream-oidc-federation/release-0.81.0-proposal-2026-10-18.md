# `0.81.0` — cycle D, proposed for authorization

> ## **AUTHORIZED** — `@nabbisen`, 2026-10-18: *"Authorized."*
> In the same breath, on who does what: *"Remember we can ask dev team to
> help the preps at least partially. Besides, to do the actual release and
> publish is the role of you and me."* So the preparation is dispatchable;
> the tag and the publish are not.
>
> ## Still open, and not answered by that authorization: **§B.**
> The cut was authorized on §A's recommendation. §B's three rulings — the
> wording of the two released-version `### Security` entries, the advisory
> question, and the rule change — were not addressed and are **not** treated
> as settled by it. They do not block the cut: the entries are already
> public, so promoting `[Unreleased]` to `[0.81.0]` discloses nothing new.
>
> ## Original framing, kept: **two things, not one.**
> **(1)** Authorize the cut. **(2)** Rule on a disclosure question that is
> already live and that I caused — §B. Read §B first; it is the part that
> should not wait on the release decision.

**Proposed.** 2026-10-18 JST, by the architect.
**Scope.** Cycle D, `0.81.0`, over `0.80.0` (2026-10-13).
**Gate.** Met — the cycle was gated on 096-B1 stage 12, which completed
2026-10-18 with 12b (`09c1e04`, Level B run `39123221966`, 27 jobs, 0 skipped).

## A — the recommendation, and its outcome

**Cut `0.81.0`.** The gate is met and the release has a real feature in it
rather than inert scaffolding, which is precisely why cycle D was not cut at
stage 10a on 2026-10-15. — **Authorized 2026-10-18.**

*This section originally read "but not before §B is settled, because cutting
publishes the notes." That was wrong and is struck: the notes are already
public, which is the whole of §B's problem. Promoting them to a versioned
section discloses nothing that `57b4ead` and `6cb866b` did not already
disclose, so §B gates nothing here — it is owed an answer on its own terms,
not as a precondition of the cut.*

**Sequencing.** RFC 107's dispatch is open and the implementation role keeps
one working tree; its package and the release preparation would both edit
`CHANGELOG.md`, so they cannot run concurrently. 107 lands first, then the
preparation is dispatched. If 107 comes back needing a fix round, `0.81.0`
is cut without it, exactly as §D says.

## B — the disclosure question, and it is mine

`.github/SECURITY.md` reserves to you *"a discussion of severity and timeline
before any public disclosure"*. For `0.80.0` I honoured that: the `### Security`
entry was drafted in `.git-exclude/`, you ruled on severity and wording, and
only then was it committed.

**For this cycle I did not, and the text is already public.** Of the five
`### Security` entries now in `CHANGELOG.md` on `main`:

| # | Entry | Severity | Present in a released version? |
|---|---|---|---|
| 1 | The federated callback's legacy mapping path still decided the outcome | **High** | **Yes — every released version with federation enabled** |
| 2 | The OIDC `state` CSRF check depended on a droppable sealed cookie | Low | No change in reachable outcome |
| 3 | A federated identity with no email claim could be provisioned | Low | No — the path was built this cycle |
| 4 | A restart could reset the five-attempt cap | Low | No — nothing could submit a proof |
| 5 | The provider-enabled and same-link checks were never re-checked | Low | **Yes — `0.80.0` included** |

**Two describe weaknesses that were live in shipped versions, one of them
High.** Entry 1 went public in `57b4ead` on 2026-10-17; entry 5 in `6cb866b`.
Neither passed your severity-and-timeline call first.

**The cause is a rule I wrote.** After the second changelog catch-up I made it
standing that *every stage dispatch from stage 12 onward carries its own
`CHANGELOG` entry*. That rule is right for `Added`, `Changed` and `Fixed`. For
`### Security` it routes around the one gate `SECURITY.md` reserves to you,
because a stage's entry is committed with the stage — and a commit to this
repository is publication.

**What is not wrong:** both weaknesses are fixed in `main` (entry 1 by stage
12a, entry 5 by stage 6d/12a's own path), and disclosing a fixed weakness
alongside the release that fixes it is ordinary practice. Nothing here is an
unfixed vulnerability sitting in a public repository. What you lost was the
decision, not the fix.

**Three things to rule on, and the third is the one that matters:**

1. **Severity and wording for entries 1 and 5, as they now stand.** I am not
   asking you to re-approve text you never saw as though the asking were a
   formality — if you want either reworded, it is reworded before the cut.
2. **A GitHub advisory for entry 1.** For `0.80.0` you said *"Unnecessary.
   Still not in production use."* The same reasoning applies unchanged, so my
   recommendation is **no advisory**. It is your call and I am not treating
   the precedent as having decided it.
3. **The rule.** My proposal: a stage's `### Security` entry is drafted in
   `.git-exclude/` and folded into `CHANGELOG.md` **at the cut**, after your
   call; `Added`/`Changed`/`Fixed` entries keep going in with the stage, which
   is what the standing rule was actually for. That keeps the catch-up problem
   solved without routing around you.

## C — what `0.81.0` contains

Fifteen entries over fifty-five commits. Eleven are feature commits, all
096-B1: stages 7, 8, 9, 10a, 10b, 11, 11b, 6d, 12a, 12c, 12b.

**The headline, and the only operator-visible feature:** a federated sign-in
can now complete its local second factor. Before this release the verifier and
its promotion path existed with no caller; a federated MFA sign-in wrote a row
and set three cookies that nothing read, while MFA completed through the
generic path. Stage 12 made the verified pipeline the one that acts.

**Two protections were carried across that switch** rather than silently
dropped — the account lock, and the generic pending row's own consumption.
Both omissions were in my dispatches, not in the returned work.

**One operator-facing break, already documented:** a federated sign-in that
stops at a second factor now emits `auth.federation.mfa_required`, not
`auth.login.password_ok_mfa_required`. An alert on the old name keeps working
for local sign-ins and stops firing for federated ones. The entry says so in
those words.

**One migration pair:** `0047` (the two continuation tables) and `0048` (at
most one active continuation per user, with a one-time cleanup that deletes
rows). The `0048` entry states the deletion.

I reconciled the notes before writing this (`2e0e63d`): one entry still said
the verifier had no caller and *"no sign-in can complete a federated second
factor — that is later work"*, which the same section's own 12b entry
contradicts. A per-stage entry can be true at its stage and false at the cut;
that reconciliation is now part of what cutting means.

## D — what `0.81.0` does not contain, said plainly

- **RFC 107** — Accepted 2026-10-18 and dispatched the same day. It joins the
  cycle if its package returns in time and **does not gate the cut**. If it is
  not back, `0.81.0` ships without it.
- **096-B2** (federation provider and link commands) — blocked on RFC 094
  M2b, which is unstarted with 43 commands unconverted. **096-B1 finishing is
  not M4-B closing**, and nothing here claims otherwise.
- **Stage 14**, live-integration evidence — needs an upstream and credentials
  only you have. It is a `:23` closure item for RFC 096, not a release gate.
- **No RFC closes with this release.** RFC 096 stays in `accepted/`; closing
  it needs its own `Closure reviewed on` and `Closure approved by` record.

## E — the mechanics, once authorized

Per `docs/src/contributing/release-process.md`:

1. Bump `[workspace.package] version` in the root `Cargo.toml` (`0.80.0` →
   `0.81.0`); the six crates inherit it.
2. Promote `## [Unreleased]` to `## [0.81.0] — <date>` with §B's outcome
   applied.
3. Full matrix on the bump commit — Level B, on the pushed commit.
4. Tag `0.81.0`, **no `v` prefix**.
5. Publish to crates.io in the documented order: `sui-id-shared`,
   `sui-id-i18n`, `sui-id-store`, `sui-id-core`, `sui-id-web`, `sui-id`.

**Step 5 is an outward-facing act and is not covered by my standing commit
and push authorization.** Authorizing the cut is not by itself authorizing the
publish; say whether you want both or only the tag.

## F — nothing else is waiting on you

The three remaining items in your queue are unchanged and block nothing: the
severity and timeline on the held CLI registration-token finding, the M3 FAQ
veto, and stage 14's upstream credentials.
