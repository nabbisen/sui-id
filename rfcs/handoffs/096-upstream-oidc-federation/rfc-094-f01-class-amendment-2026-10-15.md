# RFC 094 — the F01 class amendment, drafted for approval

> ## Whose turn it is right now: **`@nabbisen`'s.**
> The text below is ready. What is needed is approval of the sentence and of
> the round-trip it requires. The architect executes both on a yes; nothing
> else in 096-B1 moves until then.

**Written.** 2026-10-15 JST, by the architect.
**Why now.** The stage plan puts this "in the gap between stage 11 landing and
stage 12 being dispatched — a deliberate pause with nothing in flight". Stage
11 and 11b have landed; nothing is dispatched. This is that gap.

## What is being corrected

RFC 094 `:548` says:

> - F01 is the Protocol-class existing-link/no-MFA promotion transaction;

**The design says otherwise, and you approved the design on 2026-10-11**
(*"Your recommendation is accepted."*): F01 goes **Class A, folded into
RFC 102's L04**, so the attempt completion is atomic with the session it
authorises and `auth.federation.signin.success` is not downgraded from an
audit that is already atomic.

So the RFC and the agreed design disagree, and stage 6d is the stage that
would build against the disagreement. That is why the gap exists.

## The drafted replacement for `:548`

> - F01 is **Class A, folded into RFC 102's L04** rather than a separate
>   Protocol-class transaction: the existing-link/no-MFA promotion's guarded
>   `exchanging → completed` transition and its enabled-provider and
>   same-active-link/user authority checks run *inside* L04's own
>   `db.class_a` transaction, so the attempt completion commits atomically
>   with the session it authorises and `auth.federation.signin.success` stays
>   the Class-A event it already is. *(Amended 2026-10-15; the original text
>   classified F01 as Protocol, which would have separated the sign-in audit
>   from the session it records. Approved by `@nabbisen` 2026-10-11 as a
>   design; this records it.)*

## What approving it costs, measured

**It is material.** F01's class decides whether the sign-in audit record
commits atomically with the session it authorises. That is a security
invariant, so RFC 000 `:43-47` returns RFC 094 to `proposed/` atomically with
the change, and it is re-accepted from there — the same round-trip RFC 096 went
through yesterday.

**The window blocks two things and no more**, which is the measurement the
2026-10-12 review established and you approved:

- **RFC 094 M2b**, which is unstarted (43 commands unconverted);
- **RFC 096's own *Design prerequisites* line**, which reads "amended RFC 094
  Accepted" and is false while 094 sits in `proposed/` — so no 096-B1 stage is
  eligible during the window.

Both costs are zero today precisely because nothing is in flight. They are not
zero after stage 6d or stage 12 is dispatched, which is why the plan put the
amendment here and not later.

**Mechanically** it is the same work as RFC 096's round-trip: two commits, the
inbound references repointed and back, the index row moved and back, the G16
baseline churned both ways, net zero outside the RFC itself. I have just done
it once and it took one sitting.

## What I am asking for

One answer covering both:

1. **The sentence above** — or a correction to it.
2. **The round-trip**, run in one pass the way RFC 096's was, since the design
   is already approved and the review is of the wording rather than of a
   design.

On a yes I execute it, then dispatch stage 6d against the amended record.

## What I am not asking for

Not 096-A's acceptance, not the lockout decision, not RFC 107, not the held
CLI finding, not the M3 veto. Those are in the schedule at their own points
and none of them blocks this.
