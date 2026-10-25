# M3 closure — the architect's assessment of the evidence

> ## Whose turn it is: **yours**, for acceptance. Nothing else is outstanding.
> **`@nabbisen`, 2026-10-25**, asked whether any instance of his ever ran
> `0.76.3`–`0.79.0`: *"No."* Asked whether a `client_registration_token`
> was ever minted on any surviving database of his, including a local
> development one, while running an in-window build: *"No."*
> **Both answers are the closure evidence this file records.** The
> residual I left open in §B is now measured and closed; the assessment in
> §D is unconditional.

RFC 095 `:17` makes the assessment of the closure evidence the architect's
and the acceptance his. This is the assessment. It is adversarial by
instruction, so most of it is spent trying to break the claim rather than
restating it.

**Measured 2026-10-25** against `7a36998`.

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

**Corrected 2026-10-25, and the correction matters.** An earlier revision
of this file said the seven intermediate commits mattered *"only if the
answer to §C is yes"*. That is backwards. The registration-token argument
is what the second answer rests on, so a transient removal of the token
gate would defeat it **precisely when no token was ever minted** — a
registration could then have succeeded without one. The residual had to be
closed, not deferred. It is:

Every commit touching the handler between the window's endpoints, checked
individually rather than inferred — `930513b`, `71bca90`, `7813b35`,
`ff19f05`, `dfd2af0`, `be30fbf`, `901e651`, plus the two endpoints
themselves. **All nine retain the gate.** Read in full at `dfd2af0`, the
longest-lived state: `extract_bearer_token` runs first, a missing token
returns `401` and an invalid one `400`, both **before any body validation
and before any client row is built**.

Two further things that would break the chain, and do not:

- **No "open registration" knob has ever existed.** `git log -S
  "open_registration" --all` returns nothing across the whole history, so
  the handler comment's *"a future knob defaulting off"* describes
  something unbuilt rather than something off by default.
- **No second writer.** At the window's end, the only non-test
  construction of `RegistrationSource::Dynamic` is the handler itself
  (`dynamic_register.rs:199`). There is no other production path to a
  dynamically registered client.

**What this buys.** A `legacy_unstamped_dynamic_candidate` can only exist
on a database where an operator first **minted a client registration
token** and something then used it. That is a deliberate act, not a
default, and — unlike the audit log — it is a precondition rather than a
record, so its absence is not subject to the incompleteness that makes a
"zero candidates" claim inadmissible. **That is the whole reason this
route can close M3 at all**: the RFC refuses inference from missing
evidence, and this is not inference from missing evidence.

## C — the chain, stated as one argument

Answered 2026-10-25: no `client_registration_token` was ever minted on any
surviving database, local development ones included. The argument that
closes M3 is therefore:

1. A `legacy_unstamped_dynamic_candidate` exists only where a **dynamic
   registration succeeded**.
2. The only production path to one is `POST /oauth2/register` through
   `dynamic_register` — no second writer of
   `RegistrationSource::Dynamic`, §B.
3. That handler required a valid `client_registration_token` at **every
   commit in the window**, returning early before any client row is
   built — §B, nine commits checked individually.
4. No such token was ever minted on a surviving database.
5. **Therefore no surviving database can hold a candidate.**

Every step is a property of the code or of deployment history. **No step
is an inference from the absence of an audit row**, which is the one move
RFC 095's closure field refuses.

## D — my assessment

**The closure evidence is sound, and I assess it as sufficient.** The
reasoning nowhere depends on the audit log's completeness, which is the
specific defect RFC 095's prohibition exists to guard against. It rests
on three measured facts — the exposed window's boundaries, the
registration-token gate holding at every one of the nine commits inside
it, and the absence of any second writer or open-registration knob — plus
two statements of deployment history that only you can give, both now
given.

**What would overturn it**, recorded so this can be re-tested rather than
re-argued: a surviving database of yours on which a registration token
was minted; a production path to `RegistrationSource::Dynamic` that is
not `dynamic_register`; or a build run here from a branch that never
reached `main`, since every commit I checked is reachable from it.

**What I am not claiming.** Not that no candidate exists anywhere: third
party operators who ran `0.76.3`–`0.79.0` with a registration token minted
may hold candidates, and `docs/src/guides/upgrade.md:31-49` is what this
project owes them. Not that the report would have found them. The claim
is bounded to deployments you control, which is what RFC 095's field asks
for.

## E — what closes on acceptance

Mine to execute, once the evidence is accepted. Nothing in this list is
started, and none of it should be until then:

1. `rfcs/accepted/095-dynamic-client-registration-transaction.md` → status
   **Implemented**, with the closure evidence cited by path.
2. `ROADMAP.md`'s M3 row and the cycle E row, which both currently say M3
   does not close with `0.83.0`.
3. A `CHANGELOG` entry under `[Unreleased]`, shipping with `0.85.0`,
   stating what closed and on what basis — including that no
   zero-candidates claim was made, since the public record should not
   read as though one was.
4. `rfcs/handoffs/095-dynamic-client-registration/README.md`.
