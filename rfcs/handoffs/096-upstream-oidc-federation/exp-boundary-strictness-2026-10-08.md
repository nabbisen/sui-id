# Decision request — RFC 096 `:660`'s `exp` boundary is one instant stricter than anything we build

**Raised.** 2026-10-08, by the architect, for `@nabbisen`.
**Found by.** The implementation role, during RFC 096-A stage 6b, correcting an
assertion in my own dispatch.
**Status.** **Open. Nothing is blocked by it** — stage 6b is accepted and
landed; stage 6c proceeds regardless.
**What is being asked.** Whether to amend one line of an Accepted RFC's wording.

## The measurement

RFC 096's required claim matrix states three time rules. Two are non-strict and
one is strict:

| Line | Claim | Rule as written |
|---|---|---|
| `:660` | `exp` | valid only while `now < exp + 60s` — **strict** |
| `:661` | `iat` | `created_at - 60s <= iat <= now + 60s` — non-strict |
| `:662` | `nbf` | `nbf <= now + 60s` — non-strict |

`jsonwebtoken` 10.3, which performs the `exp` and `nbf` checks, accepts when
`now <= exp + 60` — non-strict. Read from `validation.rs:291`, where the reject
condition is `exp - 0 < now - 60`, and proven in a test that signs one token at
`exp = now - 60` (accepted) and one at `exp = now - 61` (refused).

So at the single instant `now == exp + 60`, the RFC's wording refuses a token
the implementation accepts. Its `nbf` rule, by contrast, matches the library
exactly.

**This is my error, not a defect in anyone's work.** My stage 6b dispatch told
the implementation role that the library's check was *"exactly the rule"*. I had
derived the inequality correctly and then restated it in prose as strict,
dropping the equality case. They re-derived it, found the discrepancy, proved it
by test rather than argument, and escalated instead of patching — which was
right, because the dispatch had put that window logic out of their scope.

## Why I am not fixing it in code

We could enforce the strict bound ourselves with a second `exp` comparison in
our own validator. **I recommend against it**, for the reason I accepted from
the implementation role one stage earlier: in stage 6a they argued that putting
the `aud` check partly in `jsonwebtoken`'s configuration and partly in our code
would mean *"one rule reasoned about in two places, for no gain"*. I agreed and
that shaped the design. Adding our own `exp` comparison on top of the library's
is the same mistake, for a one-instant difference. Consistency cuts the same way
here as it did there.

The other mechanical routes are worse: `leeway = 59` would make the boundary
strict for `exp` but also break `nbf`, whose rule the RFC states non-strictly
and which the library currently matches exactly; and
`reject_tokens_expiring_in_less_than = 1` overshoots to `now <= exp + 59`,
stricter than the RFC on the other side.

## What I recommend, and what I am not claiming

**Recommendation: amend `:660` to `now <= exp + 60s`.** My reasoning:

1. It makes the RFC describe what is actually built, and has been built all
   along — no behaviour changes, in either direction.
2. It makes `exp` consistent with its own two siblings at `:661` and `:662`,
   both already non-strict. I can find no stated rationale for `exp` alone
   being strict, and the surrounding text (`:675`, the fixed symmetric skew)
   does not distinguish it.
3. The security content of the difference is one instant at one-second
   granularity, inside a 60-second allowance that is itself a deliberate
   concession to clock skew. A token accepted at exactly `exp + 60` rather than
   refused does not change the risk the 60 seconds already accepts.

**What I am not claiming.** I do not know that the strict `<` at `:660` was
incidental. It reads like a wording artifact to me, but you wrote the
requirement and may have meant the boundary to be exclusive. If you did, say so
and I will dispatch the strict check as its own stage with the duplication
accepted deliberately and the reason recorded — that is a defensible outcome,
just not the one I would choose.

**Why this is yours and not mine.** It edits a security invariant's statement in
an Accepted RFC. I judge it immaterial under RFC 000 — no behaviour, scope or
prerequisite moves — so I do not think it forces a return to `proposed/`. But
"immaterial change to a security invariant" is exactly the judgement I should
not make alone about your requirement.

## The three outcomes

1. **Amend `:660` to non-strict.** My recommendation. One line, no code change,
   G16 and G11 re-run. 096-A's closure evidence then matches the RFC it cites.
2. **Keep `:660` strict and enforce it.** A new stage adds our own `exp`
   comparison, accepting the duplication, with the reason written where a future
   reader will find it.
3. **Keep `:660` strict and record the deviation.** Documented divergence, no
   code change. I like this least: a closure review would then have to assert
   the RFC is satisfied when one of its stated boundaries is not.

Until you rule, the code stands as accepted and the divergence is documented in
`validation_for`'s doc comment and in stage 6b's review result.
