# RFC 096-B2 — completion assessment, 2026-11-12

> ## Whose turn it is: **yours**, for three decisions (§5).

**Prepared by.** The architect, 2026-11-12, after stage 7 landed.
**Asked of you.**
1. Accept 096-B2.
2. Decide whether 096-B1's unrun stage 14, the live run against a real
   upstream, comes before 0.90.0's tag.
3. Whether the `test_id` cleanup (§4) goes into 0.90.0's preparation.

## 1 — the closure prerequisite, against the evidence

RFC 096 `:45` names 096-B2's closure evidence: *"federation provider and
link commands on the Class-A seam with rollback evidence"*, assessed by the
architect. The same clause names you as the one who accepts it.

| Command | What | Event | Rollback evidence |
|---|---|---|---|
| C16 | create, at startup | `federation.provider.created` | `c16_injected_failure_before_append_rolls_back_the_create` |
| C17 | check and enable; disable | `federation.provider.enabled`, `.disabled` | `c17_enable_injected_failure_before_append_rolls_back_everything`, `c17_disable_injected_failure_before_append_rolls_back_everything` |
| C18 | delete | `federation.provider.deleted` | `c18_injected_failure_before_append_rolls_back_everything` |
| C20 | unlink | `federation.link.deleted` | `c20_injected_failure_before_append_rolls_back_everything` |
| C23 | trust-policy replacement, at startup | `federation.provider.policy_updated` | `c23_injected_failure_before_append_rolls_back_everything` |
| C25 | disable on removal from configuration | `federation.provider.removed_from_configuration` | `c25_injected_failure_before_append_rolls_back_everything`, **written at this assessment** (§1, below) |
| C26 | display-label update | `federation.provider.display_name_changed` | `c26_injected_failure_before_append_rolls_back_the_change` |
| C24 | **retired** into C23 (A4 corrected) | — | — |
| C19 | **withdrawn** (A1) | — | — |

- **Every one is sealed and Class-A.**
- G20 (`check-m2a-rollback-coverage.sh`) refuses a declared command without
  rollback coverage, and it passes on `main`.
- **C25 had no real rollback evidence, until this assessment.** Its
  registered "rollback" test called C25 on an already-disabled provider, a
  no-op with nothing to roll back. G20 counted it, because it only checks
  that a test registers coverage. S2b's dispatch had required a real one,
  and **S2b's review, the architect's, missed that this wasn't one.**
  `c25_injected_failure_before_append_rolls_back_everything` now injects a
  failure into C25's real transaction. The provider stays enabled, with the
  same generation and evidence, the pending attempt stays pending, and no
  event is written. With the injection removed, the test fails, so it
  is not vacuous.
- **Every test named in 096-B2's contract rows exists.** Checked at this
  assessment; three did not, and they are corrected. Two were mutation
  descriptions, not tests, and one was a test renamed in S2b.

## 2 — what 096-B2 delivered beyond its clause

- **Cycle L's four protections:**
  1. a startup warning per provider;
  2. the login page offers no disabled provider, and now offers the enabled
     ones;
  3. the admin page shows state and action in one place;
  4. an upgrade note that leads.
- **Found and fixed during the cycle, each recorded in its stage's review:**
  - **A4 was wrong** (corrected and re-accepted).
  - **The schema stored only part of the trust policy** (S2a).
  - **A JWKS with no usable key passed the check** (S3).
  - **C17's window would have measured clock skew** (the decision).
  - **The login page never had buttons** (RFC 004's gap, S5b).
  - **A federated sign-in never returned to an OIDC authorization** without
    a second factor (S5b).
  - **The slug was documented but not enforced** (S5b).
  - **The issuer host rule**, deferred since S2a (S6b).

## 3 — independence, stated plainly

**The architect is not independent of 096-B2.** It wrote the stage plan and
every dispatch, and the amendments A1–A4 and the C17 seam. It also folded code
into **every** stage. This document is preparation; your acceptance is the
closure, as it was for 096-A (RFC 096 header, `:13`).

## 4 — what remains open, and is not 096-B2's

- **096-B1's stage 14 has never run.** That is *"representative
  live-integration evidence"*: one public upstream, your test credentials,
  outside CI (`096-b1-stage-plan-2026-10-09.md:357-361`). It is a 096-B1
  closure item, so **096-B1 is not closable without it**, and M4-B with it.
- **096-C**: the hostile-provider matrix, the live canary, and the closure
  review (RFC 096 `:1056`).
- `http/federation_cache.rs` is built but not wired into sign-in (S2b
  review).
- A test that signs a real passkey assertion (RFC 143's closure, open).
- **46 `test_id` entries in older rows (RFC 094's) name no function.**
  Examples are `a_u01_create_normal` and `a_k01_signing_rotate`. They look
  like case labels from before the tests existed, and no gate checks that a
  `test_id` names a real test. That is how 096-B2's three slipped through.
  Recommended: fold into 0.90.0's preparation two things, the mechanical
  replacement of each with its real test, and a check in G23 that every
  `test_id` resolves. Otherwise the contract keeps citing evidence that
  isn't there.

## 5 — your decisions

1. **Accept 096-B2.** Recommended. The clause is met, the evidence is in
   §1, and every stage's review is in `.git-exclude/reviewed/`.
2. **Stage 14 before 0.90.0's tag?** Recommended: **yes**. 0.90.0 is the
   first release in which an administrator checks a provider against a real
   upstream and re-enables it. Everything so far has run against mocks.
   - **What it needs from you:** a test OAuth client at one public provider
     (Google is the documented example), with its credentials in your
     environment. Never in a file, and never given to the architect.
   - **What the architect would do:** write the runbook (configure, check
     and enable, sign in through the button, unlink, disable), read the
     result you send back, and fix what it finds before tagging.
   - **The alternative:** tag 0.90.0 on mock evidence, and run stage 14
     against the release.
3. **The `test_id` cleanup (§4) in 0.90.0's preparation?** Recommended:
   **yes**. It is mechanical, the dev team can do it, and the G23 check stops
   it recurring.
