# RFC 096-B1 stage 5 — verified-identity mapping

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09. This is its stage 5. *(Its "F05" annotation was wrong —
F05 is terminal attempt failure. Corrected in the plan 2026-10-10.)*
**Prior.** Stages 0–4 complete. The callback now validates and builds an
`IdentityCapability`; nothing consumes it yet.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## Carried in: one required rename from stage 4's review

`current_provider_version_and_generation` ignores its `&FederationProviderRow`
argument and returns `(0, 0)`. The doc comment says so; **the name does not, and
the name is what gets read at the call site.** Rename it so it cannot be
misread — `placeholder_provider_version_and_generation` or your preference — and
keep the comment. This file is already yours this stage, so it goes here rather
than in a round of its own.

Why it matters beyond tidiness: `/start` and the callback both call it, so the
superseded comparison is `(0,0)` against `(0,0)` — **tautological, not merely
untested**. A reader who trusts the name will believe superseded attempts are
rejected. They are not, until M2b gives `federation_provider` the columns.

## Scope

RFC 096 `:693-711`. Consume the capability stage 4 builds and resolve it to a
local identity.

- **`(provider_id, sub)` is the sole lookup key** (`:695`). Not email, not
  `preferred_username`, not anything else — *"Last-seen email is stored only when
  syntactically valid and is never a lookup key."*
- **An existing link may authenticate without email** (`:695-696`). A link that
  resolves is enough; absence of email is not a failure for an existing link.
- **`link_only`**: one generic *"local account link required"* result (`:699`).
  Generic is load-bearing — it must not reveal whether the upstream identity is
  known locally. The unsigned pending-link cookie and the incomplete
  `/auth/federated/link` skeleton **are to be removed, not reused** (`:700-701`);
  M4 does not create a link.
- **`provision_on_first_login`**: requires a present email **and**
  `email_verified = true` (`:706-707`). If any local user already has that
  normalized email, **deny as a takeover collision; never auto-link**
  (`:707-708`). An absent or unverified email simply denies (`:710-711`).

**Stage 4 already gives you the verified-email state** in the capability:
`Option<String>`, `Some` only when the claim was present, mailbox-shaped, within
bound and `email_verified` exactly `true`. Do not re-derive it.

## Not in scope

**No session, no link row, no provisioning transaction.** Stage 6 is F04's
Class-A commit — link, `[Fed]` session and cap, and the
`auth.federation.provisioned` event in one transaction. This stage **resolves**
and **decides**; stage 6 acts.

So what this stage returns is a decision value, not a mutation. Name the states
explicitly — existing link resolved, link-required, provision-eligible,
denied-collision, denied-unverified — so stage 6 matches on them rather than
re-deriving the reasoning.

**Do not remove the old paths.** Stage 7 still owns that.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The rename**, done.
2. **A test per decision state**, asserting the state and not merely that
   something failed.
3. **The takeover-collision test** specifically: a local user already holding the
   normalized email, and a denial that does **not** auto-link. This is the one
   where a wrong answer silently joins two identities.
4. **A test that `link_only`'s result is generic** — identical for a known and an
   unknown upstream identity. If the two differ in body, status or timing in any
   way you can observe, say so.
5. **Confirmation that the pending-link cookie and the `/auth/federated/link`
   skeleton are removed**, with the grep that shows it.
6. **Normalization**: say which function normalizes the email for the collision
   check, and that it is the same one the local user path uses. Two
   normalizations would be a silent way to miss a collision.
7. **Mutation evidence** per decision branch.
8. **Per-hunk SHA-256** against the tip you named.
9. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace`, and **G17** if a command row changes.
10. **Anything you think is wrong with this dispatch.** Five of the last six
    packages corrected something of mine, and stage 4's correction to the
    "as recorded on the attempt row" phrasing was the right kind: it named the
    mechanism I had assumed without stating.
