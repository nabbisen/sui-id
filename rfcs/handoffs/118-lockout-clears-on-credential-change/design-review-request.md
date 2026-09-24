# RFC 118 — independent design review request

**RFC.** [RFC 118 — A credential change clears the lockout, and the user is told](../../accepted/118-lockout-clears-on-credential-change.md). **Proposed.**
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** The routing recorded in `ROADMAP.md` §S1 (2026-08-26); §S1b puts that
rule itself to `@nabbisen` for a prospective decision, which does not change
where this review goes today.
**Why now, and why before anything is built.** RFC 118 is **Proposed**. RFC 105
was dispatched for implementation while Proposed, three days ago, and the
consequence could not be undone: the role that would have reviewed its design
had built it, so RFC 105 shipped with **no independent design review at all**
and says so in its own header. That will not happen twice. **This review comes
first; acceptance follows it; implementation follows acceptance.**
**Baseline.** `a83cb6a` (tag `0.78.0`) or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## What RFC 118 claims, and what to attack

An unauthenticated party who knows a username can lock an account and keep it
locked, and **no credential change clears the lock** — not U10 (reset
completion), not U09 (self-service change). So a user can set a password and
still be refused, and the refusal says "invalid credentials".

## 1. The measurements — confirm or refute each

One row per claim: the claim, the `file:line`, whether it holds.

1. `lockout_backoff` reaches **24 hours at ten failures** and stays there.
2. An active lock refuses sign-in **before** the password is checked.
3. **U10 clears neither `failed_login_count` nor `locked_until`.**
4. **U09 clears neither either.**
5. The only thing that resets the counter is a successful password verify
   (`clear_lockout`), which the lock itself prevents.
6. **The denial is real end to end.** Construct it: lock an account, complete a
   valid reset, then fail to sign in with the new password. If it does not
   reproduce, say so — the RFC rests on it.

## 2. Is D1 right, and is it safe?

7. **Is "any successful credential change clears the counter and the lock" the
   correct rule**, or is there a path that sets a credential where clearing
   would be wrong? Enumerate the credential writers and answer for each.
8. **D2's justification, attacked rather than accepted.** The RFC argues that
   completing a reset proves possession of a single-use out-of-band token, and
   changing a password proves knowledge of the current one — both strictly
   stronger than the password attempt the lock was counting. **Is that true in
   every case?** Consider in particular: a reset token obtained by an attacker
   who *caused* the lockout; an administrator-issued provisioning link; and a
   session hijack using U09.
9. **In the same transaction.** D1 says the clearing happens in the transaction
   that writes the credential. Confirm that is expressible for both U09 and U10
   without a second write path, and say what the injected-failure test should
   assert.
10. **Does clearing the lock lose anything an operator relies on?** The counter
    feeds the audit trail and the lockout events. Say what disappears from an
    operator's view and whether anything should be recorded in its place.

## 3. D3 and D4 — the part most likely to be got wrong

11. **Where exactly can the message be shown** such that it is reachable **only**
    with a valid consumed token? Name the surface and the state it reads.
12. **Is there any path by which D3's message, or a timing or response
    difference caused by it, becomes observable to an unauthenticated visitor?**
    This is the question that decides whether D3 is safe. Trace it; do not
    assert it.
13. **What should it say?** The user has just set a password and been refused.
    Say what information is both useful and safe — whether the unlock time can
    be shown, and what it implies if it can.
14. **D4 says the sign-in form does not change.** Confirm that clearing the lock
    does not alter any sign-in response, including timing, for any account.

## 4. Anything else

15. **Threats this design introduces or misses**, including whether clearing on
    credential change gives an attacker who holds a token any capability they
    did not already have.
16. **Anything that cannot be built as described**, or is better built another
    way, including what the RFC does not mention and should. The backoff
    schedule is explicitly **not** reopened; say so if you think it should be,
    but as a separate recommendation.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the claim table for §1, one row per claim, with `file:line`;
- findings ranked blocker / high / medium / low;
- your answers to items 6–16, each citing what you read;
- a recommendation: accept as written, accept with the changes you name, or do
  not accept.

**"Do not accept" is a legitimate outcome** and will not be treated as a failure
to deliver. Do not implement anything: RFC 118 is Proposed, and this review is
what lets it be accepted.
