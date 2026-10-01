# Review is scoped to who exists

**RFC.** [RFC 129](../../accepted/129-review-scoped-to-who-exists.md), **Proposed** — nothing dispatched.
**Author.** High-capability model, requirements-architect role.

## The twelve statements, from RFC 128's stage-0 audit

**Milestone exit gates** (`ROADMAP.md`): `prep` (federation module split, which
also blocks 096-A and all RFC 094 federation work), **M2b**, **M2c**, **M3**,
**M4-B**, **M6**, **M7**.

**Closure prerequisites**: RFC 094 **M2b**; RFC 095; RFC 096 **096-A**,
**096-B1**, **096-B2**.

M0, M1a, M1b, **M2a** and **M4-A** carry no such requirement — which is why the
programme has run this far. RFC 094's M2a likewise, which is why the stage in
progress is closeable.

## What the work is

For each of the twelve: replace the unsatisfiable actor with a real one, keeping
the evidence unchanged. **The evidence is not the problem and must not be
touched** — rollback evidence, crash-injection evidence across every transition
prefix, the hostile-provider corpus, live-integration evidence. Only the
acceptor changes.

Each amendment carries a one-line note saying what it used to require and that
it was unsatisfiable (D5). **Do not silently substitute a name.**

## What is not decided here

**Whether M6/M7 is the right single point for D2's external engagement.** The
architect proposes it because that is where a public readiness claim is first
made. `@nabbisen` may put it at 1.0, or at a specific RFC, or decline it
entirely. The handoff does not assume the answer, and D2's wording is written so
the point can move without the rest of the RFC changing.

## A lifecycle note

RFCs 094, 095 and 096 are **Accepted**, not Done, so amending their closure
prerequisites is an ordinary amendment rather than a Done-RFC lifecycle act. The
`ROADMAP.md` milestone table is not an RFC and needs no such treatment.
