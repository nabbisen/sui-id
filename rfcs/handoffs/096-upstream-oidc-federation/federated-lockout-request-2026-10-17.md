# The federated second factor has no account lock — a request for your call

> ## Whose turn it is: **`@nabbisen`'s.**
> One question, one recommendation. Stage 12 is dispatched alongside this and
> carves this item out explicitly, so nothing is blocked while it is open — but
> stage 12 is what makes the path reachable, so the answer wants to land before
> that stage finishes rather than after.

**Written.** 2026-10-17 JST, by the architect.

## First, a correction to what I told you

I reported this twice as *"low severity on entropy grounds"* — once in the
stage 11b review and once in the schedule. **That was wrong, and I did not do
the arithmetic before saying it.** Doing it now:

```
guesses/day            = 1440      (5 per continuation, a fresh one every 300s)
P(success) per day     = 0.4311%
P(success) in 30 days  = 12.16%
P(success) in a year   = 79.34%
```

against a 6-digit TOTP with the ±1-step window the verifier accepts (three
live codes out of 10⁶ at any instant). A sustained attacker is not facing a
negligible probability; they are facing a near-certainty inside a year. The
number that made it sound small — 1440 a day — is the one I quoted without
compounding it.

Recovery codes remain out of reach at that rate; TOTP does not.

## The gap

`record_second_factor_failure` — RFC 102's L07, which increments
`users.mfa_failure_count` and locks the account with the password path's
backoff — is called from exactly one place: `mfa.rs:249`, the **local**
`verify_pending`. F03, `promote` and the whole `federated_mfa` module
reference no lockout at all.

So the federated second factor's only bound is five guesses per continuation,
and after stage 11b a *genuinely expired* continuation is correctly replaced by
a fresh one — which is right for a user who timed out, and is also what makes
the rate above sustainable. There is no escalating cost and no account lock.

## The objection I expected, and why I no longer think it holds

Feeding the shared counter means a federated attacker can lock the victim out
of **local** sign-in too — turning a federated nuisance into a local denial of
service. That was my first reaction and it is why I did not simply recommend
it earlier.

Measured, it is symmetric rather than new. Reaching the *local* second-factor
step requires the password first: `session.rs:229-232` only issues a pending-MFA
row after the password check succeeds, and the comment there says so. Reaching
the *federated* second-factor step requires holding the upstream identity. In
both cases the attacker has already proven factor one, and locking the account
is the designed response to repeated second-factor failures from someone who
has. The federated door is not a cheaper way to trigger the lock; it is a
different first factor.

## Recommendation

**Feed `record_second_factor_failure` from the federated path, the same way
the local path does**, so both doors to an account share one escalating bound
and one lock.

- It reuses a mechanism RFC 102 already designed, reviewed and shipped, rather
  than inventing a federated-only backoff.
- It closes the compounding exposure above rather than documenting it.
- The per-continuation cap of five stays as it is; this is the bound *across*
  continuations that does not currently exist.

The alternative worth naming: a federated-scoped backoff that bounds the
federated path without touching the account lock. It avoids the DoS objection
entirely, at the cost of a second mechanism doing a job the first already does
— and the objection, measured, is weaker than it looked.

## What the answer changes

**If you take the recommendation**, stage 12 gains it: the handler that wires
the federated MFA submission calls the same failure path the local one does,
with a test that a wrong federated second factor advances the same counter a
wrong local one does.

**If you prefer the scoped alternative, or prefer to leave it**, stage 12
ships without it and the asymmetry is recorded in `docs/threat-model.md`
instead — which it should be either way, since the threat model does not
currently mention it.

Either answer is small work. What I would not do is let it ship unrecorded.
