# RFC 123 — independent design review request

**RFC.** [RFC 123 — An endpoint that authenticates a client costs the caller something](../../proposed/123-authenticating-a-client-costs-the-caller.md). **Proposed.**
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff, and it **narrowed this finding itself** in the RFC 120
triage — from four endpoints to two — which is disclosed here rather than left
to be noticed.
**Baseline.** `9712215` or later.
**Scope.** Read-only. Report findings.
**Why now.** RFC 122 is accepted and dispatched for implementation; this is the
one review beside it. RFC 124 waits.

## What to attack

1. **Confirm the measurement** at the baseline, with `file:line`: that
   `/oauth2/introspect` and `/oauth2/revoke` take no rate limit while
   `/oauth2/token` does, and **what Argon2 parameters are actually configured** —
   D1's whole severity is the cost per call, and the RFC asserts 64 MiB without
   having measured the configured value.
2. **Is a limit the right instrument, and what shape?** An introspection endpoint
   may be called by a resource server on every request it serves. A limit sized
   for a browser breaks it. **Say what traffic shape you assumed**, and if the
   honest bucket is per-client rather than per-IP, say so — and then work through
   the tension that per-client requires authenticating *before* limiting, which
   is the ordering D2 is about. Do not pick one side and drop the other.
3. **D2 says the expensive step comes after the cheap ones.** Establish what the
   cheap ones actually are on each endpoint, and whether reordering changes any
   externally visible behaviour — in particular whether it changes *which* error
   a bad request gets, which is a protocol-conformance question (RFC 7662, RFC
   7009) and not only an internal one.
4. **D3 — a per-client failure counter, or a stated reason there is none.** The
   RFC deliberately does not assume one is right: it is state on an
   unauthenticated path, which is its own hazard. Recommend, with the hazard
   named.
5. **D4 is a measurement and must produce a committed artefact.** Time both paths
   of `authenticate_client` — known-confidential versus unknown/public/disabled —
   with enough samples to say something, in a stated environment. **Then
   recommend**: close the difference with a dummy verification, or record it as
   accepted with the numbers that justify it. Neither reflex is acceptable
   without the numbers.
6. **Anything else**, including whether this is worth doing before anything else
   in the queue. The attacker needs a valid client id and gains CPU, not secrets.
   **If the honest answer is that the exposure does not justify the limit's risk
   of breaking a legitimate resource server, that is a finding.**

## What to return

The usual package: confirmations with `file:line`, the configured Argon2 cost,
the D4 measurement as a committed file with its method, findings ranked
blocker / high / medium / low, your answers to 2–6, and a recommendation.
