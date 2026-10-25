# M3 closure — the architect's assessment of the evidence

> ## Whose turn it is: **yours**, for one narrower question than the one I asked, and then for acceptance.
> **`@nabbisen`, 2026-10-25**, asked whether any instance of his ever ran
> `0.76.3`–`0.79.0`: *"No."*

RFC 095 `:17` makes the assessment of the closure evidence the architect's
and the acceptance his. This is the assessment. It is adversarial by
instruction, so most of it is spent trying to break the claim rather than
restating it.

**Measured 2026-10-25** against `bac8fb0`.

## A — what the statement establishes, and what it does not

**Established.** No deployment ran a *released* build in the exposed
range. That range was measured in
[`m3-closure-route-2026-10-25.md`](m3-closure-route-2026-10-25.md) §C:
`0.76.3` through `0.79.0`, from `git tag --contains` on the two window
commits.

**Not established, and this is my error in how I framed the question.**
The ambiguity window is defined by **commits** — `48946f6` (2026-07-02)
through `930513b` (2026-10-02) — not by tags. A build from source inside
that window is exposed whether or not it corresponds to a release. I
asked about released versions, and for this project that is close to the
least likely way an in-window build got run: the window is three months
of active development on this very feature, so in-window builds from
source almost certainly *were* run here. The answer "No" is true and
does not reach them.

So the version question was the wrong one. Below is the one that decides
it.

## B — the precondition that actually bounds this

A client cannot self-register unless the request carries a valid
`Authorization: Bearer <token>` matched against `client_registration_token`.
Verified at **both ends of the window**, not inferred from today's code:

- At `48946f6` (window start),
  `crates/sui-id/src/handlers/dynamic_register.rs:119` — *"Authorization:
  Bearer <token> is required for dynamic client registration"* — and
  `:125` consumes the token.
- At `930513b` (window end),
  `crates/sui-id/src/http/handlers/dynamic_register.rs:117` extracts the
  bearer token before anything else.
- Today the handler's own header records the knob's state:
  *"Open registration (no token required) is a future knob defaulting
  off."*

I did **not** re-verify each of the seven intermediate commits that
touched the handler inside the window, so I cannot exclude a transient
removal. I am recording that rather than rounding it away; it matters
only in combination with the question below, and only if the answer to
that is yes.

**What this buys.** A `legacy_unstamped_dynamic_candidate` can only exist
on a database where an operator first **minted a client registration
token** and something then used it. That is a deliberate act, not a
default, and — unlike the audit log — it is a precondition rather than a
record, so its absence is not subject to the incompleteness that makes a
"zero candidates" claim inadmissible. **That is the whole reason this
route can close M3 at all**: the RFC refuses inference from missing
evidence, and this is not inference from missing evidence.

## C — the one question left

**Was a `client_registration_token` ever minted on any database of yours
that still exists — including a local development database — while
running a build from inside the window?**

If **no**: the disposition set is empty by construction, on a precondition
rather than on a silent report, and the assessment below stands as
written.

If **yes**, for any database still in existence: run, on that machine,
against that database —

```
sui-id admin legacy-dynamic-candidates --config <that instance's config>
```

— read-only, no elevated privilege, and every candidate it lists then
takes one of RFC 095's three dispositions. I will dispatch the disposal
rehearsal at that point, not before.

A database that no longer exists needs no disposition; there is nothing
to adjudicate and nothing to carry forward. Say so and I will record it
that way.

## D — my assessment

**Conditional on §C being "no": the closure evidence is sound, and I
assess it as sufficient.** The reasoning does not depend on the audit
log's completeness, which is the specific defect RFC 095's prohibition
exists to guard against. It depends on two measured facts — the exposed
window's boundaries, and the registration-token precondition holding at
both of them — plus one statement of deployment history that only you can
give.

**What I am not claiming.** Not that no candidate exists anywhere: third
party operators who ran `0.76.3`–`0.79.0` with a registration token minted
may hold candidates, and `docs/src/guides/upgrade.md:31-49` is what this
project owes them. Not that the report would have found them. The claim
is bounded to deployments you control, which is what RFC 095's field asks
for.

## E — what closes on acceptance

Mine to execute, once §C is answered and the evidence accepted:

1. `rfcs/accepted/095-dynamic-client-registration-transaction.md` → status
   **Implemented**, with the closure evidence cited by path.
2. `ROADMAP.md`'s M3 row and the cycle E row, which both currently say M3
   does not close with `0.83.0`.
3. A `CHANGELOG` entry under `[Unreleased]`, shipping with `0.85.0`,
   stating what closed and on what basis — including that no
   zero-candidates claim was made, since the public record should not
   read as though one was.
4. `rfcs/handoffs/095-dynamic-client-registration/README.md`.

Nothing in that list is started.
