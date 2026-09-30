# RFC 122 — independent design review request

**RFC.** [RFC 122 — A one-time secret does not travel where it persists](../../accepted/122-one-time-secrets-do-not-travel-where-they-persist.md). **Proposed.**
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff, though it confirmed two of these findings in the RFC 120
triage — disclosed here rather than left to be noticed.
**Baseline.** `8c137bd` or later.
**Scope.** Read-only. Report findings.
**Why this one now.** RFCs 123 and 124 wait behind it, so the dev team holds one
review and no build.

## What to attack

1. **Confirm the two measurements**, with `file:line` at the baseline: the
   rotated client secret carried in a redirect query string and read back by the
   edit page; and which surfaces showing a one-time secret do and do not carry
   `Cache-Control: no-store`. **The RFC's list is a starting point and says so —
   find the ones it missed.** A secret shown once that nobody has thought of is
   the whole risk here.
2. **D1 says a secret is never a URL component. Is that achievable for every
   surface**, or is there one where the value genuinely must survive a
   navigation? The handoff names three candidates and says the simplest — the
   operator copies it from the POST response, and it does not survive at all —
   may be right. **Say what that costs an operator before agreeing with it**;
   the person who has just rotated a client secret and navigated away has lost
   it, and the recovery is another rotation.
3. **D2 — the edit page renders whatever the query parameter contains.** Confirm
   it, and say whether that is worth anything to an attacker on its own: who can
   put a value there, what an administrator would see, and whether it is a
   phishing surface or merely untidy. **Do not assume it is severe because it is
   adjacent to a real finding.**
4. **D3's placement is the design decision.** Where do the headers go so the
   next surface cannot forget them — a response builder, a typed
   "carries a secret" wrapper, a router layer on named routes? Each has a
   failure mode; name the one you would ship and how it fails.
5. **D4 asks for a closed, checked list.** Is a test that enumerates
   secret-bearing surfaces buildable from the code rather than hand-maintained?
   RFC 120's route test derived its list from the router; is there an equivalent
   here, or is a hand-written list with a test that it is complete the honest
   best?
6. **Anything else** — including whether this RFC is worth doing at all. It says
   plainly that the exposure is modest: administrative surfaces, a service with
   no known deployment, responses a browser is unlikely to cache. **If you think
   the honest answer is "not yet, and here is what would change that", that is a
   finding, not a failure to deliver.** The counter-argument the RFC makes is
   that a secret in a URL is cheap to fix now and expensive after a log
   aggregator arrives; test it.

## What to return

The usual package: the confirmations with `file:line`, the surfaces the RFC
missed, findings ranked blocker / high / medium / low, your answers to 2–6 each
citing what you read, and a recommendation — accept as written, accept with the
changes you name, or do not accept.
