# RFC 096-B1 stage 12 — remove the shipped defect's two paths, and wire the federated second factor

> ## Whose turn it is: **the implementation role's.**
> One item inside this stage waits on `@nabbisen` — see §5 — and it is carved
> out so nothing else is blocked by it. Everything in §§1–4 and §6 can be
> built today.

**Dispatched.** 2026-10-17 JST, by the architect.
**RFC.** 096 — **Status: Accepted**. RFC 094 — **Accepted**, re-accepted
2026-10-15.
**Prior.** All of 0–5, 6a–6e, 7–11, 11b landed. Stage 6d landed `a6524e0`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.
**Release.** This stage is **cycle D's gate**. When it lands, v0.81.0 can be
cut. Nothing here is rushed by that; the gate exists so the release carries a
working federated MFA path rather than three inert stages.

## 0 — this is the largest stage in 096-B1, and it may want splitting

Five distinct pieces, listed below. Stage 10's split bought a real decision and
stage 11's would have bought nothing; I do not know which this is. **If, having
measured it, you think this should be two dispatches, say so with the seam you
would cut and why** — that argument has carried before and the stage plan's own
precedent is 6a–6e.

## 1 — the trust-on-TLS decode

`decode_id_token_claims` (`http/id_token.rs:30`) decodes an upstream ID token
without verifying its signature, and `handlers/federation.rs:1093` is still
what production calls. Its own module doc says so in the open: *"**`decode_id_token_claims`
is still what production calls, and it still…**"*.

096-A built the verification this replaces. Remove the legacy decode from the
federated callback and route it through the verified path. The function itself
may have other callers — **measure before deleting**, and say what you found.

## 2 — the cookie-replay path

The sealed `FedState` cookie (`handlers/federation.rs:409-447`,
`seal_state`). Stage 6's durable attempt row replaced what this carried;
removing it is what makes the attempt row the single source of that state.
Removing the legacy path takes the generic pending-MFA mechanism with it,
which is why stages 8–11 had to land first.

## 3 — the two discovery items 096-A could not wire

Both need `fetch_discovery` (`handlers/federation.rs:93`, called at `:338` and
`:703`), which 096-A was forbidden to touch and this stage is not:

- the configured-value cross-checks;
- `validate_discovery_bytes`'s tightened bounds.

## 4 — `:1067`'s audit note

The slug-rather-than-internal-id audit note. Small, and it travels with this
stage because it is in the same handler.

## 5 — wiring the federated second factor, and the one item that waits

This is what makes stages 8–11b reachable: `/admin/login/mfa` switches over so
a federated continuation is completed through the verifier and F03 rather than
through the generic `login_pending_mfa` row.

**Three things are already recorded for this wiring** and must not be
rediscovered:

- **The account-lock question is open, and it is `@nabbisen`'s.** The request
  is at
  `rfcs/handoffs/096-upstream-oidc-federation/federated-lockout-request-2026-10-17.md`,
  with my recommendation and a correction to a severity figure I got wrong
  twice. **Build everything else; leave this hook out until the
  answer lands**, and say in your package that you did. If the answer is the
  recommendation, it is one call to `record_second_factor_failure` plus a test
  that a wrong federated second factor advances the same counter a wrong local
  one does.
- **`FED_MFA_PENDING_COOKIE`'s `max_age`** is a flat 300 seconds on every call
  including a restart, while stage 11b deliberately leaves the row's own
  `expires_at` alone — so a late restart hands the browser a cookie claiming
  more life than the row has. Fix it here; it is this stage's handler.
- **Direction-2 `MethodMismatch` burns an attempt** when a user switches method
  mid-continuation. That is RFC-conformant (`:763-764`: "method/ceremony
  substitution is one failure"), so do not change the counting — but the person
  has to be told why, and that is this stage's UI.

## 6 — the CHANGELOG entry, from now on in every stage

`Unreleased` has needed two catch-ups because my dispatches asked for hashes,
gates and counts and never for the entry. **This stage writes its own**, in the
same package, in the register the file already uses. That is the standing
change, not a one-off for this stage.

## What this stage must not do

**No release.** Cycle D is cut separately, by `@nabbisen`'s sign-off.

**Nothing in 096-B2's scope** — provider and link commands are blocked on M2b.

**No new migration** unless you find one is needed, which would be a finding.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **Each of §§1–4**, with what you measured before removing anything —
   particularly §1's other callers.
2. **§5's wiring**, with the lockout hook explicitly absent and named as
   waiting, and the two recorded items fixed.
3. **Evidence that the legacy paths are gone**, not merely unused: a test or a
   scope check that fails if either comes back.
4. **The CHANGELOG entry** (§6).
5. **Per-hunk SHA-256** against the tip you named, regenerated after your
   *last* commit.
6. **The full matrix, all 24 green.**
7. **Count reconciliation** from `a6524e0`: `sui-id-core --lib` **201**,
   `sui-id-store --lib` **529**, `sui-id --lib` **560**, `sui-id --test e2e`
   **533**, `sui-id --test compile_fail` **12**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
8. **Anything wrong with this dispatch**, including §0's question about
   splitting. Your last six rounds corrected me on the trybuild harness, the
   `sealed` flag, the G15 framing, two RFC citations, a factual error in an RFC
   I wrote, a consequence I asserted without reading a foreign key, the
   difference between "the exploit is unreachable" and "the function is", and
   two checks I called misplaced that were absent. That rate is the useful
   number.
